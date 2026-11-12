#[derive(Debug, PartialEq, Clone)]
pub enum Value {
    SimpleString(String),
    Error(String),
    BulkString(String),
    Array(Vec<Value>),
    Integer(i64),
    Null,
}

impl Value {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Value::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
            Value::Error(msg) => format!("-ERR {}\r\n", msg).into_bytes(),
            Value::BulkString(s) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),
            Value::Integer(n) => format!(":{}\r\n", n).into_bytes(),
            Value::Array(items) => {
                let mut out = format!("*{}\r\n", items.len()).into_bytes();
                for item in items {
                    out.extend(item.serialize());
                }
                out
            }
            Value::Null => b"$-1\r\n".to_vec(),
        }
    }
}

#[allow(dead_code)]
pub fn parse_command(buffer: &[u8]) -> Result<Vec<String>, String> {
    let input = String::from_utf8_lossy(buffer);
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err("Empty command".to_string());
    }

    if trimmed.starts_with('*') {
        match try_parse_resp_array(buffer)? {
            Some((args, _)) => Ok(args),
            None => Err("Incomplete RESP command".to_string()),
        }
    } else {
        let args = parse_inline_args(trimmed);
        if args.is_empty() {
            Err("Empty command".to_string())
        } else {
            Ok(args)
        }
    }
}

pub fn try_parse_command(buffer: &[u8]) -> Result<Option<(Vec<String>, usize)>, String> {
    if buffer.is_empty() {
        return Ok(None);
    }

    if buffer[0] == b'*' {
        try_parse_resp_array(buffer)
    } else {
        let Some(nl_pos) = buffer.iter().position(|&b| b == b'\n') else {
            return Ok(None);
        };
        let line = String::from_utf8_lossy(&buffer[..nl_pos]);
        let args = parse_inline_args(line.trim_end_matches('\r'));
        Ok(Some((args, nl_pos + 1)))
    }
}

fn find_crlf(buf: &[u8], start: usize) -> Option<usize> {
    buf[start..]
        .windows(2)
        .position(|w| w == b"\r\n")
        .map(|rel| start + rel)
}

fn try_parse_resp_array(buffer: &[u8]) -> Result<Option<(Vec<String>, usize)>, String> {
    let Some(header_end) = find_crlf(buffer, 0) else {
        return Ok(None);
    };

    let count_str = std::str::from_utf8(&buffer[1..header_end])
        .map_err(|_| "Invalid UTF-8 in RESP array length".to_string())?
        .trim();
    let expected_count: usize = count_str
        .parse()
        .map_err(|_| format!("Invalid RESP array length: {}", count_str))?;

    let mut pos = header_end + 2;
    let mut parts = Vec::with_capacity(expected_count);

    for _ in 0..expected_count {
        if pos >= buffer.len() {
            return Ok(None);
        }
        if buffer[pos] != b'$' {
            return Err("Expected RESP bulk string prefix '$'".to_string());
        }

        let Some(len_end) = find_crlf(buffer, pos) else {
            return Ok(None);
        };
        let len_str = std::str::from_utf8(&buffer[pos + 1..len_end])
            .map_err(|_| "Invalid UTF-8 in RESP bulk length".to_string())?
            .trim();
        let byte_len: usize = len_str
            .parse()
            .map_err(|_| format!("Invalid RESP bulk string length: {}", len_str))?;

        pos = len_end + 2;
        if buffer.len() < pos + byte_len + 2 {
            return Ok(None);
        }

        let payload = String::from_utf8_lossy(&buffer[pos..pos + byte_len]).to_string();
        if &buffer[pos + byte_len..pos + byte_len + 2] != b"\r\n" {
            return Err("Missing CRLF after RESP bulk string".to_string());
        }

        parts.push(payload);
        pos += byte_len + 2;
    }

    Ok(Some((parts, pos)))
}

pub fn parse_inline_args(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';
    let mut escaped = false;
    let mut token_started = false;

    for ch in line.trim().chars() {
        if escaped {
            let unescaped = match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            };
            current.push(unescaped);
            escaped = false;
            token_started = true;
            continue;
        }

        if in_quotes {
            if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                in_quotes = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
            token_started = true;
        } else if ch.is_whitespace() {
            if token_started {
                args.push(std::mem::take(&mut current));
                token_started = false;
            }
        } else {
            current.push(ch);
            token_started = true;
        }
    }

    if token_started {
        args.push(current);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_resp_frames_with_valid_crlf() {
        assert_eq!(Value::SimpleString("OK".into()).serialize(), b"+OK\r\n");
        assert_eq!(
            Value::Error("unknown command 'FOO'".into()).serialize(),
            b"-ERR unknown command 'FOO'\r\n"
        );
        assert_eq!(
            Value::BulkString("yudha".into()).serialize(),
            b"$5\r\nyudha\r\n"
        );
        assert_eq!(Value::Integer(42).serialize(), b":42\r\n");
        assert_eq!(Value::Null.serialize(), b"$-1\r\n");
        assert_eq!(
            Value::Array(vec![Value::BulkString("a".into()), Value::Null]).serialize(),
            b"*2\r\n$1\r\na\r\n$-1\r\n"
        );
    }

    #[test]
    fn parses_resp_array_including_dollar_prefixed_values() {
        let raw = b"*3\r\n$3\r\nSET\r\n$5\r\nprice\r\n$4\r\n$100\r\n";
        let (args, consumed) = try_parse_command(raw).unwrap().unwrap();
        assert_eq!(args, vec!["SET", "price", "$100"]);
        assert_eq!(consumed, raw.len());
    }

    #[test]
    fn handles_partial_and_pipelined_resp_frames() {
        let partial = b"*2\r\n$3\r\nGET\r\n$3\r\nke";
        assert!(try_parse_command(partial).unwrap().is_none());

        let pipelined = b"*1\r\n$4\r\nPING\r\n*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n";
        let (first, used) = try_parse_command(pipelined).unwrap().unwrap();
        assert_eq!(first, vec!["PING"]);
        let (second, used2) = try_parse_command(&pipelined[used..]).unwrap().unwrap();
        assert_eq!(second, vec!["GET", "key"]);
        assert_eq!(used + used2, pipelined.len());
    }

    #[test]
    fn parses_inline_quoted_arguments() {
        let raw = b"SET greeting \"hello world\"\r\n";
        let (args, _) = try_parse_command(raw).unwrap().unwrap();
        assert_eq!(args, vec!["SET", "greeting", "hello world"]);
    }
}
