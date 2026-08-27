mod project;
mod protocol;
mod support;

use std::{
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
};

use decay_semantic::{DiagnosticPhase, Environment, ExternalSymbol};
use decay_syntax::{Item, Member, parse};
use project::ProjectIndex;
use protocol::{read_message, write_message};
use serde_json::{Value, json};
use support::{
    KEYWORDS, completion_chain, completion_item, container_members, hover_symbol,
    initialization_root, offset_at, span_range, string_argument, symbol_completion, type_members,
    word_at,
};

struct Server {
    documents: HashMap<String, String>,
    environment: Environment,
    root: Option<PathBuf>,
    project: ProjectIndex,
    shutdown: bool,
}

impl Server {
    fn new() -> Self {
        Self {
            documents: HashMap::new(),
            environment: sindri_decay::environment(),
            root: None,
            project: ProjectIndex::default(),
            shutdown: false,
        }
    }

    fn handle(&mut self, message: &Value, output: &mut impl Write) -> io::Result<bool> {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => self.initialize(id, &params, output)?,
            "initialized" => {}
            "shutdown" => {
                self.shutdown = true;
                if let Some(id) = id {
                    write_message(output, &json!({"jsonrpc":"2.0","id":id,"result":null}))?;
                }
            }
            "exit" => return Ok(false),
            "textDocument/didOpen" => self.did_open(&params, output)?,
            "textDocument/didChange" => self.did_change(&params, output)?,
            "textDocument/didSave" => self.did_save(&params, output)?,
            "textDocument/completion" => {
                Self::respond(id, self.completion(&params), output)?;
            }
            "textDocument/hover" => {
                Self::respond(id, self.hover(&params), output)?;
            }
            "textDocument/documentSymbol" => {
                Self::respond(id, self.document_symbols(&params), output)?;
            }
            _ => Self::respond(id, Value::Null, output)?,
        }
        Ok(!self.shutdown || method != "exit")
    }

    fn initialize(
        &mut self,
        id: Option<Value>,
        params: &Value,
        output: &mut impl Write,
    ) -> io::Result<()> {
        self.root = initialization_root(params);
        self.project = ProjectIndex::scan(self.root.as_deref());
        Self::respond(
            id,
            json!({
                "serverInfo": { "name": "decay-lsp", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {
                    "textDocumentSync": 1,
                    "completionProvider": { "triggerCharacters": [".", "\""] },
                    "hoverProvider": true,
                    "documentSymbolProvider": true
                }
            }),
            output,
        )
    }

    fn respond(id: Option<Value>, result: Value, output: &mut impl Write) -> io::Result<()> {
        if let Some(id) = id {
            let mut response = serde_json::Map::new();
            response.insert("jsonrpc".to_owned(), Value::String("2.0".to_owned()));
            response.insert("id".to_owned(), id);
            response.insert("result".to_owned(), result);
            write_message(output, &Value::Object(response))?;
        }
        Ok(())
    }

    fn did_open(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        if let (Some(uri), Some(text)) = (
            params.pointer("/textDocument/uri").and_then(Value::as_str),
            params.pointer("/textDocument/text").and_then(Value::as_str),
        ) {
            self.documents.insert(uri.to_owned(), text.to_owned());
            self.publish_diagnostics(uri, text, output)?;
        }
        Ok(())
    }

    fn did_change(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str)
            && let Some(text) = params
                .pointer("/contentChanges/0/text")
                .and_then(Value::as_str)
        {
            self.documents.insert(uri.to_owned(), text.to_owned());
            self.publish_diagnostics(uri, text, output)?;
        }
        Ok(())
    }

    fn did_save(&mut self, params: &Value, output: &mut impl Write) -> io::Result<()> {
        self.project = ProjectIndex::scan(self.root.as_deref());
        if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str)
            && let Some(text) = self.documents.get(uri).cloned()
        {
            self.publish_diagnostics(uri, &text, output)?;
        }
        Ok(())
    }

    fn publish_diagnostics(
        &self,
        uri: &str,
        source: &str,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let analysis = decay_semantic::analyze_with_environment(source, &self.environment);
        let diagnostics = analysis
            .diagnostics
            .into_iter()
            .map(|diagnostic| {
                let source_name = match diagnostic.phase {
                    DiagnosticPhase::Syntax => "decay-syntax",
                    DiagnosticPhase::Semantic => "decay-semantic",
                };
                json!({
                    "range": span_range(source, diagnostic.span),
                    "severity": 1,
                    "source": source_name,
                    "message": diagnostic.message
                })
            })
            .collect::<Vec<_>>();
        write_message(
            output,
            &json!({
                "jsonrpc":"2.0",
                "method":"textDocument/publishDiagnostics",
                "params":{"uri":uri,"diagnostics":diagnostics}
            }),
        )
    }

    fn completion(&self, params: &Value) -> Value {
        let Some((source, offset)) = self.source_and_offset(params) else {
            return json!([]);
        };
        let before = &source[..offset.min(source.len())];

        if string_argument(before, "World.find").is_some() {
            return Value::Array(
                self.project
                    .entity_names
                    .iter()
                    .map(|id| completion_item(id, 12, Some("Scene entity")))
                    .collect(),
            );
        }
        if string_argument(before, "Audio.play").is_some()
            || string_argument(before, "Audio.loop").is_some()
        {
            return Value::Array(
                self.project
                    .audio_assets
                    .iter()
                    .map(|asset| completion_item(asset, 17, Some("Project audio asset")))
                    .collect(),
            );
        }

        let (chain, prefix) = completion_chain(before);
        if let Some(chain) = chain {
            return Value::Array(
                self.members_for_chain(&source, &chain)
                    .into_iter()
                    .filter(|(name, _)| name.starts_with(&prefix))
                    .map(|(name, symbol)| symbol_completion(&name, &symbol))
                    .collect(),
            );
        }

        let mut items = KEYWORDS
            .iter()
            .map(|keyword| completion_item(keyword, 14, Some("Decay keyword")))
            .collect::<Vec<_>>();
        items.extend(
            self.environment
                .globals()
                .map(|(name, symbol)| symbol_completion(name, symbol)),
        );
        items.extend(
            container_members(&source)
                .into_iter()
                .map(|(name, symbol)| symbol_completion(&name, &symbol)),
        );
        Value::Array(items)
    }

    fn hover(&self, params: &Value) -> Value {
        let Some((source, offset)) = self.source_and_offset(params) else {
            return Value::Null;
        };
        let Some(word) = word_at(&source, offset) else {
            return Value::Null;
        };
        if let Some((_, symbol)) = self.environment.globals().find(|(name, _)| *name == word) {
            return hover_symbol(word, symbol);
        }
        if let Some((_, symbol)) = container_members(&source)
            .into_iter()
            .find(|(name, _)| name == word)
        {
            return hover_symbol(word, &symbol);
        }
        if KEYWORDS.contains(&word) {
            return json!({
                "contents":{"kind":"markdown","value":format!("`{word}` · Decay keyword")}
            });
        }
        Value::Null
    }

    fn document_symbols(&self, params: &Value) -> Value {
        let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
            return json!([]);
        };
        let Some(source) = self.documents.get(uri) else {
            return json!([]);
        };
        let parsed = parse(source);
        let mut symbols = Vec::new();
        for item in parsed.program.items {
            let container = match item {
                Item::Script(container) | Item::Component(container) => container,
            };
            let children = container
                .members
                .into_iter()
                .map(|member| match member {
                    Member::Field(field) => json!({
                        "name": field.name,
                        "kind": 8,
                        "range": span_range(source, field.span),
                        "selectionRange": span_range(source, field.span)
                    }),
                    Member::Function(function) => json!({
                        "name": function.name,
                        "kind": 12,
                        "range": span_range(source, function.span),
                        "selectionRange": span_range(source, function.span)
                    }),
                })
                .collect::<Vec<_>>();
            symbols.push(json!({
                "name": container.name,
                "kind": 5,
                "range": span_range(source, container.span),
                "selectionRange": span_range(source, container.span),
                "children": children
            }));
        }
        Value::Array(symbols)
    }

    fn source_and_offset(&self, params: &Value) -> Option<(String, usize)> {
        let uri = params.pointer("/textDocument/uri")?.as_str()?;
        let source = self.documents.get(uri)?.clone();
        let line = usize::try_from(params.pointer("/position/line")?.as_u64()?).ok()?;
        let character = usize::try_from(params.pointer("/position/character")?.as_u64()?).ok()?;
        let offset = offset_at(&source, line, character);
        Some((source, offset))
    }

    fn members_for_chain(&self, source: &str, chain: &[String]) -> Vec<(String, ExternalSymbol)> {
        let Some(first) = chain.first() else {
            return Vec::new();
        };
        let mut current = if first == "this" {
            None
        } else {
            self.root_symbol_type(source, first)
        };

        if chain.len() == 1 && first == "this" {
            let mut members = self
                .environment
                .this()
                .members()
                .map(|(name, symbol)| (name.to_owned(), symbol.clone()))
                .collect::<Vec<_>>();
            members.extend(container_members(source));
            return members;
        }

        let start = usize::from(first == "this");
        for segment in &chain[start..] {
            let symbol = if start == 1 && current.is_none() {
                self.environment
                    .this()
                    .member(segment)
                    .cloned()
                    .or_else(|| {
                        container_members(source)
                            .into_iter()
                            .find(|(name, _)| name == segment)
                            .map(|(_, symbol)| symbol)
                    })
            } else {
                current.as_ref().and_then(|ty| {
                    type_members(&self.environment, ty)
                        .and_then(|host| host.member(segment).cloned())
                })
            };
            current = symbol.map(symbol_type);
            if current.is_none() {
                return Vec::new();
            }
        }

        current
            .and_then(|ty| type_members(&self.environment, &ty))
            .map_or_else(Vec::new, |host| {
                host.members()
                    .map(|(name, symbol)| (name.to_owned(), symbol.clone()))
                    .collect()
            })
    }

    fn root_symbol_type(&self, source: &str, name: &str) -> Option<decay_semantic::Type> {
        self.environment
            .globals()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, symbol)| symbol_type(symbol.clone()))
            .or_else(|| {
                container_members(source)
                    .into_iter()
                    .find(|(candidate, _)| candidate == name)
                    .map(|(_, symbol)| symbol_type(symbol))
            })
    }
}

fn symbol_type(symbol: ExternalSymbol) -> decay_semantic::Type {
    match symbol {
        ExternalSymbol::Value(ty) => ty,
        ExternalSymbol::Function(function) => function.return_type,
    }
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let mut server = Server::new();
    while let Some(message) = read_message(&mut input)? {
        if !server.handle(&message, &mut output)? {
            break;
        }
    }
    Ok(())
}
