//! # struct-cli
//!
//! A library for packing and unpacking binary structs from typed field definitions.
//!
//! Fields are described using type strings. Encoding and decoding support
//! unsigned and signed integers, booleans, bit fields, fixed and unbounded
//! strings, pascal strings, and raw byte sequences.
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
//!
//! ## Quick Start
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! let types = parse_type_list("u8,u16,bool").unwrap();
//! let values = vec!["42".to_string(), "1000".to_string(), "true".to_string()];
//! let bytes = encode_fields(&types, &values).unwrap();
//! let results = decode_fields(&types, &bytes).unwrap();
//!
//! assert_eq!(results[0].value, "42");
//! assert_eq!(results[1].value, "1000");
//! assert_eq!(results[2].value, "true");
//! ```
//!
//! ## Endianness
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // Default: little-endian. Value 256 (0x0100) stored as [0x00, 0x01].
//! let le = parse_type_list("u16").unwrap();
//! let le_bytes = encode_fields(&le, &["256".to_string()]).unwrap();
//! assert_eq!(le_bytes, vec![0x00, 0x01]);
//!
//! // Big-endian. Value 256 (0x0100) stored as [0x01, 0x00].
//! let be = parse_type_list(">u16").unwrap();
//! let be_bytes = encode_fields(&be, &["256".to_string()]).unwrap();
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
//! let bytes = encode_fields(&types, &["1010".to_string(), "0101".to_string()]).unwrap();
//! assert_eq!(bytes, vec![0xA5]);
//! let results = decode_fields(&types, &bytes).unwrap();
//! assert_eq!(results[0].value, "1010");
//! assert_eq!(results[1].value, "0101");
//! ```
//!
//! ## Strings
//!
//! ```
//! use struct_cli::{encode_fields, decode_fields, parse_type_list};
//!
//! // Pascal string
//! let types = parse_type_list("p").unwrap();
//! let bytes = encode_fields(&types, &["hello".to_string()]).unwrap();
//! let results = decode_fields(&types, &bytes).unwrap();
//! assert_eq!(results[0].value, "hello");
//!
//! // Fixed string (zero-padded to 8 bytes)
//! let types = parse_type_list("s8").unwrap();
//! let bytes = encode_fields(&types, &["hi".to_string()]).unwrap();
//! assert_eq!(bytes.len(), 8);
//! let results = decode_fields(&types, &bytes).unwrap();
//! assert_eq!(results[0].value, "hi");
//! ```

pub mod config;
pub mod decode;
pub mod encode;
pub mod types;

pub use decode::{decode_fields, DecodeResult};
pub use encode::{encode_fields, parse_hex};
pub use types::{parse_type, parse_type_list, Endian, FieldType};
