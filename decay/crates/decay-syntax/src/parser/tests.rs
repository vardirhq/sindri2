//! What the parser makes of a script, and what it says when it cannot.

use crate::ast::{AssignOp, ExprKind, Item, Member, Stmt};

use super::parse;

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
        r"
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
    ",
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
