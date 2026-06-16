use std::fmt;
use serde_json::Value;

/// Byte order for multi-byte numeric fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Default for Endian {
    fn default() -> Self {
        Endian::Little
    }
}

impl Endian {
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Endian::Big => ">",
            Endian::Little => "",
        }
    }
}

/// A single typed field in a struct definition.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    U8,
    U16(Endian),
    U32(Endian),
    U64(Endian),
    U128(Endian),
    I8,
    I16(Endian),
    I32(Endian),
    I64(Endian),
    I128(Endian),
    /// 1-byte boolean: 0 = false, any other value = true.
    Bool,
    /// Bit field of N bits (1-7), packed MSB-first within a byte.
    Bits(u8),
    /// Fixed-length UTF-8 string occupying exactly N bytes, zero-padded.
    StringFixed(usize),
    /// Unbounded UTF-8 string that consumes the remainder of the input.
    StringRest,
    /// Pascal string: 1-byte length prefix followed by UTF-8 data (max 255 bytes).
    PascalString,
    /// Exactly N raw bytes, encoded/decoded as a hex string.
    HexBytes(usize),
    /// Unbounded raw bytes that consume the remainder of the input.
    HexBytesRest,
    /// Skip N bytes: writes zeros on encode, discards bytes on decode. No value slot.
    Skip(usize),
    /// A group of fields encoded/decoded as a unit, represented as a JSON array.
    /// Bit fields are not permitted inside groups (byte-aligned only).
    Group(Vec<FieldType>),
}

impl FieldType {
    /// Returns the canonical type name string.
    pub fn type_name(&self) -> String {
        match self {
            FieldType::U8 => "u8".into(),
            FieldType::U16(e) => format!("{}u16", e.prefix()),
            FieldType::U32(e) => format!("{}u32", e.prefix()),
            FieldType::U64(e) => format!("{}u64", e.prefix()),
            FieldType::U128(e) => format!("{}u128", e.prefix()),
            FieldType::I8 => "i8".into(),
            FieldType::I16(e) => format!("{}i16", e.prefix()),
            FieldType::I32(e) => format!("{}i32", e.prefix()),
            FieldType::I64(e) => format!("{}i64", e.prefix()),
            FieldType::I128(e) => format!("{}i128", e.prefix()),
            FieldType::Bool => "bool".into(),
            FieldType::Bits(n) => format!("b{n}"),
            FieldType::StringFixed(n) => format!("s{n}"),
            FieldType::StringRest => "s".into(),
            FieldType::PascalString => "p".into(),
            FieldType::HexBytes(n) => format!("x{n}"),
            FieldType::HexBytesRest => "x".into(),
            FieldType::Skip(n) => format!("z{n}"),
            FieldType::Group(fields) => {
                let inner = fields.iter().map(|f| f.type_name()).collect::<Vec<_>>().join(",");
                format!("[{inner}]")
            }
        }
    }

    /// Serialise as the JSON type descriptor used in `--types-json` and config files.
    ///
    /// Primitives and skip become a JSON string; groups become a nested JSON array.
    pub fn to_json_type(&self) -> Value {
        match self {
            FieldType::Group(fields) => {
                Value::Array(fields.iter().map(|f| f.to_json_type()).collect())
            }
            _ => Value::String(self.type_name()),
        }
    }

    pub fn is_bit_type(&self) -> bool {
        matches!(self, FieldType::Bits(_))
    }
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.type_name())
    }
}

/// Parse a single type descriptor string into a [`FieldType`].
///
/// Numeric types may be prefixed with `>` (big-endian) or `<` (little-endian).
/// The default (no prefix) is little-endian.
///
/// # Examples
///
/// ```
/// use struct_cli::{parse_type, FieldType, Endian};
///
/// assert_eq!(parse_type("u16").unwrap(), FieldType::U16(Endian::Little));
/// assert_eq!(parse_type(">u16").unwrap(), FieldType::U16(Endian::Big));
/// assert_eq!(parse_type("b4").unwrap(), FieldType::Bits(4));
/// assert_eq!(parse_type("s8").unwrap(), FieldType::StringFixed(8));
/// assert_eq!(parse_type("z4").unwrap(), FieldType::Skip(4));
/// ```
pub fn parse_type(s: &str) -> Result<FieldType, String> {
    let (endian, s) = if let Some(r) = s.strip_prefix('>') {
        (Endian::Big, r)
    } else if let Some(r) = s.strip_prefix('<') {
        (Endian::Little, r)
    } else {
        (Endian::Little, s)
    };

    match s {
        "u8" => Ok(FieldType::U8),
        "u16" => Ok(FieldType::U16(endian)),
        "u32" => Ok(FieldType::U32(endian)),
        "u64" => Ok(FieldType::U64(endian)),
        "u128" => Ok(FieldType::U128(endian)),
        "i8" => Ok(FieldType::I8),
        "i16" => Ok(FieldType::I16(endian)),
        "i32" => Ok(FieldType::I32(endian)),
        "i64" => Ok(FieldType::I64(endian)),
        "i128" => Ok(FieldType::I128(endian)),
        "bool" => Ok(FieldType::Bool),
        "s" => Ok(FieldType::StringRest),
        "p" => Ok(FieldType::PascalString),
        "x" => Ok(FieldType::HexBytesRest),
        _ if s.starts_with('b') => {
            let n: u8 = s[1..].parse().map_err(|_| format!("invalid bit type: {s}"))?;
            if n == 0 || n > 7 {
                return Err(format!("bit width b{n} out of range (1-7)"));
            }
            Ok(FieldType::Bits(n))
        }
        _ if s.starts_with('s') => {
            let n: usize = s[1..].parse().map_err(|_| format!("invalid string type: {s}"))?;
            if n == 0 {
                return Err("string length must be > 0".into());
            }
            Ok(FieldType::StringFixed(n))
        }
        _ if s.starts_with('x') => {
            let n: usize = s[1..].parse().map_err(|_| format!("invalid hex type: {s}"))?;
            if n == 0 {
                return Err("hex byte count must be > 0".into());
            }
            Ok(FieldType::HexBytes(n))
        }
        _ if s.starts_with('z') => {
            let n: usize = s[1..].parse().map_err(|_| format!("invalid skip type: {s}"))?;
            if n == 0 {
                return Err("skip byte count must be > 0".into());
            }
            Ok(FieldType::Skip(n))
        }
        _ => Err(format!("unknown type: {s}")),
    }
}

/// Split a type list string on commas, respecting `[...]` bracket nesting.
fn split_type_tokens(s: &str) -> Result<Vec<&str>, String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                if depth == 0 {
                    return Err("unexpected ']' in type list".into());
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unclosed '[' in type list".into());
    }
    let last = s[start..].trim();
    if !last.is_empty() || !parts.is_empty() {
        parts.push(last);
    }
    Ok(parts)
}

fn parse_type_token(s: &str) -> Result<FieldType, String> {
    if let Some(inner) = s.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        let fields = parse_type_list(inner)?;
        Ok(FieldType::Group(fields))
    } else {
        parse_type(s)
    }
}

/// Parse a comma-separated list of type descriptors, supporting `[...]` groups.
///
/// Groups can be nested: `[i8,[i8,i8]]`.
///
/// # Examples
///
/// ```
/// use struct_cli::{parse_type_list, FieldType, Endian};
///
/// let types = parse_type_list("u8,>u16,bool").unwrap();
/// assert_eq!(types[0], FieldType::U8);
/// assert_eq!(types[1], FieldType::U16(Endian::Big));
/// assert_eq!(types[2], FieldType::Bool);
///
/// // Groups produce a nested Vec
/// let types = parse_type_list("i32,[i8,i8],i16").unwrap();
/// assert_eq!(types.len(), 3);
/// assert!(matches!(&types[1], FieldType::Group(inner) if inner.len() == 2));
///
/// // Skip type
/// let types = parse_type_list("u8,z4,u8").unwrap();
/// assert_eq!(types[1], FieldType::Skip(4));
/// ```
pub fn parse_type_list(s: &str) -> Result<Vec<FieldType>, String> {
    let tokens = split_type_tokens(s)?;
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| parse_type_token(t).map_err(|e| format!("field {i}: {e}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skip() {
        assert_eq!(parse_type("z4").unwrap(), FieldType::Skip(4));
        assert_eq!(parse_type("z1").unwrap(), FieldType::Skip(1));
        assert!(parse_type("z0").is_err());
    }

    #[test]
    fn test_parse_group() {
        let types = parse_type_list("i32,[i8,i8],i16").unwrap();
        assert_eq!(types.len(), 3);
        assert_eq!(types[0], FieldType::I32(Endian::Little));
        assert_eq!(types[2], FieldType::I16(Endian::Little));
        if let FieldType::Group(inner) = &types[1] {
            assert_eq!(inner.len(), 2);
            assert_eq!(inner[0], FieldType::I8);
            assert_eq!(inner[1], FieldType::I8);
        } else {
            panic!("expected Group");
        }
    }

    #[test]
    fn test_parse_nested_group() {
        let types = parse_type_list("i32,[i8,[i8,i8]],i16").unwrap();
        assert_eq!(types.len(), 3);
        if let FieldType::Group(outer) = &types[1] {
            assert_eq!(outer.len(), 2);
            assert_eq!(outer[0], FieldType::I8);
            if let FieldType::Group(inner) = &outer[1] {
                assert_eq!(inner.len(), 2);
                assert_eq!(inner[0], FieldType::I8);
                assert_eq!(inner[1], FieldType::I8);
            } else {
                panic!("expected nested Group");
            }
        } else {
            panic!("expected outer Group");
        }
    }

    #[test]
    fn test_type_name_group() {
        let ft = FieldType::Group(vec![FieldType::I8, FieldType::U16(Endian::Big)]);
        assert_eq!(ft.type_name(), "[i8,>u16]");
    }

    #[test]
    fn test_to_json_type_group() {
        let ft = FieldType::Group(vec![FieldType::I8, FieldType::I8]);
        let jt = ft.to_json_type();
        assert_eq!(jt, serde_json::json!(["i8", "i8"]));
    }

    #[test]
    fn test_to_json_type_nested() {
        let ft = FieldType::Group(vec![
            FieldType::I8,
            FieldType::Group(vec![FieldType::I8, FieldType::I8]),
        ]);
        let jt = ft.to_json_type();
        assert_eq!(jt, serde_json::json!(["i8", ["i8", "i8"]]));
    }
}
