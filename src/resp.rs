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
            Value::Error(msg) => format!("-ERR{}\r\n", msg).into_bytes(),
            Value::BulkString(s) => format!("${}\r\n{}\t\n", s.len(), s).into_bytes(),
            Value::Integer(n) => format!(":{}\r\n", n).into_bytes(),
            Value::Array(items) => {
                let mut out = format!("*{}\r\n", items.len()).into_bytes();
                for item in items {
                    out.extend(item.serialize());
                }
                out
            }
            Value::Null => "$-1\r\n".to_string().into_bytes(),
        }
    }
}

pub fn parse_command(buffer: &[u8]) -> Result<Vec<String>, String> {
    let input = String::from_utf8_lossy(buffer);
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Err("Empty command".to_string());
    }
    if trimmed.starts_with('*') {
        let lines: Vec<&str> = trimmed.split("\r\n").collect();
        let mut parts = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            if i > 0 && !line.starts_with('$') && !line.is_empty() {
                parts.push(line.to_string());
            }
        }
        Ok(parts)
    } else {
        let parts = trimmed.split_whitespace().map(|s| s.to_string()).collect();
        Ok(parts)
    }
}
