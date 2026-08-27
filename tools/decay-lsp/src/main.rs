use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::{self, BufRead, Read, Write},
    path::{Path, PathBuf},
};

use decay_semantic::{DiagnosticPhase, Environment, ExternalSymbol, FunctionType, Type};
use decay_syntax::{Item, Member, Span, parse};
use serde_json::{Value, json};

const KEYWORDS: &[&str] = &[
    "script", "component", "fn", "let", "var", "if", "else", "while", "break",
    "continue", "return", "true", "false", "null",
];

#[derive(Default)]
struct ProjectIndex {
    entity_ids: BTreeSet<String>,
    audio_assets: BTreeSet<String>,
}

impl ProjectIndex {
    fn scan(root: Option<&Path>) -> Self {
        let mut index = Self::default();
        if let Some(root) = root {
            index.scan_dir(root, root);
        }
        index
    }

    fn scan_dir(&mut self, root: &Path, directory: &Path) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(path.file_name().and_then(|name| name.to_str()), Some(".git" | "target" | "node_modules")) {
                    self.scan_dir(root, &path);
                }
                continue;
            }
            let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            if matches!(path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(), Some("wav" | "ogg" | "mp3" | "flac")) {
                self.audio_assets.insert(relative.clone());
            }
            if !relative.ends_with(".scene.json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            collect_source_ids(&value, &mut self.entity_ids);
        }
    }
}

fn collect_source_ids(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(id) = map.get("source_id").and_then(Value::as_str) {
                into.insert(id.to_owned());
            }
            for child in map.values() {
                collect_source_ids(child, into);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_source_ids(child, into);
            }
        }
        _ => {}
    }
}

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

    fn handle(&mut self, message: Value, output: &mut impl Write) -> io::Result<bool> {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match method {
            "initialize" => {
                self.root = initialization_root(&params);
                self.project = ProjectIndex::scan(self.root.as_deref());
                if let Some(id) = id {
                    write_message(output, &json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "serverInfo": { "name": "decay-lsp", "version": env!("CARGO_PKG_VERSION") },
                            "capabilities": {
                                "textDocumentSync": 1,
                                "completionProvider": { "triggerCharacters": [".", "\""] },
                                "hoverProvider": true,
                                "documentSymbolProvider": true
                            }
                        }
                    }))?;
                }
            }
            "initialized" => {}
            "shutdown" => {
                self.shutdown = true;
                if let Some(id) = id {
                    write_message(output, &json!({"jsonrpc":"2.0","id":id,"result":null}))?;
                }
            }
            "exit" => return Ok(false),
            "textDocument/didOpen" => {
                if let (Some(uri), Some(text)) = (
                    params.pointer("/textDocument/uri").and_then(Value::as_str),
                    params.pointer("/textDocument/text").and_then(Value::as_str),
                ) {
                    self.documents.insert(uri.to_owned(), text.to_owned());
                    self.publish_diagnostics(uri, text, output)?;
                }
            }
            "textDocument/didChange" => {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str)
                    && let Some(text) = params.pointer("/contentChanges/0/text").and_then(Value::as_str)
                {
                    self.documents.insert(uri.to_owned(), text.to_owned());
                    self.publish_diagnostics(uri, text, output)?;
                }
            }
            "textDocument/didSave" => {
                self.project = ProjectIndex::scan(self.root.as_deref());
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str)
                    && let Some(text) = self.documents.get(uri).cloned()
                {
                    self.publish_diagnostics(uri, &text, output)?;
                }
            }
            "textDocument/completion" => {
                if let Some(id) = id {
                    let result = self.completion(&params);
                    write_message(output, &json!({"jsonrpc":"2.0","id":id,"result":result}))?;
                }
            }
            "textDocument/hover" => {
                if let Some(id) = id {
                    let result = self.hover(&params);
                    write_message(output, &json!({"jsonrpc":"2.0","id":id,"result":result}))?;
                }
            }
            "textDocument/documentSymbol" => {
                if let Some(id) = id {
                    let result = self.document_symbols(&params);
                    write_message(output, &json!({"jsonrpc":"2.0","id":id,"result":result}))?;
                }
            }
            _ => {
                if let Some(id) = id {
                    write_message(output, &json!({"jsonrpc":"2.0","id":id,"result":null}))?;
                }
            }
        }
        Ok(!self.shutdown || method != "exit")
    }

    fn publish_diagnostics(&self, uri: &str, source: &str, output: &mut impl Write) -> io::Result<()> {
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
        write_message(output, &json!({
            "jsonrpc":"2.0",
            "method":"textDocument/publishDiagnostics",
            "params":{"uri":uri,"diagnostics":diagnostics}
        }))
    }

    fn completion(&self, params: &Value) -> Value {
        let Some((source, offset)) = self.source_and_offset(params) else {
            return json!([]);
        };
        let before = &source[..offset.min(source.len())];

        if string_argument(before, "World.find").is_some() {
            return Value::Array(self.project.entity_ids.iter().map(|id| completion_item(id, 12, Some("Scene entity"))).collect());
        }
        if string_argument(before, "Audio.play").is_some() || string_argument(before, "Audio.loop").is_some() {
            return Value::Array(self.project.audio_assets.iter().map(|asset| completion_item(asset, 17, Some("Project audio asset"))).collect());
        }

        let (chain, prefix) = completion_chain(before);
        if let Some(chain) = chain {
            let items = self.members_for_chain(&source, &chain)
                .into_iter()
                .filter(|(name, _)| name.starts_with(&prefix))
                .map(|(name, symbol)| symbol_completion(&name, &symbol))
                .collect();
            return Value::Array(items);
        }

        let mut items = KEYWORDS.iter().map(|keyword| completion_item(keyword, 14, Some("Decay keyword"))).collect::<Vec<_>>();
        items.extend(self.environment.globals().map(|(name, symbol)| symbol_completion(name, symbol)));
        items.extend(container_members(&source).into_iter().map(|(name, symbol)| symbol_completion(&name, &symbol)));
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
        if let Some((_, symbol)) = container_members(&source).into_iter().find(|(name, _)| name == word) {
            return hover_symbol(word, &symbol);
        }
        if KEYWORDS.contains(&word) {
            return json!({"contents":{"kind":"markdown","value":format!("`{word}` · Decay keyword")}});
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
            let children = container.members.into_iter().map(|member| match member {
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
            }).collect::<Vec<_>>();
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
        let line = params.pointer("/position/line")?.as_u64()? as usize;
        let character = params.pointer("/position/character")?.as_u64()? as usize;
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
            match self.environment.globals().find(|(name, _)| name == first).map(|(_, symbol)| symbol.clone()) {
                Some(ExternalSymbol::Value(ty)) => Some(ty),
                Some(ExternalSymbol::Function(function)) => Some(function.return_type),
                None => container_members(source).into_iter().find(|(name, _)| name == first).and_then(|(_, symbol)| match symbol {
                    ExternalSymbol::Value(ty) => Some(ty),
                    ExternalSymbol::Function(function) => Some(function.return_type),
                }),
            }
        };

        if chain.len() == 1 && first == "this" {
            let mut members = self.environment.this().members().map(|(name, symbol)| (name.to_owned(), symbol.clone())).collect::<Vec<_>>();
            members.extend(container_members(source));
            return members;
        }

        let start = usize::from(first == "this");
        for segment in &chain[start..] {
            let symbol = if start == 1 && current.is_none() {
                self.environment.this().member(segment).cloned().or_else(|| container_members(source).into_iter().find(|(name, _)| name == segment).map(|(_, symbol)| symbol))
            } else {
                current.as_ref().and_then(|ty| type_members(&self.environment, ty).and_then(|host| host.member(segment).cloned()))
            };
            current = symbol.and_then(|symbol| match symbol {
                ExternalSymbol::Value(ty) => Some(ty),
                ExternalSymbol::Function(function) => Some(function.return_type),
            });
            if current.is_none() {
                return Vec::new();
            }
        }

        current.and_then(|ty| type_members(&self.environment, &ty)).map_or_else(Vec::new, |host| host.members().map(|(name, symbol)| (name.to_owned(), symbol.clone())).collect())
    }
}

fn type_members<'a>(environment: &'a Environment, ty: &Type) -> Option<&'a decay_semantic::HostType> {
    match ty {
        Type::Named(name) => environment.get_type(name),
        _ => None,
    }
}

fn container_members(source: &str) -> Vec<(String, ExternalSymbol)> {
    let parsed = parse(source);
    let Some(item) = parsed.program.items.first() else {
        return Vec::new();
    };
    let container = match item {
        Item::Script(container) | Item::Component(container) => container,
    };
    container.members.iter().map(|member| match member {
        Member::Field(field) => (
            field.name.clone(),
            ExternalSymbol::Value(field.ty.as_ref().map_or(Type::Unknown, Type::from_ref)),
        ),
        Member::Function(function) => (
            function.name.clone(),
            ExternalSymbol::Function(FunctionType {
                params: function.params.iter().map(|param| param.ty.as_ref().map_or(Type::Unknown, Type::from_ref)).collect(),
                return_type: function.return_type.as_ref().map_or(Type::Unit, Type::from_ref),
            }),
        ),
    }).collect()
}

fn completion_chain(before: &str) -> (Option<Vec<String>>, String) {
    let tail = before.chars().rev().take_while(|character| character.is_ascii_alphanumeric() || *character == '_' || *character == '.').collect::<String>().chars().rev().collect::<String>();
    if !tail.contains('.') {
        return (None, tail);
    }
    let mut parts = tail.split('.').map(str::to_owned).collect::<Vec<_>>();
    let prefix = parts.pop().unwrap_or_default();
    (Some(parts), prefix)
}

fn string_argument(before: &str, call: &str) -> Option<String> {
    let marker = format!("{call}(\"");
    let start = before.rfind(&marker)? + marker.len();
    let tail = &before[start..];
    (!tail.contains('"')).then(|| tail.to_owned())
}

fn completion_item(label: &str, kind: u8, detail: Option<&str>) -> Value {
    json!({"label":label,"kind":kind,"detail":detail})
}

fn symbol_completion(name: &str, symbol: &ExternalSymbol) -> Value {
    match symbol {
        ExternalSymbol::Value(ty) => json!({"label":name,"kind":6,"detail":ty.display_name()}),
        ExternalSymbol::Function(function) => json!({"label":name,"kind":3,"detail":signature(name, function),"insertText":format!("{name}($1)"),"insertTextFormat":2}),
    }
}

fn hover_symbol(name: &str, symbol: &ExternalSymbol) -> Value {
    let value = match symbol {
        ExternalSymbol::Value(ty) => format!("```decay\n{name}: {}\n```", ty.display_name()),
        ExternalSymbol::Function(function) => format!("```decay\n{}\n```", signature(name, function)),
    };
    json!({"contents":{"kind":"markdown","value":value}})
}

fn signature(name: &str, function: &FunctionType) -> String {
    let params = function.params.iter().map(Type::display_name).collect::<Vec<_>>().join(", ");
    format!("fn {name}({params}) -> {}", function.return_type.display_name())
}

fn initialization_root(params: &Value) -> Option<PathBuf> {
    params.get("rootPath").and_then(Value::as_str).map(PathBuf::from).or_else(|| {
        params.get("rootUri").and_then(Value::as_str).and_then(file_uri_to_path)
    })
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let value = uri.strip_prefix("file://")?;
    Some(PathBuf::from(percent_decode(value)))
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len()
            && let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16)
        {
            output.push(hex);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn span_range(source: &str, span: Span) -> Value {
    let (start_line, start_character) = position_at(source, span.start);
    let (end_line, end_character) = position_at(source, span.end.max(span.start + 1).min(source.len()));
    json!({
        "start":{"line":start_line,"character":start_character},
        "end":{"line":end_line,"character":end_character}
    })
}

fn position_at(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = source[line_start..offset].encode_utf16().count();
    (line, character)
}

fn offset_at(source: &str, line: usize, character: usize) -> usize {
    let mut offset = 0;
    for (index, part) in source.split_inclusive('\n').enumerate() {
        if index == line {
            let without_newline = part.strip_suffix('\n').unwrap_or(part);
            let mut utf16 = 0;
            for (byte, ch) in without_newline.char_indices() {
                if utf16 >= character {
                    return offset + byte;
                }
                utf16 += ch.len_utf16();
            }
            return offset + without_newline.len();
        }
        offset += part.len();
    }
    source.len()
}

fn word_at(source: &str, offset: usize) -> Option<&str> {
    let offset = offset.min(source.len());
    let start = source[..offset].rfind(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).map_or(0, |index| index + 1);
    let end = source[offset..].find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).map_or(source.len(), |index| offset + index);
    (start < end).then(|| &source[start..end])
}

fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
        if let Some(value) = header.strip_prefix("Content-Length:") {
            content_length = value.trim().parse::<usize>().ok();
        }
    }
    let Some(length) = content_length else {
        return Ok(None);
    };
    let mut body = vec![0; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(io::Error::other)
}

fn write_message(output: &mut impl Write, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(io::Error::other)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();
    let mut server = Server::new();
    while let Some(message) = read_message(&mut input)? {
        if !server.handle(message, &mut output)? {
            break;
        }
    }
    Ok(())
}
