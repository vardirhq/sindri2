//! Collections: the one value that holds several things.
//!
//! There is no literal for one and no way to build one, so every test here
//! gets its collection from a host — which is the whole shape of the feature.
//! A script reads what the host handed it, walks it, indexes it, and asks how
//! long it is; it cannot grow one, shrink one, or write into one.

use decay_ir::lower_with_environment;
use decay_semantic::{Environment, FunctionType, HostType, Type};

use crate::{Host, Path, Runtime, RuntimeError, Value};

/// A host offering `Group.of(count)`, a collection of the numbers below
/// `count`, and `Group.names()`, a collection of text.
struct GroupHost;

impl Host for GroupHost {
    fn load(&mut self, _subject: Option<u64>, _path: &Path) -> Result<Option<Value>, RuntimeError> {
        Ok(None)
    }

    fn store(
        &mut self,
        _subject: Option<u64>,
        _path: &Path,
        _value: Value,
    ) -> Result<bool, RuntimeError> {
        Ok(false)
    }

    fn call(
        &mut self,
        _subject: Option<u64>,
        path: &Path,
        args: &[Value],
    ) -> Result<Option<Value>, RuntimeError> {
        match path.dotted().as_str() {
            "Group.of" => {
                let Some(Value::Number(count)) = args.first() else {
                    return Ok(None);
                };
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let count = *count as usize;
                Ok(Some(Value::array(
                    #[allow(clippy::cast_precision_loss)]
                    (0..count).map(|n| Value::Number(n as f64)).collect(),
                )))
            }
            "Group.names" => Ok(Some(Value::array(vec![
                Value::String("first".to_owned()),
                Value::String("second".to_owned()),
            ]))),
            _ => Ok(None),
        }
    }
}

fn environment() -> Environment {
    let mut environment = Environment::new();
    let group = HostType::new()
        .with_function(
            "of",
            FunctionType {
                params: vec![Type::F32],
                return_type: Type::array_of(Type::F32),
            },
        )
        .with_function(
            "names",
            FunctionType {
                params: Vec::new(),
                return_type: Type::array_of(Type::String),
            },
        );
    environment.add_type("Group", group);
    environment.add_value("Group", Type::Named("Group".to_owned()));
    environment
}

fn run(source: &str, function: &str, args: Vec<Value>) -> Result<Value, RuntimeError> {
    let lowered = lower_with_environment(source, &environment());
    let program = lowered
        .program
        .unwrap_or_else(|| panic!("{:?}", lowered.analysis.diagnostics));
    let mut runtime = Runtime::new(&program, GroupHost);
    runtime.call("Collections", function, args)
}

/// The diagnostics a source produces, for the cases that should not compile.
fn refuse(source: &str) -> Vec<String> {
    lower_with_environment(source, &environment())
        .analysis
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn a_collection_says_how_many_it_holds() {
    assert_eq!(
        run(
            r"script Collections {
                fn count() -> f32 {
                    let items: Array<f32> = Group.of(4.0);
                    return items.len;
                }
            }",
            "count",
            Vec::new(),
        ),
        Ok(Value::Number(4.0))
    );
}

#[test]
fn an_element_is_reached_by_index() {
    assert_eq!(
        run(
            r"script Collections {
                fn third() -> f32 {
                    let items: Array<f32> = Group.of(5.0);
                    return items[2.0];
                }
            }",
            "third",
            Vec::new(),
        ),
        Ok(Value::Number(2.0))
    );
}

#[test]
fn a_for_loop_walks_every_element() {
    assert_eq!(
        run(
            r"script Collections {
                fn sum() -> f32 {
                    var total: f32 = 0.0;
                    for value in Group.of(5.0) {
                        total += value;
                    }
                    return total;
                }
            }",
            "sum",
            Vec::new(),
        ),
        Ok(Value::Number(10.0))
    );
}

#[test]
fn break_and_continue_work_inside_a_walk() {
    assert_eq!(
        run(
            r"script Collections {
                fn sum() -> f32 {
                    var total: f32 = 0.0;
                    for value in Group.of(10.0) {
                        if value == 3.0 {
                            continue;
                        }
                        if value == 6.0 {
                            break;
                        }
                        total += value;
                    }
                    return total;
                }
            }",
            "sum",
            Vec::new(),
        ),
        // 0 + 1 + 2 + 4 + 5, with 3 skipped and 6 ending it.
        Ok(Value::Number(12.0))
    );
}

/// A `break` out of a walk has to let go of what it was walking, and the loop
/// around it has to be unaffected. This is the test that would fail if the
/// walk lived on the value stack and a break left it there.
#[test]
fn a_walk_inside_a_walk_leaves_the_outer_one_intact() {
    assert_eq!(
        run(
            r"script Collections {
                fn sum() -> f32 {
                    var total: f32 = 0.0;
                    for outer in Group.of(4.0) {
                        for inner in Group.of(4.0) {
                            if inner == 2.0 {
                                break;
                            }
                            total += 1.0;
                        }
                        total += outer;
                    }
                    return total;
                }
            }",
            "sum",
            Vec::new(),
        ),
        // Two inner steps per outer turn, four turns, plus 0 + 1 + 2 + 3.
        Ok(Value::Number(14.0))
    );
}

#[test]
fn a_collection_of_text_walks_the_same_way() {
    assert_eq!(
        run(
            r"script Collections {
                fn second() -> String {
                    let names: Array<String> = Group.names();
                    return names[1.0];
                }
            }",
            "second",
            Vec::new(),
        ),
        Ok(Value::String("second".to_owned()))
    );
}

#[test]
fn an_index_past_the_end_names_the_length_it_was_measured_against() {
    assert_eq!(
        run(
            r"script Collections {
                fn missing() -> f32 {
                    let items: Array<f32> = Group.of(3.0);
                    return items[7.0];
                }
            }",
            "missing",
            Vec::new(),
        ),
        Err(RuntimeError::IndexOutOfRange {
            index: 7,
            length: 3
        })
    );
}

/// There is no integer type, so "a whole number" is a property of the value.
/// A calculation that should have rounded is a different mistake from a loop
/// bound that is wrong, and each says so.
#[test]
fn a_fractional_index_is_refused_rather_than_rounded() {
    assert_eq!(
        run(
            r"script Collections {
                fn between() -> f32 {
                    let items: Array<f32> = Group.of(3.0);
                    return items[1.5];
                }
            }",
            "between",
            Vec::new(),
        ),
        Err(RuntimeError::IndexNotWhole(1.5))
    );
}

#[test]
fn a_negative_index_is_not_counted_from_the_end() {
    assert_eq!(
        run(
            r"script Collections {
                fn last() -> f32 {
                    let items: Array<f32> = Group.of(3.0);
                    return items[-1.0];
                }
            }",
            "last",
            Vec::new(),
        ),
        Err(RuntimeError::IndexNotWhole(-1.0))
    );
}

#[test]
fn indexing_something_that_holds_one_value_does_not_compile() {
    let messages = refuse(
        r"script Collections {
            fn wrong() -> f32 {
                let count: f32 = 3.0;
                return count[0.0];
            }
        }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot be indexed")),
        "{messages:?}"
    );
}

#[test]
fn walking_something_that_holds_one_value_does_not_compile() {
    let messages = refuse(
        r"script Collections {
            fn wrong() {
                for value in 3.0 {
                }
            }
        }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("needs something to walk")),
        "{messages:?}"
    );
}

#[test]
fn an_element_cannot_be_assigned_to() {
    let messages = refuse(
        r"script Collections {
            fn wrong() {
                for value in Group.of(3.0) {
                    value = 1.0;
                }
            }
        }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot assign to immutable")),
        "{messages:?}"
    );
}

#[test]
fn the_element_type_is_checked_rather_than_assumed() {
    let messages = refuse(
        r"script Collections {
            fn wrong() -> f32 {
                let names: Array<String> = Group.names();
                return names[0.0];
            }
        }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot assign `String` to `f32`")),
        "{messages:?}"
    );
}

#[test]
fn a_collection_type_written_without_an_element_type_is_refused() {
    let messages = refuse(
        r"script Collections {
            fn wrong() {
                let items: Array = Group.of(1.0);
            }
        }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("needs an element type")),
        "{messages:?}"
    );
}

#[test]
fn only_a_collection_takes_a_type_argument() {
    let messages = refuse(
        r"script Collections {
            fn wrong() {
                let count: f32<String> = 1.0;
            }
        }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("takes no type argument")),
        "{messages:?}"
    );
}

/// The name a collection's length is spelled with is the language's, not a
/// global function's, so it costs no script the use of `len` as a name.
#[test]
fn len_stays_available_as_an_ordinary_name() {
    assert_eq!(
        run(
            r"script Collections {
                fn count() -> f32 {
                    let len: f32 = 2.0;
                    let items: Array<f32> = Group.of(4.0);
                    return items.len + len;
                }
            }",
            "count",
            Vec::new(),
        ),
        Ok(Value::Number(6.0))
    );
}

/// A walk holds the collection it was given, not the name it came from.
#[test]
fn reassigning_the_name_a_walk_came_from_does_not_change_the_walk() {
    assert_eq!(
        run(
            r"script Collections {
                fn sum() -> f32 {
                    var items: Array<f32> = Group.of(4.0);
                    var total: f32 = 0.0;
                    for value in items {
                        items = Group.of(1.0);
                        total += value;
                    }
                    return total;
                }
            }",
            "sum",
            Vec::new(),
        ),
        Ok(Value::Number(6.0))
    );
}
