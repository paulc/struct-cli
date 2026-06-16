use serde_json::Value;

use crate::types::{Endian, FieldType};

/// Encode a sequence of (type, value) pairs into a byte vector.
///
/// `values` must be a `serde_json::Value::Array`. Each element corresponds to
/// one non-skip field in `types` (skip fields consume no value slot). Group
/// fields consume one element, which must itself be a JSON array.
///
/// Values are accepted as typed JSON (numbers, booleans) or as strings:
/// - Numeric: JSON number, or decimal/`0x`-prefixed hex string
/// - Bool: JSON boolean, or `"true"`/`"false"`/`"1"`/`"0"`/`"yes"`/`"no"`
/// - Bits: binary string matching the bit width (e.g. `"1010"` for `b4`), or number
/// - StringFixed/StringRest/PascalString: JSON string
/// - HexBytes: even-length hex string (e.g. `"DEADBEEF"`)
/// - Group: JSON array of values for the inner fields
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
/// // Accepts typed JSON values
/// let bytes = encode_fields(&types, &serde_json::json!([42, 1000, true])).unwrap();
/// assert_eq!(bytes, vec![0x2A, 0xE8, 0x03, 0x01]);
///
/// // Also accepts string values
/// let bytes2 = encode_fields(&types, &serde_json::json!(["42", "1000", "true"])).unwrap();
/// assert_eq!(bytes2, bytes);
/// ```
pub fn encode_fields(types: &[FieldType], values: &Value) -> Result<Vec<u8>, String> {
    let vals = values.as_array().ok_or("values must be a JSON array")?;

    let expected = non_skip_count(types);
    if vals.len() != expected {
        return Err(format!(
            "value count ({}) does not match field count ({expected})",
            vals.len()
        ));
    }

    let mut out = Vec::new();
    let mut bit_buf: u8 = 0;
    let mut bit_pos: u8 = 0;
    let mut val_idx = 0usize;

    encode_top_level(types, vals, &mut val_idx, &mut out, &mut bit_buf, &mut bit_pos)?;

    flush_bits(&mut bit_buf, &mut bit_pos, &mut out);
    Ok(out)
}

/// Number of value slots consumed by `types` (skips are transparent).
fn non_skip_count(types: &[FieldType]) -> usize {
    types.iter().filter(|t| !matches!(t, FieldType::Skip(_))).count()
}

fn flush_bits(bit_buf: &mut u8, bit_pos: &mut u8, out: &mut Vec<u8>) {
    if *bit_pos > 0 {
        out.push(*bit_buf);
        *bit_buf = 0;
        *bit_pos = 0;
    }
}

fn encode_top_level(
    types: &[FieldType],
    vals: &[Value],
    val_idx: &mut usize,
    out: &mut Vec<u8>,
    bit_buf: &mut u8,
    bit_pos: &mut u8,
) -> Result<(), String> {
    for (i, ft) in types.iter().enumerate() {
        if let FieldType::Skip(n) = ft {
            flush_bits(bit_buf, bit_pos, out);
            out.extend(std::iter::repeat(0u8).take(*n));
            continue;
        }

        // All remaining variants consume one value slot
        let val = &vals[*val_idx];
        *val_idx += 1;
        let label = format!("field {i} ({})", ft.type_name());

        if ft.is_bit_type() {
            let FieldType::Bits(n) = ft else { unreachable!() };
            let n = *n;
            if *bit_pos + n > 8 {
                return Err(format!(
                    "{label}: bit fields cross byte boundary ({bit_pos} bits already packed)"
                ));
            }
            let bits = parse_bits_value(val, n, &label)?;
            *bit_buf |= bits << (8 - *bit_pos - n);
            *bit_pos += n;
            if *bit_pos == 8 {
                out.push(*bit_buf);
                *bit_buf = 0;
                *bit_pos = 0;
            }
        } else if let FieldType::Group(inner) = ft {
            flush_bits(bit_buf, bit_pos, out);
            let inner_vals = val
                .as_array()
                .ok_or_else(|| format!("{label}: expected JSON array for group"))?;
            let expected = non_skip_count(inner);
            if inner_vals.len() != expected {
                return Err(format!(
                    "{label}: group value count ({}) does not match field count ({expected})",
                    inner_vals.len()
                ));
            }
            let mut inner_idx = 0usize;
            encode_group(inner, inner_vals, &mut inner_idx, out, &label)?;
        } else {
            flush_bits(bit_buf, bit_pos, out);
            out.extend_from_slice(&encode_one(ft, val, &label)?);
        }
    }
    Ok(())
}

fn encode_group(
    types: &[FieldType],
    vals: &[Value],
    val_idx: &mut usize,
    out: &mut Vec<u8>,
    parent_label: &str,
) -> Result<(), String> {
    for (i, ft) in types.iter().enumerate() {
        let label = format!("{parent_label}[{i}] ({})", ft.type_name());

        if let FieldType::Skip(n) = ft {
            out.extend(std::iter::repeat(0u8).take(*n));
            continue;
        }
        if ft.is_bit_type() {
            return Err(format!("{label}: bit fields are not allowed inside groups"));
        }

        let val = &vals[*val_idx];
        *val_idx += 1;

        if let FieldType::Group(inner) = ft {
            let inner_vals = val
                .as_array()
                .ok_or_else(|| format!("{label}: expected JSON array for nested group"))?;
            let expected = non_skip_count(inner);
            if inner_vals.len() != expected {
                return Err(format!(
                    "{label}: group value count ({}) does not match field count ({expected})",
                    inner_vals.len()
                ));
            }
            let mut inner_idx = 0usize;
            encode_group(inner, inner_vals, &mut inner_idx, out, &label)?;
        } else {
            out.extend_from_slice(&encode_one(ft, val, &label)?);
        }
    }
    Ok(())
}

fn json_to_u64(val: &Value, label: &str) -> Result<u64, String> {
    match val {
        Value::Number(n) => {
            n.as_u64().ok_or_else(|| format!("{label}: expected non-negative integer, got {n}"))
        }
        Value::String(s) => parse_uint(s, label),
        _ => Err(format!("{label}: expected number or string for integer field")),
    }
}

fn json_to_i64(val: &Value, label: &str) -> Result<i64, String> {
    match val {
        Value::Number(n) => {
            n.as_i64().ok_or_else(|| format!("{label}: expected integer, got {n}"))
        }
        Value::String(s) => parse_sint(s, label),
        _ => Err(format!("{label}: expected number or string for integer field")),
    }
}

fn val_to_str<'a>(val: &'a Value, label: &str) -> Result<&'a str, String> {
    val.as_str().ok_or_else(|| format!("{label}: expected a string value"))
}

fn parse_bits_value(val: &Value, n: u8, label: &str) -> Result<u8, String> {
    match val {
        Value::String(s) => parse_bits(s, n, label),
        Value::Number(num) => {
            let v = num
                .as_u64()
                .ok_or_else(|| format!("{label}: invalid bit value"))?;
            if v >= (1u64 << n) {
                return Err(format!("{label}: value {v} does not fit in {n} bits"));
            }
            Ok(v as u8)
        }
        _ => Err(format!("{label}: expected string or number for bit field")),
    }
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

fn encode_one(ft: &FieldType, val: &Value, label: &str) -> Result<Vec<u8>, String> {
    match ft {
        FieldType::U8 => {
            let v = json_to_u64(val, label)?;
            if v > u8::MAX as u64 {
                return Err(format!("{label}: {v} out of range for u8 (0-255)"));
            }
            Ok(vec![v as u8])
        }
        FieldType::U16(e) => {
            let v = json_to_u64(val, label)?;
            if v > u16::MAX as u64 {
                return Err(format!("{label}: {v} out of range for u16 (0-65535)"));
            }
            Ok(encode_int!(v, u16, e))
        }
        FieldType::U32(e) => {
            let v = json_to_u64(val, label)?;
            if v > u32::MAX as u64 {
                return Err(format!("{label}: {v} out of range for u32"));
            }
            Ok(encode_int!(v, u32, e))
        }
        FieldType::U64(e) => {
            let v = json_to_u64(val, label)?;
            Ok(encode_int!(v, u64, e))
        }
        FieldType::U128(e) => {
            let v: u128 = match val {
                Value::String(s) => {
                    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                        u128::from_str_radix(h, 16)
                            .map_err(|e| format!("{label}: invalid hex: {e}"))?
                    } else {
                        s.parse::<u128>()
                            .map_err(|e| format!("{label}: invalid integer: {e}"))?
                    }
                }
                Value::Number(n) => n
                    .as_u64()
                    .map(|x| x as u128)
                    .ok_or_else(|| format!("{label}: value out of range for u128"))?,
                _ => return Err(format!("{label}: expected string or number for u128")),
            };
            Ok(match e {
                Endian::Little => v.to_le_bytes().to_vec(),
                Endian::Big => v.to_be_bytes().to_vec(),
            })
        }
        FieldType::I8 => {
            let v = json_to_i64(val, label)?;
            if v < i8::MIN as i64 || v > i8::MAX as i64 {
                return Err(format!("{label}: {v} out of range for i8 (-128 to 127)"));
            }
            Ok(vec![(v as i8) as u8])
        }
        FieldType::I16(e) => {
            let v = json_to_i64(val, label)?;
            if v < i16::MIN as i64 || v > i16::MAX as i64 {
                return Err(format!("{label}: {v} out of range for i16"));
            }
            Ok(encode_int!(v, i16, e))
        }
        FieldType::I32(e) => {
            let v = json_to_i64(val, label)?;
            if v < i32::MIN as i64 || v > i32::MAX as i64 {
                return Err(format!("{label}: {v} out of range for i32"));
            }
            Ok(encode_int!(v, i32, e))
        }
        FieldType::I64(e) => {
            let v = json_to_i64(val, label)?;
            Ok(encode_int!(v, i64, e))
        }
        FieldType::I128(e) => {
            let v: i128 = match val {
                Value::String(s) => {
                    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                        i128::from_str_radix(h, 16)
                            .map_err(|e| format!("{label}: invalid hex: {e}"))?
                    } else {
                        s.parse::<i128>()
                            .map_err(|e| format!("{label}: invalid integer: {e}"))?
                    }
                }
                Value::Number(n) => n
                    .as_i64()
                    .map(|x| x as i128)
                    .ok_or_else(|| format!("{label}: value out of range for i128"))?,
                _ => return Err(format!("{label}: expected string or number for i128")),
            };
            Ok(match e {
                Endian::Little => v.to_le_bytes().to_vec(),
                Endian::Big => v.to_be_bytes().to_vec(),
            })
        }
        FieldType::Bool => match val {
            Value::Bool(b) => Ok(vec![*b as u8]),
            Value::Number(n) => match n.as_u64() {
                Some(0) => Ok(vec![0]),
                Some(1) => Ok(vec![1]),
                _ => Err(format!("{label}: bool value must be 0 or 1")),
            },
            Value::String(s) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(vec![1]),
                "false" | "0" | "no" => Ok(vec![0]),
                _ => Err(format!(
                    "{label}: invalid bool value '{s}' (use true/false, 1/0, yes/no)"
                )),
            },
            _ => Err(format!("{label}: expected bool, number, or string")),
        },
        FieldType::StringFixed(n) => {
            let s = val_to_str(val, label)?;
            let bytes = s.as_bytes();
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
        FieldType::StringRest => {
            let s = val_to_str(val, label)?;
            Ok(s.as_bytes().to_vec())
        }
        FieldType::PascalString => {
            let s = val_to_str(val, label)?;
            let bytes = s.as_bytes();
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
            let s = val_to_str(val, label)?;
            let bytes = parse_hex(s, label)?;
            if bytes.len() != *n {
                return Err(format!(
                    "{label}: expected {n} hex bytes, got {}",
                    bytes.len()
                ));
            }
            Ok(bytes)
        }
        FieldType::HexBytesRest => {
            let s = val_to_str(val, label)?;
            parse_hex(s, label)
        }
        FieldType::Bits(_) => unreachable!("bit fields handled in caller"),
        FieldType::Skip(_) => unreachable!("skip handled in caller"),
        FieldType::Group(_) => unreachable!("groups handled in caller"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::parse_type_list;

    #[test]
    fn test_encode_typed_json_values() {
        let types = parse_type_list("u8,u16,bool").unwrap();
        let bytes = encode_fields(&types, &serde_json::json!([42, 1000, true])).unwrap();
        assert_eq!(bytes, vec![0x2A, 0xE8, 0x03, 0x01]);
    }

    #[test]
    fn test_encode_string_values_still_work() {
        let types = parse_type_list("u8,u16,bool").unwrap();
        let bytes =
            encode_fields(&types, &serde_json::json!(["42", "1000", "true"])).unwrap();
        assert_eq!(bytes, vec![0x2A, 0xE8, 0x03, 0x01]);
    }

    #[test]
    fn test_encode_skip_transparent() {
        let types = parse_type_list("u8,z2,u8").unwrap();
        let bytes = encode_fields(&types, &serde_json::json!([10, 20])).unwrap();
        assert_eq!(bytes, vec![10, 0, 0, 20]);
    }

    #[test]
    fn test_encode_skip_writes_zeros() {
        let types = parse_type_list("z3").unwrap();
        // z3 has no value slots, so pass empty array
        let bytes = encode_fields(&types, &serde_json::json!([])).unwrap();
        assert_eq!(bytes, vec![0, 0, 0]);
    }

    #[test]
    fn test_encode_group() {
        let types = parse_type_list("[u8,u8],u8").unwrap();
        let bytes = encode_fields(&types, &serde_json::json!([[1, 2], 3])).unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[test]
    fn test_encode_nested_group() {
        let types = parse_type_list("[u8,[u8,u8]]").unwrap();
        let bytes = encode_fields(&types, &serde_json::json!([[1, [2, 3]]])).unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
    }

    #[test]
    fn test_encode_group_with_skip() {
        let types = parse_type_list("[u8,z1,u8]").unwrap();
        let bytes = encode_fields(&types, &serde_json::json!([[10, 20]])).unwrap();
        assert_eq!(bytes, vec![10, 0, 20]);
    }

    #[test]
    fn test_encode_wrong_value_count() {
        let types = parse_type_list("u8,z2,u8").unwrap();
        // z2 is transparent, so we need 2 values, not 3
        assert!(encode_fields(&types, &serde_json::json!([1, 2, 3])).is_err());
    }

    #[test]
    fn test_encode_bool_typed() {
        let types = parse_type_list("bool").unwrap();
        assert_eq!(
            encode_fields(&types, &serde_json::json!([true])).unwrap(),
            vec![1]
        );
        assert_eq!(
            encode_fields(&types, &serde_json::json!([false])).unwrap(),
            vec![0]
        );
    }
}
