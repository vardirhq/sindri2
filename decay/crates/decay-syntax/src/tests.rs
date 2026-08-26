//! What the lexer makes of a script.

use super::{TokenKind, lex};

#[test]
fn lexes_a_decay_gameplay_shape() {
    let source = r"
        script Player {
            @export
            let speed: f32 = 6.0;

            fn update(dt: f32) {
                this.transform.position.x += speed * dt;
            }
        }
    ";

    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Script)
    );
    assert!(lexed.tokens.iter().any(|token| token.kind == TokenKind::At));
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::PlusEqual)
    );
    assert!(
        lexed
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Number(6.0))
    );
}

#[test]
fn ignores_comments() {
    let lexed = lex("let speed = 6.0; // tuned in editor\nlet jump = 8.0;");
    assert!(lexed.diagnostics.is_empty());
    assert_eq!(
        lexed
            .tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Let)
            .count(),
        2
    );
}

#[test]
fn reports_line_and_column_for_bad_input() {
    let lexed = lex("let ok = 1;\nlet bad = \"never closes");
    assert_eq!(lexed.diagnostics.len(), 1);
    assert_eq!(lexed.diagnostics[0].line, 2);
    assert_eq!(lexed.diagnostics[0].column, 11);
    assert!(lexed.diagnostics[0].message.contains("unterminated"));
}
