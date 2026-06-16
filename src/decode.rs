use serde_json::{Number, Value};

use crate::types::{Endian, FieldType};

/// Decode a sequence of fields from a byte slice.
///
/// Returns a `serde_json::Value::Array` with one element per non-skip field.
/// Skip fields consume bytes but produce no value. Group fields produce a nested
/// `Value::Array`. Primitive types produce typed values (numbers, booleans, strings).
///
/// Bit fields are packed MSB-first within each byte and must not cross byte
/// boundaries. Bit fields are not permitted inside groups.
///
/// # Errors
///
/// Returns an error string describing the problem field and reason.
///
/// # Examples
///
/// ```
/// use struct_cli::{decode_fields, parse_type_list};
///
/// let types = parse_type_list("u8,u16").unwrap();
/// // little-endian u16: value 256 is stored as [0x00, 0x01]
/// let result = decode_fields(&types, &[0x2A, 0x00, 0x01]).unwrap();
/// let arr = result.as_array().unwrap();
/// assert_eq!(arr[0], serde_json::json!(42));
/// assert_eq!(arr[1], serde_json::json!(256));
/// ```
pub fn decode_fields(types: &[FieldType], data: &[u8]) -> Result<Value, String> {
    let mut pos = 0usize;
    let mut bit_pos = 0u8;
    let mut bit_byte: u8 = 0;
    let mut in_bits = false;
    let mut results = Vec::new();

    for (i, ft) in types.iter().enumerate() {
        if let FieldType::Skip(n) = ft {
            if in_bits {
                in_bits = false;
                bit_pos = 0;
            }
            let remain = data.len() - pos;
            if remain < *n {
                return Err(format!(
                    "field {i} (z{n}): need {n} bytes, only {remain} available"
                ));
            }
            pos += n;
            // Skip produces no value
        } else if ft.is_bit_type() {
            if !in_bits {
                if pos >= data.len() {
                    return Err(format!("field {i} ({ft}): unexpected end of data"));
                }
                bit_byte = data[pos];
                pos += 1;
                bit_pos = 0;
                in_bits = true;
            }
            let FieldType::Bits(n) = ft else { unreachable!() };
            let n = *n;
            if bit_pos + n > 8 {
                return Err(format!(
                    "field {i} ({ft}): bit fields cross byte boundary ({bit_pos} bits already consumed)"
                ));
            }
            let shift = 8 - bit_pos - n;
            let mask = (1u8 << n) - 1;
            let val = (bit_byte >> shift) & mask;
            results.push(Value::String(format!("{val:0width$b}", width = n as usize)));
            bit_pos += n;
            if bit_pos == 8 {
                in_bits = false;
                bit_pos = 0;
            }
        } else if let FieldType::Group(inner) = ft {
            if in_bits {
                in_bits = false;
                bit_pos = 0;
            }
            let (group_val, consumed) = decode_group(inner, data, pos, i)?;
            pos += consumed;
            results.push(group_val);
        } else {
            if in_bits {
                in_bits = false;
                bit_pos = 0;
            }
            let r = decode_one(ft, data, pos, i)?;
            pos += r.consumed;
            results.push(r.value);
        }
    }

    Ok(Value::Array(results))
}

fn decode_group(
    types: &[FieldType],
    data: &[u8],
    start: usize,
    outer_idx: usize,
) -> Result<(Value, usize), String> {
    let mut pos = start;
    let mut results = Vec::new();

    for (i, ft) in types.iter().enumerate() {
        let label = format!("field {outer_idx}[{i}] ({})", ft.type_name());
        if let FieldType::Skip(n) = ft {
            let remain = data.len() - pos;
            if remain < *n {
                return Err(format!("{label}: need {n} bytes, only {remain} available"));
            }
            pos += n;
        } else if ft.is_bit_type() {
            return Err(format!("{label}: bit fields are not allowed inside groups"));
        } else if let FieldType::Group(inner) = ft {
            let (val, consumed) = decode_group(inner, data, pos, outer_idx)?;
            pos += consumed;
            results.push(val);
        } else {
            let r = decode_one(ft, data, pos, i)?;
            pos += r.consumed;
            results.push(r.value);
        }
    }

    Ok((Value::Array(results), pos - start))
}

struct OneResult {
    value: Value,
    consumed: usize,
}

macro_rules! decode_numeric {
    ($data:expr, $n:literal, $ty:ty, $e:expr) => {{
        let bytes: [u8; $n] = $data[..$n].try_into().unwrap();
        match $e {
            Endian::Little => <$ty>::from_le_bytes(bytes),
            Endian::Big => <$ty>::from_be_bytes(bytes),
        }
    }};
}

fn decode_one(ft: &FieldType, data: &[u8], pos: usize, i: usize) -> Result<OneResult, String> {
    let remain = &data[pos..];
    let label = format!("field {i} ({})", ft.type_name());

    macro_rules! need {
        ($n:expr) => {
            if remain.len() < $n {
                return Err(format!(
                    "{label}: need {} bytes, only {} available",
                    $n,
                    remain.len()
                ));
            }
        };
    }

    match ft {
        FieldType::U8 => {
            need!(1);
            Ok(OneResult { value: Value::from(remain[0]), consumed: 1 })
        }
        FieldType::U16(e) => {
            need!(2);
            Ok(OneResult { value: Value::from(decode_numeric!(remain, 2, u16, e)), consumed: 2 })
        }
        FieldType::U32(e) => {
            need!(4);
            Ok(OneResult { value: Value::from(decode_numeric!(remain, 4, u32, e)), consumed: 4 })
        }
        FieldType::U64(e) => {
            need!(8);
            Ok(OneResult { value: Value::from(decode_numeric!(remain, 8, u64, e)), consumed: 8 })
        }
        FieldType::U128(e) => {
            need!(16);
            // u128 may exceed serde_json's native integer range; use a string.
            Ok(OneResult {
                value: Value::String(decode_numeric!(remain, 16, u128, e).to_string()),
                consumed: 16,
            })
        }
        FieldType::I8 => {
            need!(1);
            Ok(OneResult { value: Value::from(remain[0] as i8), consumed: 1 })
        }
        FieldType::I16(e) => {
            need!(2);
            Ok(OneResult { value: Value::from(decode_numeric!(remain, 2, i16, e)), consumed: 2 })
        }
        FieldType::I32(e) => {
            need!(4);
            Ok(OneResult { value: Value::from(decode_numeric!(remain, 4, i32, e)), consumed: 4 })
        }
        FieldType::I64(e) => {
            need!(8);
            Ok(OneResult { value: Value::from(decode_numeric!(remain, 8, i64, e)), consumed: 8 })
        }
        FieldType::I128(e) => {
            need!(16);
            // i128 may exceed serde_json's native integer range; use a string.
            Ok(OneResult {
                value: Value::String(decode_numeric!(remain, 16, i128, e).to_string()),
                consumed: 16,
            })
        }
        FieldType::Bool => {
            need!(1);
            Ok(OneResult { value: Value::Bool(remain[0] != 0), consumed: 1 })
        }
        FieldType::StringFixed(n) => {
            need!(*n);
            let bytes = &remain[..*n];
            let end = bytes.iter().rposition(|&b| b != 0).map(|p| p + 1).unwrap_or(0);
            let s = std::str::from_utf8(&bytes[..end])
                .map_err(|e| format!("{label}: invalid UTF-8: {e}"))?;
            Ok(OneResult { value: Value::String(s.to_string()), consumed: *n })
        }
        FieldType::StringRest => {
            let s = std::str::from_utf8(remain)
                .map_err(|e| format!("{label}: invalid UTF-8: {e}"))?;
            Ok(OneResult { value: Value::String(s.to_string()), consumed: remain.len() })
        }
        FieldType::PascalString => {
            need!(1);
            let len = remain[0] as usize;
            need!(1 + len);
            let s = std::str::from_utf8(&remain[1..1 + len])
                .map_err(|e| format!("{label}: invalid UTF-8: {e}"))?;
            Ok(OneResult { value: Value::String(s.to_string()), consumed: 1 + len })
        }
        FieldType::HexBytes(n) => {
            need!(*n);
            let hex: String = remain[..*n].iter().map(|b| format!("{b:02X}")).collect();
            Ok(OneResult { value: Value::String(hex), consumed: *n })
        }
        FieldType::HexBytesRest => {
            let hex: String = remain.iter().map(|b| format!("{b:02X}")).collect();
            Ok(OneResult { value: Value::String(hex), consumed: remain.len() })
        }
        FieldType::Bits(_) => unreachable!("bit fields handled in caller"),
        FieldType::Skip(_) => unreachable!("skip handled in caller"),
        FieldType::Group(_) => unreachable!("groups handled in caller"),
    }
}

// Suppress unused import warning — Number is used implicitly via Value::from impls
// but we may need it for future extensions.
#[allow(dead_code)]
fn _use_number(_: Number) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::parse_type_list;

    #[test]
    fn test_decode_typed_numbers() {
        let types = parse_type_list("u8,i8,bool").unwrap();
        let result = decode_fields(&types, &[42, 0xFF, 1]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0], serde_json::json!(42u8));
        assert_eq!(arr[1], serde_json::json!(-1i8));
        assert_eq!(arr[2], serde_json::json!(true));
    }

    #[test]
    fn test_decode_skip_transparent() {
        let types = parse_type_list("u8,z2,u8").unwrap();
        let result = decode_fields(&types, &[10, 0, 0, 20]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2, "skip fields do not appear in output");
        assert_eq!(arr[0], serde_json::json!(10));
        assert_eq!(arr[1], serde_json::json!(20));
    }

    #[test]
    fn test_decode_group() {
        let types = parse_type_list("[u8,u8],u8").unwrap();
        let result = decode_fields(&types, &[1, 2, 3]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], serde_json::json!([1, 2]));
        assert_eq!(arr[1], serde_json::json!(3));
    }

    #[test]
    fn test_decode_nested_group() {
        let types = parse_type_list("[u8,[u8,u8]]").unwrap();
        let result = decode_fields(&types, &[1, 2, 3]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], serde_json::json!([1, [2, 3]]));
    }

    #[test]
    fn test_decode_group_with_skip() {
        let types = parse_type_list("[u8,z1,u8]").unwrap();
        let result = decode_fields(&types, &[10, 0, 20]).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], serde_json::json!([10, 20]));
    }

    #[test]
    fn test_decode_u128_as_string() {
        let types = parse_type_list("u128").unwrap();
        let mut data = [0u8; 16];
        data[15] = 1; // value = 2^120 in big-endian, but we're little-endian
        // Actually little-endian: byte[0] is LSB. Let's do value = 1.
        data[0] = 1;
        data[15] = 0;
        let result = decode_fields(&types, &data).unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr[0], serde_json::json!("1"));
    }
}
