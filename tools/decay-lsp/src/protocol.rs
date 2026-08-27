use std::io::{self, BufRead, Write};

use serde_json::Value;

pub(crate) fn read_message(input: &mut impl BufRead) -> io::Result<Option<Value>> {
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
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(io::Error::other)
}

pub(crate) fn write_message(output: &mut impl Write, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value).map_err(io::Error::other)?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len())?;
    output.write_all(&body)?;
    output.flush()
}
