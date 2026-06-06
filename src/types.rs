use std::fmt;

/// Byte order for multi-byte numeric fields.
///
/// The default (undecorated type) is little-endian.
/// Prefix a type with `>` for big-endian or `<` for explicit little-endian.
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
}

impl FieldType {
    /// Returns the canonical type name string.
    ///
    /// Big-endian numeric types are prefixed with `>`. Little-endian (default)
    /// has no prefix.
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
/// # Errors
///
/// Returns an error string if the type is unrecognised or has an invalid parameter.
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
        _ => Err(format!("unknown type: {s}")),
    }
}

/// Parse a comma-separated list of type descriptors.
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
/// ```
pub fn parse_type_list(s: &str) -> Result<Vec<FieldType>, String> {
    s.split(',')
        .enumerate()
        .map(|(i, t)| parse_type(t.trim()).map_err(|e| format!("field {i}: {e}")))
        .collect()
}
