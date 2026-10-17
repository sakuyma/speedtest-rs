//! Minimal ordered JSON value, parser and writer.
//!
//! Floating point values are always emitted with exactly three decimal places,
//! matching the CLI's machine-readable output.

use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn object() -> Value {
        Value::Object(Vec::new())
    }
    pub fn string(s: impl Into<String>) -> Value {
        Value::Str(s.into())
    }
    pub fn int(i: i64) -> Value {
        Value::Int(i)
    }
    pub fn double(d: f64) -> Value {
        Value::Double(d)
    }
    pub fn bool(b: bool) -> Value {
        Value::Bool(b)
    }

    /// Insert or replace a key (preserving position for existing keys).
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        let key = key.into();
        if let Value::Object(map) = self {
            for kv in map.iter_mut() {
                if kv.0 == key {
                    kv.1 = value;
                    return;
                }
            }
            map.push((key, value));
        }
    }

    pub fn find(&self, key: &str) -> Option<&Value> {
        if let Value::Object(map) = self {
            for kv in map {
                if kv.0 == key {
                    return Some(&kv.1);
                }
            }
        }
        None
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_string(&self) -> String {
        self.as_str().unwrap_or("").to_string()
    }
    pub fn get_string(&self, key: &str, default: &str) -> String {
        self.find(key).and_then(|v| v.as_str()).unwrap_or(default).to_string()
    }
    pub fn as_i64(&self) -> i64 {
        match self {
            Value::Int(i) => *i,
            Value::Double(d) => *d as i64,
            Value::Str(s) => s.trim().parse().unwrap_or(0),
            Value::Bool(b) => *b as i64,
            _ => 0,
        }
    }
    #[allow(dead_code)]
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Int(i) => *i as f64,
            Value::Double(d) => *d,
            Value::Str(s) => s.trim().parse().unwrap_or(0.0),
            Value::Bool(b) => *b as i64 as f64,
            _ => 0.0,
        }
    }
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(i) => *i != 0,
            Value::Str(s) => s == "true" || s == "1",
            _ => false,
        }
    }
    pub fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.find(key).map(|v| v.as_i64()).unwrap_or(default)
    }
    #[allow(dead_code)]
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        self.find(key).map(|v| v.as_bool()).unwrap_or(default)
    }
    pub fn is_string(&self) -> bool {
        matches!(self, Value::Str(_))
    }
    pub fn is_array(&self) -> bool {
        matches!(self, Value::Array(_))
    }
    #[allow(dead_code)]
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }
    pub fn as_array(&self) -> &[Value] {
        match self {
            Value::Array(a) => a,
            _ => &[],
        }
    }
    pub fn as_object(&self) -> &[(String, Value)] {
        match self {
            Value::Object(o) => o,
            _ => &[],
        }
    }

    pub fn dump(&self, pretty: bool) -> String {
        let mut out = String::new();
        self.write_to(&mut out, pretty, 0);
        out
    }

    fn write_to(&self, out: &mut String, pretty: bool, indent: usize) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Value::Double(d) => {
                if d.is_finite() {
                    let _ = write!(out, "{d:.3}");
                } else {
                    out.push_str("null");
                }
            }
            Value::Str(s) => escape_to(out, s),
            Value::Array(a) => {
                if a.is_empty() {
                    out.push_str("[]");
                    return;
                }
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                        push_indent(out, indent + 1);
                    }
                    v.write_to(out, pretty, indent + 1);
                }
                if pretty {
                    out.push('\n');
                    push_indent(out, indent);
                }
                out.push(']');
            }
            Value::Object(o) => {
                if o.is_empty() {
                    out.push_str("{}");
                    return;
                }
                out.push('{');
                for (i, (k, v)) in o.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    if pretty {
                        out.push('\n');
                        push_indent(out, indent + 1);
                    }
                    escape_to(out, k);
                    out.push_str(if pretty { ": " } else { ":" });
                    v.write_to(out, pretty, indent + 1);
                }
                if pretty {
                    out.push('\n');
                    push_indent(out, indent);
                }
                out.push('}');
            }
        }
    }
}

fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("    ");
    }
}

fn escape_to(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ---- parser ----------------------------------------------------------------

pub fn parse(input: &str) -> Result<Value, String> {
    let mut p = Parser { bytes: input.as_bytes(), pos: 0 };
    p.skip_ws();
    let v = p.parse_value()?;
    p.skip_ws();
    if p.pos != p.bytes.len() {
        return Err(format!("trailing characters at byte {}", p.pos));
    }
    Ok(v)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::Str(self.parse_string()?)),
            Some(b't') | Some(b'f') => self.parse_bool(),
            Some(b'n') => self.parse_null(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            other => Err(format!("unexpected byte {other:?} at {}", self.pos)),
        }
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        self.pos += 1; // {
        let mut map = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Object(map));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(format!("expected string key at {}", self.pos));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("expected ':' at {}", self.pos));
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.parse_value()?;
            map.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(format!("expected ',' or '}}' at {}", self.pos)),
            }
        }
        Ok(Value::Object(map))
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        self.pos += 1; // [
        let mut arr = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(arr));
        }
        loop {
            self.skip_ws();
            arr.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(format!("expected ',' or ']' at {}", self.pos)),
            }
        }
        Ok(Value::Array(arr))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.pos += 1; // opening quote
        let mut s = String::new();
        loop {
            let c = self.peek().ok_or("unterminated string")?;
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek().ok_or("unterminated escape")?;
                    self.pos += 1;
                    match e {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{08}'),
                        b'f' => s.push('\u{0c}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let cp = self.parse_hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // surrogate pair
                                if self.peek() == Some(b'\\') {
                                    self.pos += 1;
                                    if self.peek() == Some(b'u') {
                                        self.pos += 1;
                                        let lo = self.parse_hex4()?;
                                        let combined =
                                            0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                        if let Some(ch) = char::from_u32(combined) {
                                            s.push(ch);
                                        }
                                    }
                                }
                            } else if let Some(ch) = char::from_u32(cp) {
                                s.push(ch);
                            }
                        }
                        _ => return Err(format!("bad escape \\{}", e as char)),
                    }
                }
                _ => {
                    // Decode a UTF-8 sequence starting at pos-1.
                    let start = self.pos - 1;
                    let len = utf8_len(c);
                    let end = (start + len).min(self.bytes.len());
                    match std::str::from_utf8(&self.bytes[start..end]) {
                        Ok(sub) => {
                            s.push_str(sub);
                            self.pos = end;
                        }
                        Err(_) => s.push(c as char),
                    }
                }
            }
        }
        Ok(s)
    }

    fn parse_hex4(&mut self) -> Result<u32, String> {
        if self.pos + 4 > self.bytes.len() {
            return Err("bad \\u escape".into());
        }
        let mut v: u32 = 0;
        for _ in 0..4 {
            let d = self.bytes[self.pos];
            self.pos += 1;
            let nibble = match d {
                b'0'..=b'9' => (d - b'0') as u32,
                b'a'..=b'f' => (d - b'a' + 10) as u32,
                b'A'..=b'F' => (d - b'A' + 10) as u32,
                _ => return Err("bad hex digit".into()),
            };
            v = v * 16 + nibble;
        }
        Ok(v)
    }

    fn parse_bool(&mut self) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(Value::Bool(true))
        } else if self.bytes[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(Value::Bool(false))
        } else {
            Err(format!("bad literal at {}", self.pos))
        }
    }

    fn parse_null(&mut self) -> Result<Value, String> {
        if self.bytes[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(Value::Null)
        } else {
            Err(format!("bad literal at {}", self.pos))
        }
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        let mut is_float = false;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(c) = self.peek() {
            match c {
                b'0'..=b'9' => self.pos += 1,
                b'.' | b'e' | b'E' | b'+' | b'-' => {
                    is_float = true;
                    self.pos += 1;
                }
                _ => break,
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| "bad number")?;
        if is_float {
            text.parse::<f64>()
                .map(Value::Double)
                .map_err(|_| format!("bad number {text}"))
        } else {
            text.parse::<i64>()
                .map(Value::Int)
                .map_err(|_| format!("bad number {text}"))
        }
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_dump() {
        let mut v = Value::object();
        v.set("a", Value::int(1));
        v.set("b", Value::string("x\"y"));
        v.set("c", Value::bool(true));
        assert_eq!(v.dump(false), "{\"a\":1,\"b\":\"x\\\"y\",\"c\":true}");
    }

    #[test]
    fn parse_basics() {
        let parsed = parse("{\"k\": [1, 2.5, null], \"s\": \"v\"}").unwrap();
        assert_eq!(parsed.get_string("s", ""), "v");
        let k = parsed.find("k").unwrap();
        assert!(matches!(k, Value::Array(_)));
        let arr = k.as_array();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_i64(), 1);
        assert!((arr[1].as_f64() - 2.5).abs() < 1e-9);
        assert!(matches!(arr[2], Value::Null));
    }

    #[test]
    fn double_three_decimals() {
        assert_eq!(Value::double(4.404).dump(false), "4.404");
        assert_eq!(Value::double(4.3).dump(false), "4.300");
    }

    #[test]
    fn pretty_layout() {
        let mut v = Value::object();
        v.set("a", Value::int(1));
        let pretty = v.dump(true);
        assert_eq!(pretty, "{\n    \"a\": 1\n}");
    }
}
