use crate::types::{Endian, FieldType};

/// Encode a sequence of (type, value) pairs into a byte vector.
///
/// Values are formatted according to their type:
/// - Numeric: decimal or `0x`-prefixed hex (e.g. `42`, `0xFF`)
/// - Bool: `true`/`false`, `1`/`0`, `yes`/`no`
/// - Bits: binary string matching the bit width (e.g. `1010` for `b4`), or numeric
/// - StringFixed/StringRest: UTF-8 text
/// - PascalString: UTF-8 text (max 255 bytes)
/// - HexBytes: even-length hex string (e.g. `DEADBEEF`)
///
/// # Errors
///
/// Returns an error string describing the problem field and reason.
///
/// # Examples
///
/// ```
/// use struct_cli::{encode_fields, parse_type_list};
///
/// let types = parse_type_list("u8,u16,bool").unwrap();
/// let values = vec!["42".to_string(), "1000".to_string(), "true".to_string()];
/// let bytes = encode_fields(&types, &values).unwrap();
/// // u16 1000 = 0x03E8 in little-endian = [0xE8, 0x03]
/// assert_eq!(bytes, vec![0x2A, 0xE8, 0x03, 0x01]);
/// ```
pub fn encode_fields(types: &[FieldType], values: &[String]) -> Result<Vec<u8>, String> {
    if types.len() != values.len() {
        return Err(format!(
            "type count ({}) does not match value count ({})",
            types.len(),
            values.len()
        ));
    }

    let mut out = Vec::new();
    let mut bit_buf: u8 = 0;
    let mut bit_pos: u8 = 0;

    for (i, (ft, val)) in types.iter().zip(values.iter()).enumerate() {
        let label = format!("field {i} ({})", ft.type_name());

        if ft.is_bit_type() {
            let FieldType::Bits(n) = ft else { unreachable!() };
            let n = *n;
            if bit_pos + n > 8 {
                return Err(format!(
                    "{label}: bit fields cross byte boundary ({bit_pos} bits already packed)"
                ));
            }
            let bits = parse_bits(val, n, &label)?;
            bit_buf |= bits << (8 - bit_pos - n);
            bit_pos += n;
            if bit_pos == 8 {
                out.push(bit_buf);
                bit_buf = 0;
                bit_pos = 0;
            }
        } else {
            if bit_pos > 0 {
                out.push(bit_buf);
                bit_buf = 0;
                bit_pos = 0;
            }
            out.extend_from_slice(&encode_one(ft, val, &label)?);
        }
    }

    if bit_pos > 0 {
        out.push(bit_buf);
    }

    Ok(out)
}

fn parse_bits(val: &str, n: u8, label: &str) -> Result<u8, String> {
    if val.len() == n as usize && val.chars().all(|c| c == '0' || c == '1') {
        let mut v = 0u8;
        for c in val.chars() {
            v = (v << 1) | (c as u8 - b'0');
        }
        return Ok(v);
    }
    let v = parse_uint(val, label)?;
    if v >= (1u64 << n) {
        return Err(format!("{label}: value {v} does not fit in {n} bits"));
    }
    Ok(v as u8)
}

fn parse_uint(s: &str, label: &str) -> Result<u64, String> {
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).map_err(|e| format!("{label}: invalid hex: {e}"))
    } else {
        s.parse::<u64>().map_err(|e| format!("{label}: invalid integer: {e}"))
    }
}

fn parse_sint(s: &str, label: &str) -> Result<i64, String> {
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).map_err(|e| format!("{label}: invalid hex: {e}"))
    } else {
        s.parse::<i64>().map_err(|e| format!("{label}: invalid integer: {e}"))
    }
}

macro_rules! encode_int {
    ($val:expr, $ty:ty, $e:expr) => {
        match $e {
            Endian::Little => ($val as $ty).to_le_bytes().to_vec(),
            Endian::Big => ($val as $ty).to_be_bytes().to_vec(),
        }
    };
}

fn encode_one(ft: &FieldType, val: &str, label: &str) -> Result<Vec<u8>, String> {
    match ft {
        FieldType::U8 => {
            let v = parse_uint(val, label)?;
            if v > u8::MAX as u64 {
                return Err(format!("{label}: {v} out of range for u8 (0-255)"));
            }
            Ok(vec![v as u8])
        }
        FieldType::U16(e) => {
            let v = parse_uint(val, label)?;
            if v > u16::MAX as u64 {
                return Err(format!("{label}: {v} out of range for u16 (0-65535)"));
            }
            Ok(encode_int!(v, u16, e))
        }
        FieldType::U32(e) => {
            let v = parse_uint(val, label)?;
            if v > u32::MAX as u64 {
                return Err(format!("{label}: {v} out of range for u32"));
            }
            Ok(encode_int!(v, u32, e))
        }
        FieldType::U64(e) => {
            let v = parse_uint(val, label)?;
            Ok(encode_int!(v, u64, e))
        }
        FieldType::U128(e) => {
            let v: u128 = if let Some(h) = val.strip_prefix("0x").or_else(|| val.strip_prefix("0X")) {
                u128::from_str_radix(h, 16).map_err(|e| format!("{label}: invalid hex: {e}"))?
            } else {
                val.parse::<u128>().map_err(|e| format!("{label}: invalid integer: {e}"))?
            };
            Ok(match e {
                Endian::Little => v.to_le_bytes().to_vec(),
                Endian::Big => v.to_be_bytes().to_vec(),
            })
        }
        FieldType::I8 => {
            let v = parse_sint(val, label)?;
            if v < i8::MIN as i64 || v > i8::MAX as i64 {
                return Err(format!("{label}: {v} out of range for i8 (-128 to 127)"));
            }
            Ok(vec![(v as i8) as u8])
        }
        FieldType::I16(e) => {
            let v = parse_sint(val, label)?;
            if v < i16::MIN as i64 || v > i16::MAX as i64 {
                return Err(format!("{label}: {v} out of range for i16"));
            }
            Ok(encode_int!(v, i16, e))
        }
        FieldType::I32(e) => {
            let v = parse_sint(val, label)?;
            if v < i32::MIN as i64 || v > i32::MAX as i64 {
                return Err(format!("{label}: {v} out of range for i32"));
            }
            Ok(encode_int!(v, i32, e))
        }
        FieldType::I64(e) => {
            let v = parse_sint(val, label)?;
            Ok(encode_int!(v, i64, e))
        }
        FieldType::I128(e) => {
            let v: i128 = if let Some(h) = val.strip_prefix("0x").or_else(|| val.strip_prefix("0X")) {
                i128::from_str_radix(h, 16).map_err(|e| format!("{label}: invalid hex: {e}"))?
            } else {
                val.parse::<i128>().map_err(|e| format!("{label}: invalid integer: {e}"))?
            };
            Ok(match e {
                Endian::Little => v.to_le_bytes().to_vec(),
                Endian::Big => v.to_be_bytes().to_vec(),
            })
        }
        FieldType::Bool => match val.to_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(vec![1]),
            "false" | "0" | "no" => Ok(vec![0]),
            _ => Err(format!(
                "{label}: invalid bool value '{val}' (use true/false, 1/0, yes/no)"
            )),
        },
        FieldType::StringFixed(n) => {
            let bytes = val.as_bytes();
            if bytes.len() > *n {
                return Err(format!(
                    "{label}: string ({} bytes) exceeds field size {n}",
                    bytes.len()
                ));
            }
            let mut buf = vec![0u8; *n];
            buf[..bytes.len()].copy_from_slice(bytes);
            Ok(buf)
        }
        FieldType::StringRest => Ok(val.as_bytes().to_vec()),
        FieldType::PascalString => {
            let bytes = val.as_bytes();
            if bytes.len() > 255 {
                return Err(format!(
                    "{label}: string too long for pascal ({} > 255 bytes)",
                    bytes.len()
                ));
            }
            let mut out = vec![bytes.len() as u8];
            out.extend_from_slice(bytes);
            Ok(out)
        }
        FieldType::HexBytes(n) => {
            let bytes = parse_hex(val, label)?;
            if bytes.len() != *n {
                return Err(format!(
                    "{label}: expected {n} hex bytes, got {}",
                    bytes.len()
                ));
            }
            Ok(bytes)
        }
        FieldType::HexBytesRest => parse_hex(val, label),
        FieldType::Bits(_) => unreachable!("bit fields handled in caller"),
    }
}

/// Parse an even-length hex string (no separator) into bytes.
///
/// Accepts both uppercase and lowercase hex digits.
///
/// # Examples
///
/// ```
/// use struct_cli::parse_hex;
///
/// assert_eq!(parse_hex("DEADBEEF", "test").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
/// assert_eq!(parse_hex("deadbeef", "test").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
/// ```
pub fn parse_hex(s: &str, label: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        return Err(format!(
            "{label}: hex string must have an even number of characters (got {})",
            s.len()
        ));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("{label}: invalid hex at offset {i}: {e}"))
        })
        .collect()
}
