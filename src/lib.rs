//! # struct-cli
//!
//! A library for packing and unpacking binary structs from typed field definitions.
//!
//! Fields are described using type strings. Encoding and decoding support
//! unsigned and signed integers, booleans, bit fields, fixed and unbounded
//! strings, pascal strings, raw byte sequences, skip fields, and grouped fields.
//!
//! All numeric types default to **little-endian**. Prefix a type with `>` for
//! big-endian or `<` for explicit little-endian.
//!
//! ## Type Reference
//!
//! | Type | Description |
//! |------|-------------|
//! | `u8`/`u16`/`u32`/`u64`/`u128` | Unsigned integer (little-endian by default) |
//! | `i8`/`i16`/`i32`/`i64`/`i128` | Signed integer (little-endian by default) |
//! | `>u16`, `>i32`, etc. | Big-endian variant |
//! | `<u16`, `<i32`, etc. | Explicit little-endian |
//! | `bool` | 1-byte boolean (0 = false, non-zero = true) |
//! | `b1`..`b7` | Bit field of N bits, packed MSB-first within one byte |
//! | `sN` | Fixed UTF-8 string, N bytes, zero-padded |
//! | `s` | Unbounded UTF-8 string, consumes rest of input |
//! | `p` | Pascal string: 1-byte length prefix + UTF-8 data (max 255 bytes) |
//! | `xN` | Exactly N raw bytes, hex-encoded |
//! | `x` | Unbounded raw bytes, consumes rest of input |
//! | `zN` | Skip N bytes (zeros on encode, discarded on decode; no value slot) |
//! | `[t1,t2,...]` | Group: encode/decode as a JSON array (recursive) |
//!
//! ## Quick Start
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! let types = parse_type_list("u8,u16,bool").unwrap();
//! let values = serde_json::json!([42, 1000, true]);
//! let bytes = encode_fields(&types, &values).unwrap();
//! let result = decode_fields(&types, &bytes).unwrap();
//! let arr = result.as_array().unwrap();
//!
//! assert_eq!(arr[0], serde_json::json!(42));
//! assert_eq!(arr[1], serde_json::json!(1000));
//! assert_eq!(arr[2], serde_json::json!(true));
//! ```
//!
//! ## Endianness
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // Default: little-endian. Value 256 (0x0100) stored as [0x00, 0x01].
//! let le = parse_type_list("u16").unwrap();
//! let le_bytes = encode_fields(&le, &serde_json::json!([256])).unwrap();
//! assert_eq!(le_bytes, vec![0x00, 0x01]);
//!
//! // Big-endian. Value 256 (0x0100) stored as [0x01, 0x00].
//! let be = parse_type_list(">u16").unwrap();
//! let be_bytes = encode_fields(&be, &serde_json::json!([256])).unwrap();
//! assert_eq!(be_bytes, vec![0x01, 0x00]);
//! ```
//!
//! ## Bit Fields
//!
//! Bit fields are packed MSB-first within a single byte and must not cross
//! byte boundaries (the sum of bits in a contiguous run must be <= 8).
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // b4 + b4 -> one byte
//! let types = parse_type_list("b4,b4").unwrap();
//! let bytes = encode_fields(&types, &serde_json::json!(["1010", "0101"])).unwrap();
//! assert_eq!(bytes, vec![0xA5]);
//! let result = decode_fields(&types, &bytes).unwrap();
//! let arr = result.as_array().unwrap();
//! assert_eq!(arr[0], serde_json::json!("1010"));
//! assert_eq!(arr[1], serde_json::json!("0101"));
//! ```
//!
//! ## Strings
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // Pascal string
//! let types = parse_type_list("p").unwrap();
//! let bytes = encode_fields(&types, &serde_json::json!(["hello"])).unwrap();
//! let result = decode_fields(&types, &bytes).unwrap();
//! assert_eq!(result.as_array().unwrap()[0], serde_json::json!("hello"));
//!
//! // Fixed string (zero-padded to 8 bytes)
//! let types = parse_type_list("s8").unwrap();
//! let bytes = encode_fields(&types, &serde_json::json!(["hi"])).unwrap();
//! assert_eq!(bytes.len(), 8);
//! let result = decode_fields(&types, &bytes).unwrap();
//! assert_eq!(result.as_array().unwrap()[0], serde_json::json!("hi"));
//! ```
//!
//! ## Groups
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // Group [i8, i8] produces a nested JSON array
//! let types = parse_type_list("u8,[i8,i8],u8").unwrap();
//! let values = serde_json::json!([1, [-1, 2], 3]);
//! let bytes = encode_fields(&types, &values).unwrap();
//! let result = decode_fields(&types, &bytes).unwrap();
//! let arr = result.as_array().unwrap();
//! assert_eq!(arr[0], serde_json::json!(1));
//! assert_eq!(arr[1], serde_json::json!([-1, 2]));
//! assert_eq!(arr[2], serde_json::json!(3));
//! ```
//!
//! ## Skip Fields
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // z2 skips 2 bytes - no value slot in the array
//! let types = parse_type_list("u8,z2,u8").unwrap();
//! let bytes = encode_fields(&types, &serde_json::json!([10, 20])).unwrap();
//! assert_eq!(bytes, vec![10, 0, 0, 20]);
//! let result = decode_fields(&types, &bytes).unwrap();
//! let arr = result.as_array().unwrap();
//! assert_eq!(arr.len(), 2);
//! assert_eq!(arr[0], serde_json::json!(10));
//! assert_eq!(arr[1], serde_json::json!(20));
//! ```

pub mod config;
pub mod decode;
pub mod encode;
pub mod types;
#[cfg(feature = "rquickjs")]
pub mod quickjs;

pub use decode::decode_fields;
pub use encode::{encode_fields, parse_hex};
pub use types::{parse_type, parse_type_list, Endian, FieldType};
#[cfg(feature = "rquickjs")]
pub use quickjs::register_struct;
