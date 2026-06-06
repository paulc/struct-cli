use crate::types::{Endian, FieldType};

/// A single decoded field value.
#[derive(Debug)]
pub struct DecodeResult {
    /// Canonical type name (e.g. `"u16"`, `">u32"`).
    pub type_name: String,
    /// Decoded value as a string.
    pub value: String,
}

/// Decode a sequence of fields from a byte slice.
///
/// Bit fields are packed MSB-first within each byte and must not cross byte
/// boundaries. All other types consume whole bytes.
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
/// let results = decode_fields(&types, &[0x2A, 0x00, 0x01]).unwrap();
/// assert_eq!(results[0].value, "42");
/// assert_eq!(results[1].value, "256");
/// ```
pub fn decode_fields(types: &[FieldType], data: &[u8]) -> Result<Vec<DecodeResult>, String> {
    let mut results = Vec::new();
    let mut pos = 0usize;
    let mut bit_pos = 0u8;
    let mut bit_byte: u8 = 0;
    let mut in_bits = false;

    for (i, ft) in types.iter().enumerate() {
        if ft.is_bit_type() {
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
            results.push(DecodeResult {
                type_name: ft.type_name(),
                value: format!("{val:0width$b}", width = n as usize),
            });
            bit_pos += n;
            if bit_pos == 8 {
                in_bits = false;
                bit_pos = 0;
            }
        } else {
            if in_bits {
                in_bits = false;
                bit_pos = 0;
            }
            let r = decode_one(ft, data, pos, i)?;
            pos += r.consumed;
            results.push(DecodeResult {
                type_name: ft.type_name(),
                value: r.value,
            });
        }
    }
    Ok(results)
}

struct OneResult {
    value: String,
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
            Ok(OneResult { value: remain[0].to_string(), consumed: 1 })
        }
        FieldType::U16(e) => {
            need!(2);
            Ok(OneResult { value: decode_numeric!(remain, 2, u16, e).to_string(), consumed: 2 })
        }
        FieldType::U32(e) => {
            need!(4);
            Ok(OneResult { value: decode_numeric!(remain, 4, u32, e).to_string(), consumed: 4 })
        }
        FieldType::U64(e) => {
            need!(8);
            Ok(OneResult { value: decode_numeric!(remain, 8, u64, e).to_string(), consumed: 8 })
        }
        FieldType::U128(e) => {
            need!(16);
            Ok(OneResult { value: decode_numeric!(remain, 16, u128, e).to_string(), consumed: 16 })
        }
        FieldType::I8 => {
            need!(1);
            Ok(OneResult { value: (remain[0] as i8).to_string(), consumed: 1 })
        }
        FieldType::I16(e) => {
            need!(2);
            Ok(OneResult { value: decode_numeric!(remain, 2, i16, e).to_string(), consumed: 2 })
        }
        FieldType::I32(e) => {
            need!(4);
            Ok(OneResult { value: decode_numeric!(remain, 4, i32, e).to_string(), consumed: 4 })
        }
        FieldType::I64(e) => {
            need!(8);
            Ok(OneResult { value: decode_numeric!(remain, 8, i64, e).to_string(), consumed: 8 })
        }
        FieldType::I128(e) => {
            need!(16);
            Ok(OneResult { value: decode_numeric!(remain, 16, i128, e).to_string(), consumed: 16 })
        }
        FieldType::Bool => {
            need!(1);
            Ok(OneResult {
                value: if remain[0] != 0 { "true" } else { "false" }.into(),
                consumed: 1,
            })
        }
        FieldType::StringFixed(n) => {
            need!(*n);
            let bytes = &remain[..*n];
            let end = bytes.iter().rposition(|&b| b != 0).map(|p| p + 1).unwrap_or(0);
            let s = std::str::from_utf8(&bytes[..end])
                .map_err(|e| format!("{label}: invalid UTF-8: {e}"))?;
            Ok(OneResult { value: s.to_string(), consumed: *n })
        }
        FieldType::StringRest => {
            let s = std::str::from_utf8(remain)
                .map_err(|e| format!("{label}: invalid UTF-8: {e}"))?;
            Ok(OneResult { value: s.to_string(), consumed: remain.len() })
        }
        FieldType::PascalString => {
            need!(1);
            let len = remain[0] as usize;
            need!(1 + len);
            let s = std::str::from_utf8(&remain[1..1 + len])
                .map_err(|e| format!("{label}: invalid UTF-8: {e}"))?;
            Ok(OneResult { value: s.to_string(), consumed: 1 + len })
        }
        FieldType::HexBytes(n) => {
            need!(*n);
            let hex: String = remain[..*n].iter().map(|b| format!("{b:02X}")).collect();
            Ok(OneResult { value: hex, consumed: *n })
        }
        FieldType::HexBytesRest => {
            let hex: String = remain.iter().map(|b| format!("{b:02X}")).collect();
            Ok(OneResult { value: hex, consumed: remain.len() })
        }
        FieldType::Bits(_) => unreachable!("bit fields handled in caller"),
    }
}
