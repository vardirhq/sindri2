//! Writing a document back out the one way it is written.
//!
//! A canonical scene is a fixed point: reading one and writing it again
//! produces the same bytes, so a diff on a scene file is the edit and
//! nothing else. Scalar arrays are folded back onto one line, because
//! `serde_json`'s pretty printer puts every number of a transform on a
//! line of its own.

/// The column budget for keeping an array of scalars on one line.
pub(super) const INLINE_ARRAY_WIDTH: usize = 96;

/// Collapses arrays that contain only scalars onto a single line.
///
/// `serde_json` gives every array element its own line, which turns a
/// three-component position into five lines and buries real changes in review.
/// An array of scalars is unambiguous on one line, so it is collapsed whenever
/// the result stays inside [`INLINE_ARRAY_WIDTH`] columns. Arrays holding
/// objects or nested arrays, and scalar arrays too long to fit, keep the
/// expanded form. The decision depends only on the already deterministic
/// pretty output, so canonical serialization stays a fixed point.
pub(super) fn collapse_scalar_arrays(pretty: &str) -> String {
    let bytes = pretty.as_bytes();
    let mut output = String::with_capacity(pretty.len());
    let mut index = 0;
    let mut line_start = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                output.push('\n');
                index += 1;
                line_start = output.len();
            }
            b'"' => {
                let end = string_end(bytes, index);
                output.push_str(&pretty[index..end]);
                index = end;
            }
            b'[' => {
                let inlined = scalar_array_end(bytes, index)
                    .map(|end| (inline_array(&pretty[index..end]), end));
                match inlined {
                    Some((inline, end))
                        if output.len() - line_start + inline.len() < INLINE_ARRAY_WIDTH =>
                    {
                        output.push_str(&inline);
                        index = end;
                    }
                    _ => {
                        output.push('[');
                        index += 1;
                    }
                }
            }
            _ => {
                let character = pretty[index..]
                    .chars()
                    .next()
                    .expect("index stays on a character boundary");
                output.push(character);
                index += character.len_utf8();
            }
        }
    }
    output
}

/// Returns the index just past the closing quote of the string starting at `start`.
pub(super) fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

/// Returns the index just past the closing bracket when the array starting at
/// `start` holds only scalars, or `None` when it nests objects or arrays.
pub(super) fn scalar_array_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index),
            b']' => return Some(index + 1),
            b'[' | b'{' | b'}' => return None,
            _ => index += 1,
        }
    }
    None
}

/// Rewrites an already validated scalar array as a single line.
pub(super) fn inline_array(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    output.push('[');
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b']' => break,
            b',' => {
                output.push_str(", ");
                index += 1;
            }
            b'"' => {
                let end = string_end(bytes, index);
                output.push_str(&source[index..end]);
                index = end;
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            _ => {
                let character = source[index..]
                    .chars()
                    .next()
                    .expect("index stays on a character boundary");
                output.push(character);
                index += character.len_utf8();
            }
        }
    }
    output.push(']');
    output
}
