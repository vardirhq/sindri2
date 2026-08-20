use crate::{Diagnostic, Span, Token, TokenKind, ast::*, lex, line_column};

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn parse(source: &str) -> Parsed {
    let lexed = lex(source);
    let mut parser = Parser::new(source, lexed.tokens, lexed.diagnostics);
    let program = parser.parse_program();
    Parsed {
        program,
        diagnostics: parser.diagnostics,
    }
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, tokens: Vec<Token>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            diagnostics,
        }
    }

    fn parse_program(&mut self) -> Program {
        let mut items = Vec::new();
        while !self.at(&TokenKind::Eof) {
            let item = if self.at(&TokenKind::Script) {
                self.parse_container(true)
            } else if self.at(&TokenKind::Component) {
                self.parse_container(false)
            } else {
                self.error_here("expected `script` or `component`");
                self.advance();
                None
            };
            if let Some(item) = item {
                items.push(item);
            }
        }
        Program { items }
    }

    fn parse_container(&mut self, script: bool) -> Option<Item> {
        let start = self.current().span;
        self.advance();
        let (name, _) = self.expect_identifier("expected a name after declaration keyword")?;
        self.expect_simple(TokenKind::LeftBrace, "expected `{` after declaration name")?;

        let mut members = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            let attributes = self.parse_attributes();
            if self.at(&TokenKind::Fn) {
                if !attributes.is_empty() {
                    self.error_span(
                        attributes[0].span,
                        "attributes on functions are not supported yet",
                    );
                }
                if let Some(function) = self.parse_function() {
                    members.push(Member::Function(function));
                } else {
                    self.synchronize_member();
                }
            } else if self.at(&TokenKind::Let) || self.at(&TokenKind::Var) {
                if let Some(field) = self.parse_field(attributes) {
                    members.push(Member::Field(field));
                } else {
                    self.synchronize_member();
                }
            } else {
                self.error_here("expected a field or function in declaration body");
                self.synchronize_member();
            }
        }

        let end = self
            .expect_simple(TokenKind::RightBrace, "expected `}` after declaration body")
            .unwrap_or_else(|| self.current().span);
        let decl = ContainerDecl {
            name,
            members,
            span: start.join(end),
        };
        Some(if script {
            Item::Script(decl)
        } else {
            Item::Component(decl)
        })
    }

    fn parse_attributes(&mut self) -> Vec<Attribute> {
        let mut attributes = Vec::new();
        while self.at(&TokenKind::At) {
            let start = self.current().span;
            self.advance();
            if let Some((name, name_span)) =
                self.expect_identifier("expected attribute name after `@`")
            {
                attributes.push(Attribute {
                    name,
                    span: start.join(name_span),
                });
            } else {
                break;
            }
        }
        attributes
    }

    fn parse_field(&mut self, attributes: Vec<Attribute>) -> Option<FieldDecl> {
        let start = self.current().span;
        let mutable = self.at(&TokenKind::Var);
        self.advance();
        let (name, _) = self.expect_identifier("expected field name")?;
        let ty = self.parse_optional_type();
        let initializer = if self.consume_simple(&TokenKind::Equal).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let end = self.expect_simple(TokenKind::Semicolon, "expected `;` after field")?;
        Some(FieldDecl {
            attributes,
            mutable,
            name,
            ty,
            initializer,
            span: start.join(end),
        })
    }

    fn parse_function(&mut self) -> Option<FunctionDecl> {
        let start = self.expect_simple(TokenKind::Fn, "expected `fn`")?;
        let (name, _) = self.expect_identifier("expected function name")?;
        self.expect_simple(TokenKind::LeftParen, "expected `(` after function name")?;
        let mut params = Vec::new();
        if !self.at(&TokenKind::RightParen) {
            loop {
                let (name, name_span) = self.expect_identifier("expected parameter name")?;
                let ty = self.parse_optional_type();
                let end = ty.as_ref().map_or(name_span, |ty| ty.span);
                params.push(Param {
                    name,
                    ty,
                    span: name_span.join(end),
                });
                if self.consume_simple(&TokenKind::Comma).is_none() {
                    break;
                }
            }
        }
        self.expect_simple(TokenKind::RightParen, "expected `)` after parameters")?;
        let return_type = if self.consume_simple(&TokenKind::Arrow).is_some() {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        let span = start.join(body.span);
        Some(FunctionDecl {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    fn parse_optional_type(&mut self) -> Option<TypeRef> {
        if self.consume_simple(&TokenKind::Colon).is_some() {
            self.parse_type()
        } else {
            None
        }
    }

    fn parse_type(&mut self) -> Option<TypeRef> {
        self.expect_identifier("expected type name")
            .map(|(name, span)| TypeRef { name, span })
    }

    fn parse_block(&mut self) -> Option<Block> {
        let start = self.expect_simple(TokenKind::LeftBrace, "expected `{`")?;
        let mut statements = Vec::new();
        while !self.at(&TokenKind::RightBrace) && !self.at(&TokenKind::Eof) {
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            } else {
                self.synchronize_statement();
            }
        }
        let end = self.expect_simple(TokenKind::RightBrace, "expected `}` after block")?;
        Some(Block {
            statements,
            span: start.join(end),
        })
    }

    fn parse_statement(&mut self) -> Option<Stmt> {
        if self.at(&TokenKind::Let) || self.at(&TokenKind::Var) {
            return self.parse_binding_statement();
        }
        if self.at(&TokenKind::Return) {
            return self.parse_return_statement();
        }
        if self.at(&TokenKind::If) {
            return self.parse_if_statement();
        }
        if self.at(&TokenKind::LeftBrace) {
            return self.parse_block().map(Stmt::Block);
        }

        let expr = self.parse_expression()?;
        let end = self.expect_simple(TokenKind::Semicolon, "expected `;` after expression")?;
        let span = expr.span.join(end);
        Some(Stmt::Expr { expr, span })
    }

    fn parse_binding_statement(&mut self) -> Option<Stmt> {
        let start = self.current().span;
        let mutable = self.at(&TokenKind::Var);
        self.advance();
        let (name, _) = self.expect_identifier("expected variable name")?;
        let ty = self.parse_optional_type();
        let initializer = if self.consume_simple(&TokenKind::Equal).is_some() {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let end = self.expect_simple(TokenKind::Semicolon, "expected `;` after binding")?;
        Some(Stmt::Binding {
            mutable,
            name,
            ty,
            initializer,
            span: start.join(end),
        })
    }

    fn parse_return_statement(&mut self) -> Option<Stmt> {
        let start = self.expect_simple(TokenKind::Return, "expected `return`")?;
        let value = if self.at(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        let end = self.expect_simple(TokenKind::Semicolon, "expected `;` after return")?;
        Some(Stmt::Return {
            value,
            span: start.join(end),
        })
    }

    fn parse_if_statement(&mut self) -> Option<Stmt> {
        let start = self.expect_simple(TokenKind::If, "expected `if`")?;
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;
        let mut end = then_branch.span;
        let else_branch = if self.consume_simple(&TokenKind::Else).is_some() {
            let block = self.parse_block()?;
            end = block.span;
            Some(block)
        } else {
            None
        };
        Some(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: start.join(end),
        })
    }

    fn parse_expression(&mut self) -> Option<Expr> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Option<Expr> {
        let target = self.parse_binary(0)?;
        let op = match self.current().kind {
            TokenKind::Equal => AssignOp::Assign,
            TokenKind::PlusEqual => AssignOp::Add,
            TokenKind::MinusEqual => AssignOp::Subtract,
            TokenKind::StarEqual => AssignOp::Multiply,
            TokenKind::SlashEqual => AssignOp::Divide,
            _ => return Some(target),
        };
        self.advance();
        let value = self.parse_assignment()?;
        let span = target.span.join(value.span);
        Some(Expr {
            kind: ExprKind::Assign {
                target: Box::new(target),
                op,
                value: Box::new(value),
            },
            span,
        })
    }

    fn parse_binary(&mut self, min_precedence: u8) -> Option<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let Some((precedence, op)) = binary_operator(&self.current().kind) else {
                break;
            };
            if precedence < min_precedence {
                break;
            }
            self.advance();
            let right = self.parse_binary(precedence + 1)?;
            let span = left.span.join(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
                span,
            };
        }
        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        let op = match self.current().kind {
            TokenKind::Minus => Some(UnaryOp::Negate),
            TokenKind::Bang => Some(UnaryOp::Not),
            _ => None,
        };
        if let Some(op) = op {
            let start = self.current().span;
            self.advance();
            let expr = self.parse_unary()?;
            let span = start.join(expr.span);
            return Some(Expr {
                kind: ExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                },
                span,
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.consume_simple(&TokenKind::Dot).is_some() {
                let (field, field_span) =
                    self.expect_identifier("expected member name after `.`")?;
                let span = expr.span.join(field_span);
                expr = Expr {
                    kind: ExprKind::Member {
                        object: Box::new(expr),
                        field,
                    },
                    span,
                };
                continue;
            }
            if self.consume_simple(&TokenKind::LeftParen).is_some() {
                let mut args = Vec::new();
                if !self.at(&TokenKind::RightParen) {
                    loop {
                        args.push(self.parse_expression()?);
                        if self.consume_simple(&TokenKind::Comma).is_none() {
                            break;
                        }
                    }
                }
                let end =
                    self.expect_simple(TokenKind::RightParen, "expected `)` after arguments")?;
                let span = expr.span.join(end);
                expr = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(expr),
                        args,
                    },
                    span,
                };
                continue;
            }
            break;
        }
        Some(expr)
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        let token = self.current().clone();
        let kind = match token.kind {
            TokenKind::Identifier(name) => {
                self.advance();
                ExprKind::Identifier(name)
            }
            TokenKind::Number(value) => {
                self.advance();
                ExprKind::Number(value)
            }
            TokenKind::String(value) => {
                self.advance();
                ExprKind::String(value)
            }
            TokenKind::True => {
                self.advance();
                ExprKind::Bool(true)
            }
            TokenKind::False => {
                self.advance();
                ExprKind::Bool(false)
            }
            TokenKind::Null => {
                self.advance();
                ExprKind::Null
            }
            TokenKind::LeftParen => {
                self.advance();
                let inner = self.parse_expression()?;
                let end =
                    self.expect_simple(TokenKind::RightParen, "expected `)` after expression")?;
                return Some(Expr {
                    span: token.span.join(end),
                    kind: ExprKind::Group(Box::new(inner)),
                });
            }
            _ => {
                self.error_here("expected expression");
                return None;
            }
        };
        Some(Expr {
            kind,
            span: token.span,
        })
    }

    fn synchronize_member(&mut self) {
        while !self.at(&TokenKind::Eof) && !self.at(&TokenKind::RightBrace) {
            if self.at(&TokenKind::Fn)
                || self.at(&TokenKind::Let)
                || self.at(&TokenKind::Var)
                || self.at(&TokenKind::At)
            {
                break;
            }
            self.advance();
        }
    }

    fn synchronize_statement(&mut self) {
        while !self.at(&TokenKind::Eof) && !self.at(&TokenKind::RightBrace) {
            if self.consume_simple(&TokenKind::Semicolon).is_some() {
                break;
            }
            self.advance();
        }
    }

    fn expect_identifier(&mut self, message: &str) -> Option<(String, Span)> {
        let token = self.current().clone();
        if let TokenKind::Identifier(name) = token.kind {
            self.advance();
            Some((name, token.span))
        } else {
            self.error_here(message);
            None
        }
    }

    fn expect_simple(&mut self, expected: TokenKind, message: &str) -> Option<Span> {
        if self.at(&expected) {
            let span = self.current().span;
            self.advance();
            Some(span)
        } else {
            self.error_here(message);
            None
        }
    }

    fn consume_simple(&mut self, expected: &TokenKind) -> Option<Span> {
        if self.at(expected) {
            let span = self.current().span;
            self.advance();
            Some(span)
        } else {
            None
        }
    }

    fn at(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(expected)
    }

    fn current(&self) -> &Token {
        &self.tokens[self.cursor.min(self.tokens.len().saturating_sub(1))]
    }

    fn advance(&mut self) {
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
    }

    fn error_here(&mut self, message: &str) {
        self.error_span(self.current().span, message);
    }

    fn error_span(&mut self, span: Span, message: &str) {
        let (line, column) = line_column(self.source, span.start);
        self.diagnostics.push(Diagnostic {
            message: message.to_owned(),
            span,
            line,
            column,
        });
    }
}

fn binary_operator(kind: &TokenKind) -> Option<(u8, BinaryOp)> {
    match kind {
        TokenKind::OrOr => Some((1, BinaryOp::Or)),
        TokenKind::AndAnd => Some((2, BinaryOp::And)),
        TokenKind::EqualEqual => Some((3, BinaryOp::Equal)),
        TokenKind::BangEqual => Some((3, BinaryOp::NotEqual)),
        TokenKind::Less => Some((4, BinaryOp::Less)),
        TokenKind::LessEqual => Some((4, BinaryOp::LessEqual)),
        TokenKind::Greater => Some((4, BinaryOp::Greater)),
        TokenKind::GreaterEqual => Some((4, BinaryOp::GreaterEqual)),
        TokenKind::Plus => Some((5, BinaryOp::Add)),
        TokenKind::Minus => Some((5, BinaryOp::Subtract)),
        TokenKind::Star => Some((6, BinaryOp::Multiply)),
        TokenKind::Slash => Some((6, BinaryOp::Divide)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::ast::{AssignOp, ExprKind, Item, Member, Stmt};

    #[test]
    fn parses_script_fields_functions_and_member_assignment() {
        let parsed = parse(
            r#"
            script PlayerController {
                @export
                let speed: f32 = 6.0;

                fn update(dt: f32) {
                    let movement = Input.axis("move_left", "move_right");
                    this.transform.position.x += movement * speed * dt;
                }
            }
        "#,
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let Item::Script(script) = &parsed.program.items[0] else {
            panic!("expected script");
        };
        assert_eq!(script.name, "PlayerController");
        let Member::Field(field) = &script.members[0] else {
            panic!("expected field");
        };
        assert_eq!(field.attributes[0].name, "export");
        assert_eq!(field.ty.as_ref().unwrap().name, "f32");
        let Member::Function(function) = &script.members[1] else {
            panic!("expected function");
        };
        assert_eq!(function.name, "update");
        assert_eq!(function.params[0].name, "dt");
        let Stmt::Expr { expr, .. } = &function.body.statements[1] else {
            panic!("expected expression statement");
        };
        let ExprKind::Assign { op, .. } = &expr.kind else {
            panic!("expected assignment");
        };
        assert_eq!(*op, AssignOp::Add);
    }

    #[test]
    fn observes_operator_precedence() {
        let parsed = parse("script Test { fn update() { let value = 1.0 + 2.0 * 3.0; } }");
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let Item::Script(script) = &parsed.program.items[0] else {
            panic!()
        };
        let Member::Function(function) = &script.members[0] else {
            panic!()
        };
        let Stmt::Binding {
            initializer: Some(expr),
            ..
        } = &function.body.statements[0]
        else {
            panic!()
        };
        let ExprKind::Binary { right, .. } = &expr.kind else {
            panic!()
        };
        assert!(matches!(right.kind, ExprKind::Binary { .. }));
    }

    #[test]
    fn parses_component_and_control_flow() {
        let parsed = parse(
            r#"
            component Health {
                @export
                var current: f32 = 100.0;

                fn damage(amount: f32) {
                    current -= amount;
                    if current <= 0.0 {
                        return;
                    }
                }
            }
        "#,
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert!(matches!(parsed.program.items[0], Item::Component(_)));
    }

    #[test]
    fn parser_reports_missing_semicolon() {
        let parsed = parse("script Test { let speed: f32 = 4.0 }");
        assert_eq!(parsed.diagnostics.len(), 1);
        assert!(parsed.diagnostics[0].message.contains("`;`"));
    }
}
