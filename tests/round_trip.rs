use struct_cli::{decode_fields, encode_fields, parse_hex, parse_type, parse_type_list, Endian, FieldType};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn enc(types_str: &str, values: &[&str]) -> Vec<u8> {
    let types = parse_type_list(types_str).unwrap();
    let values: Vec<String> = values.iter().map(|s| s.to_string()).collect();
    encode_fields(&types, &values).unwrap()
}

fn dec(types_str: &str, data: &[u8]) -> Vec<String> {
    let types = parse_type_list(types_str).unwrap();
    decode_fields(&types, data)
        .unwrap()
        .into_iter()
        .map(|r| r.value)
        .collect()
}

fn round_trip(types_str: &str, values: &[&str]) -> Vec<String> {
    let bytes = enc(types_str, values);
    dec(types_str, &bytes)
}

fn enc_err(types_str: &str, values: &[&str]) -> String {
    let types = parse_type_list(types_str).unwrap();
    let values: Vec<String> = values.iter().map(|s| s.to_string()).collect();
    encode_fields(&types, &values).unwrap_err()
}

fn dec_err(types_str: &str, data: &[u8]) -> String {
    let types = parse_type_list(types_str).unwrap();
    decode_fields(&types, data).unwrap_err()
}

// ── Type parsing ──────────────────────────────────────────────────────────────

#[test]
fn parse_unsigned_types() {
    assert_eq!(parse_type("u8").unwrap(), FieldType::U8);
    assert_eq!(parse_type("u16").unwrap(), FieldType::U16(Endian::Little));
    assert_eq!(parse_type("u32").unwrap(), FieldType::U32(Endian::Little));
    assert_eq!(parse_type("u64").unwrap(), FieldType::U64(Endian::Little));
    assert_eq!(parse_type("u128").unwrap(), FieldType::U128(Endian::Little));
}

#[test]
fn parse_signed_types() {
    assert_eq!(parse_type("i8").unwrap(), FieldType::I8);
    assert_eq!(parse_type("i16").unwrap(), FieldType::I16(Endian::Little));
    assert_eq!(parse_type("i32").unwrap(), FieldType::I32(Endian::Little));
    assert_eq!(parse_type("i64").unwrap(), FieldType::I64(Endian::Little));
    assert_eq!(parse_type("i128").unwrap(), FieldType::I128(Endian::Little));
}

#[test]
fn parse_big_endian_prefix() {
    assert_eq!(parse_type(">u16").unwrap(), FieldType::U16(Endian::Big));
    assert_eq!(parse_type(">i32").unwrap(), FieldType::I32(Endian::Big));
    assert_eq!(parse_type(">u64").unwrap(), FieldType::U64(Endian::Big));
}

#[test]
fn parse_explicit_little_endian_prefix() {
    assert_eq!(parse_type("<u16").unwrap(), FieldType::U16(Endian::Little));
    assert_eq!(parse_type("<i32").unwrap(), FieldType::I32(Endian::Little));
}

#[test]
fn parse_bit_types() {
    for n in 1u8..=7 {
        assert_eq!(parse_type(&format!("b{n}")).unwrap(), FieldType::Bits(n));
    }
    assert!(parse_type("b0").is_err());
    assert!(parse_type("b8").is_err());
}

#[test]
fn parse_string_types() {
    assert_eq!(parse_type("s").unwrap(), FieldType::StringRest);
    assert_eq!(parse_type("s1").unwrap(), FieldType::StringFixed(1));
    assert_eq!(parse_type("s256").unwrap(), FieldType::StringFixed(256));
    assert_eq!(parse_type("p").unwrap(), FieldType::PascalString);
    assert!(parse_type("s0").is_err());
}

#[test]
fn parse_hex_types() {
    assert_eq!(parse_type("x").unwrap(), FieldType::HexBytesRest);
    assert_eq!(parse_type("x1").unwrap(), FieldType::HexBytes(1));
    assert_eq!(parse_type("x16").unwrap(), FieldType::HexBytes(16));
    assert!(parse_type("x0").is_err());
}

#[test]
fn parse_unknown_type_errors() {
    assert!(parse_type("z8").is_err());
    assert!(parse_type("uint").is_err());
    assert!(parse_type("").is_err());
}

#[test]
fn type_name_round_trips() {
    let cases = [
        "u8", "u16", "u32", "u64", "u128",
        "i8", "i16", "i32", "i64", "i128",
        ">u16", ">u32", ">i64",
        "bool", "b1", "b4", "b7",
        "s8", "s256", "s", "p",
        "x4", "x", "x16",
    ];
    for s in cases {
        let ft = parse_type(s).unwrap();
        assert_eq!(ft.type_name(), s, "type_name() round-trip failed for '{s}'");
    }
}

// ── u8 ────────────────────────────────────────────────────────────────────────

#[test]
fn u8_boundary_values() {
    for v in [0u8, 1, 127, 128, 254, 255] {
        assert_eq!(round_trip("u8", &[&v.to_string()]), vec![v.to_string()]);
    }
}

#[test]
fn u8_byte_layout() {
    assert_eq!(enc("u8", &["0"]), vec![0x00]);
    assert_eq!(enc("u8", &["255"]), vec![0xFF]);
}

#[test]
fn u8_hex_input() {
    assert_eq!(round_trip("u8", &["0xFF"]), vec!["255"]);
    assert_eq!(round_trip("u8", &["0x00"]), vec!["0"]);
}

#[test]
fn u8_out_of_range() {
    assert!(enc_err("u8", &["256"]).contains("out of range"));
}

// ── u16 ───────────────────────────────────────────────────────────────────────

#[test]
fn u16_little_endian_byte_order() {
    // 256 = 0x0100; LE = [0x00, 0x01]
    assert_eq!(enc("u16", &["256"]), vec![0x00, 0x01]);
    // 1000 = 0x03E8; LE = [0xE8, 0x03]
    assert_eq!(enc("u16", &["1000"]), vec![0xE8, 0x03]);
}

#[test]
fn u16_big_endian_byte_order() {
    // 256 = 0x0100; BE = [0x01, 0x00]
    assert_eq!(enc(">u16", &["256"]), vec![0x01, 0x00]);
    assert_eq!(enc(">u16", &["1000"]), vec![0x03, 0xE8]);
}

#[test]
fn u16_round_trip() {
    for v in [0u16, 1, 255, 256, 1000, 32768, 65535] {
        assert_eq!(round_trip("u16", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">u16", &[&v.to_string()]), vec![v.to_string()]);
    }
}

#[test]
fn u16_out_of_range() {
    assert!(enc_err("u16", &["65536"]).contains("out of range"));
}

// ── u32 ───────────────────────────────────────────────────────────────────────

#[test]
fn u32_little_endian_byte_order() {
    // 0x01020304 LE = [0x04, 0x03, 0x02, 0x01]
    assert_eq!(enc("u32", &["0x01020304"]), vec![0x04, 0x03, 0x02, 0x01]);
}

#[test]
fn u32_big_endian_byte_order() {
    assert_eq!(enc(">u32", &["0x01020304"]), vec![0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn u32_round_trip() {
    for v in [0u32, 1, 65535, 65536, u32::MAX] {
        assert_eq!(round_trip("u32", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">u32", &[&v.to_string()]), vec![v.to_string()]);
    }
}

// ── u64 ───────────────────────────────────────────────────────────────────────

#[test]
fn u64_round_trip() {
    for v in [0u64, 1, u32::MAX as u64 + 1, u64::MAX] {
        assert_eq!(round_trip("u64", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">u64", &[&v.to_string()]), vec![v.to_string()]);
    }
}

#[test]
fn u64_endian_bytes() {
    // value 1 LE = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    let le = enc("u64", &["1"]);
    assert_eq!(le[0], 1);
    assert_eq!(&le[1..], &[0u8; 7]);
    // BE = [0x00, ..., 0x01]
    let be = enc(">u64", &["1"]);
    assert_eq!(be[7], 1);
    assert_eq!(&be[..7], &[0u8; 7]);
}

// ── u128 ──────────────────────────────────────────────────────────────────────

#[test]
fn u128_round_trip() {
    for v in [0u128, 1, u64::MAX as u128 + 1, u128::MAX] {
        assert_eq!(round_trip("u128", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">u128", &[&v.to_string()]), vec![v.to_string()]);
    }
}

// ── i8 ────────────────────────────────────────────────────────────────────────

#[test]
fn i8_round_trip() {
    for v in [i8::MIN, -1i8, 0, 1, i8::MAX] {
        assert_eq!(round_trip("i8", &[&v.to_string()]), vec![v.to_string()]);
    }
}

#[test]
fn i8_out_of_range() {
    assert!(enc_err("i8", &["128"]).contains("out of range"));
    assert!(enc_err("i8", &["-129"]).contains("out of range"));
}

// ── i16 ───────────────────────────────────────────────────────────────────────

#[test]
fn i16_little_endian_byte_order() {
    // -1 as i16 LE = [0xFF, 0xFF]
    assert_eq!(enc("i16", &["-1"]), vec![0xFF, 0xFF]);
    // -256 = 0xFF00 as u16, LE = [0x00, 0xFF]
    assert_eq!(enc("i16", &["-256"]), vec![0x00, 0xFF]);
}

#[test]
fn i16_big_endian_byte_order() {
    assert_eq!(enc(">i16", &["-1"]), vec![0xFF, 0xFF]);
    assert_eq!(enc(">i16", &["-256"]), vec![0xFF, 0x00]);
}

#[test]
fn i16_round_trip() {
    for v in [i16::MIN, -1000i16, -1, 0, 1, 1000, i16::MAX] {
        assert_eq!(round_trip("i16", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">i16", &[&v.to_string()]), vec![v.to_string()]);
    }
}

// ── i32/i64/i128 ──────────────────────────────────────────────────────────────

#[test]
fn i32_round_trip() {
    for v in [i32::MIN, -1i32, 0, 1, i32::MAX] {
        assert_eq!(round_trip("i32", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">i32", &[&v.to_string()]), vec![v.to_string()]);
    }
}

#[test]
fn i64_round_trip() {
    for v in [i64::MIN, -1i64, 0, 1, i64::MAX] {
        assert_eq!(round_trip("i64", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">i64", &[&v.to_string()]), vec![v.to_string()]);
    }
}

#[test]
fn i128_round_trip() {
    for v in [i128::MIN, -1i128, 0, 1, i128::MAX] {
        assert_eq!(round_trip("i128", &[&v.to_string()]), vec![v.to_string()]);
        assert_eq!(round_trip(">i128", &[&v.to_string()]), vec![v.to_string()]);
    }
}

// ── bool ─────────────────────────────────────────────────────────────────────

#[test]
fn bool_values() {
    for t in ["true", "1", "yes"] {
        assert_eq!(round_trip("bool", &[t]), vec!["true"]);
    }
    for f in ["false", "0", "no"] {
        assert_eq!(round_trip("bool", &[f]), vec!["false"]);
    }
}

#[test]
fn bool_byte_layout() {
    assert_eq!(enc("bool", &["true"]), vec![1]);
    assert_eq!(enc("bool", &["false"]), vec![0]);
    // any non-zero byte decodes as true
    assert_eq!(dec("bool", &[0xFF]), vec!["true"]);
    assert_eq!(dec("bool", &[42]), vec!["true"]);
}

#[test]
fn bool_invalid_value() {
    assert!(enc_err("bool", &["maybe"]).contains("invalid bool"));
}

// ── Bit fields ────────────────────────────────────────────────────────────────

#[test]
fn bits_b4_pair_fills_one_byte() {
    // b4::1010 + b4::0101 = 0xA5
    assert_eq!(enc("b4,b4", &["1010", "0101"]), vec![0xA5]);
}

#[test]
fn bits_b1_to_b7_single_byte() {
    // b1+b7 = 8 bits
    let bytes = enc("b1,b7", &["1", "1010101"]);
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0], 0b1_1010101);
}

#[test]
fn bits_three_groups() {
    // b3+b2+b3 = 8 bits
    let bytes = enc("b3,b2,b3", &["101", "11", "010"]);
    assert_eq!(bytes.len(), 1);
    assert_eq!(bytes[0], 0b101_11_010);
}

#[test]
fn bits_round_trip_all_widths() {
    for n in 1u8..=7 {
        let type_str = format!("b{n}");
        let max_val = (1u8 << n) - 1;
        // all zeros
        let zeros = "0".repeat(n as usize);
        assert_eq!(round_trip(&type_str, &[&zeros]), vec![zeros.clone()]);
        // all ones
        let ones = "1".repeat(n as usize);
        assert_eq!(round_trip(&type_str, &[&ones]), vec![ones.clone()]);
        // max numeric value
        let val = max_val.to_string();
        let result = round_trip(&type_str, &[&val]);
        // decode gives binary string
        let expected = format!("{max_val:0width$b}", width = n as usize);
        assert_eq!(result, vec![expected]);
    }
}

#[test]
fn bits_cross_byte_boundary_encode_error() {
    assert!(enc_err("b5,b4", &["10101", "1010"]).contains("cross byte boundary"));
}

#[test]
fn bits_cross_byte_boundary_decode_error() {
    assert!(dec_err("b5,b4", &[0xFF]).contains("cross byte boundary"));
}

#[test]
fn bits_value_too_large() {
    assert!(enc_err("b3", &["8"]).contains("does not fit in 3 bits"));
}

#[test]
fn bits_followed_by_byte_field() {
    // b4 + u8: bit byte then separate byte
    let bytes = enc("b4,b4,u8", &["1111", "0000", "42"]);
    assert_eq!(bytes, vec![0xF0, 42]);
    assert_eq!(dec("b4,b4,u8", &bytes), vec!["1111", "0000", "42"]);
}

// ── Fixed string ──────────────────────────────────────────────────────────────

#[test]
fn string_fixed_exact_fill() {
    let bytes = enc("s5", &["hello"]);
    assert_eq!(bytes, b"hello");
    assert_eq!(dec("s5", &bytes), vec!["hello"]);
}

#[test]
fn string_fixed_zero_padded() {
    let bytes = enc("s8", &["hi"]);
    assert_eq!(bytes.len(), 8);
    assert_eq!(&bytes[..2], b"hi");
    assert_eq!(&bytes[2..], &[0u8; 6]);
    // decode strips null padding
    assert_eq!(dec("s8", &bytes), vec!["hi"]);
}

#[test]
fn string_fixed_all_zeros_decodes_empty() {
    let bytes = vec![0u8; 4];
    assert_eq!(dec("s4", &bytes), vec![""]);
}

#[test]
fn string_fixed_too_long() {
    assert!(enc_err("s4", &["hello"]).contains("exceeds field size"));
}

#[test]
fn string_fixed_round_trip() {
    for s in ["", "a", "hello", "test!"] {
        let padded_len = s.len().max(8);
        let type_str = format!("s{padded_len}");
        assert_eq!(round_trip(&type_str, &[s]), vec![s.to_string()]);
    }
}

// ── Rest string ───────────────────────────────────────────────────────────────

#[test]
fn string_rest_round_trip() {
    for s in ["", "hello", "hello world", "test 123!"] {
        assert_eq!(round_trip("s", &[s]), vec![s.to_string()]);
    }
}

#[test]
fn string_rest_after_fixed_field() {
    let bytes = enc("u8,s", &["5", "hello"]);
    assert_eq!(bytes, vec![5, b'h', b'e', b'l', b'l', b'o']);
    assert_eq!(dec("u8,s", &bytes), vec!["5", "hello"]);
}

// ── Pascal string ─────────────────────────────────────────────────────────────

#[test]
fn pascal_string_round_trip() {
    for s in ["", "hi", "hello world", "test 123"] {
        assert_eq!(round_trip("p", &[s]), vec![s.to_string()]);
    }
}

#[test]
fn pascal_string_byte_layout() {
    let bytes = enc("p", &["abc"]);
    assert_eq!(bytes, vec![3, b'a', b'b', b'c']);
}

#[test]
fn pascal_string_empty() {
    assert_eq!(enc("p", &[""]), vec![0]);
    assert_eq!(dec("p", &[0]), vec![""]);
}

#[test]
fn pascal_string_too_long() {
    let long = "a".repeat(256);
    assert!(enc_err("p", &[&long]).contains("too long"));
}

#[test]
fn pascal_string_max_length() {
    let s = "x".repeat(255);
    assert_eq!(round_trip("p", &[&s]), vec![s]);
}

// ── Hex bytes ─────────────────────────────────────────────────────────────────

#[test]
fn hex_bytes_fixed_round_trip() {
    assert_eq!(round_trip("x4", &["DEADBEEF"]), vec!["DEADBEEF"]);
    assert_eq!(round_trip("x4", &["deadbeef"]), vec!["DEADBEEF"]);
    assert_eq!(round_trip("x1", &["AA"]), vec!["AA"]);
}

#[test]
fn hex_bytes_fixed_wrong_length() {
    assert!(enc_err("x4", &["AABB"]).contains("expected 4 hex bytes"));
}

#[test]
fn hex_bytes_rest_round_trip() {
    for v in ["", "AABB", "DEADBEEFCAFE"] {
        assert_eq!(round_trip("x", &[v]), vec![v.to_string()]);
    }
}

#[test]
fn hex_bytes_rest_after_field() {
    let bytes = enc("u8,x", &["1", "AABBCC"]);
    assert_eq!(bytes, vec![1, 0xAA, 0xBB, 0xCC]);
    assert_eq!(dec("u8,x", &bytes), vec!["1", "AABBCC"]);
}

#[test]
fn hex_odd_length_error() {
    assert!(parse_hex("ABC", "test").unwrap_err().contains("even number"));
}

#[test]
fn hex_invalid_chars() {
    assert!(parse_hex("ZZZZ", "test").is_err());
}

// ── Mixed struct round-trips ──────────────────────────────────────────────────

#[test]
fn mixed_basic_struct() {
    let types = "u8,u16,bool";
    let values = ["42", "1000", "true"];
    assert_eq!(round_trip(types, &values), vec!["42", "1000", "true"]);
}

#[test]
fn mixed_big_endian_struct() {
    let types = ">u8,>u16,bool";
    let values = ["42", "1000", "true"];
    assert_eq!(round_trip(types, &values), vec!["42", "1000", "true"]);
}

#[test]
fn mixed_endian_struct() {
    // first field LE, second BE
    let bytes = enc("u16,>u16", &["1", "1"]);
    // LE 1 = [0x01, 0x00], BE 1 = [0x00, 0x01]
    assert_eq!(bytes, vec![0x01, 0x00, 0x00, 0x01]);
    let result = dec("u16,>u16", &bytes);
    assert_eq!(result, vec!["1", "1"]);
}

#[test]
fn mixed_with_strings() {
    let types = "u8,s8,p,u16";
    let values = ["99", "hello", "world", "12345"];
    assert_eq!(round_trip(types, &values), vec!["99", "hello", "world", "12345"]);
}

#[test]
fn mixed_with_bits_and_ints() {
    let types = "b4,b4,u8,u16";
    let values = ["1010", "0101", "255", "1000"];
    let result = round_trip(types, &values);
    assert_eq!(result, vec!["1010", "0101", "255", "1000"]);
}

#[test]
fn mixed_with_hex_bytes() {
    let types = "u8,x4,u16";
    let values = ["1", "DEADBEEF", "500"];
    assert_eq!(round_trip(types, &values), vec!["1", "DEADBEEF", "500"]);
}

#[test]
fn rest_field_at_end() {
    let types = "u8,u16,s";
    let values = ["1", "2", "rest of the data"];
    assert_eq!(round_trip(types, &values), vec!["1", "2", "rest of the data"]);
}

#[test]
fn hex_rest_at_end() {
    let types = "u8,x";
    let values = ["255", "CAFEBABE"];
    assert_eq!(round_trip(types, &values), vec!["255", "CAFEBABE"]);
}

// ── Error cases ───────────────────────────────────────────────────────────────

#[test]
fn error_type_value_count_mismatch() {
    let types = parse_type_list("u8,u16").unwrap();
    let values = vec!["42".to_string()];
    let err = encode_fields(&types, &values).unwrap_err();
    assert!(err.contains("type count"), "got: {err}");
}

#[test]
fn error_truncated_u16() {
    assert!(dec_err("u16", &[0xAA]).contains("need 2 bytes"));
}

#[test]
fn error_truncated_u32() {
    assert!(dec_err("u32", &[0x01, 0x02]).contains("need 4 bytes"));
}

#[test]
fn error_truncated_u64() {
    assert!(dec_err("u64", &[0u8; 4]).contains("need 8 bytes"));
}

#[test]
fn error_truncated_bit_field() {
    assert!(dec_err("b4", &[]).contains("end of data"));
}

#[test]
fn error_bad_type_name() {
    assert!(parse_type("xyz").is_err());
    assert!(parse_type("uint").is_err());
    assert!(parse_type(">bool").is_ok()); // prefix ignored on non-numeric type
}

#[test]
fn error_invalid_hex_input() {
    assert!(parse_hex("GG", "test").is_err());
    assert!(parse_hex("A", "test").is_err()); // odd length
}

#[test]
fn error_u32_out_of_range() {
    assert!(enc_err("u32", &["4294967296"]).contains("out of range"));
}

#[test]
fn error_i8_boundary() {
    assert_eq!(round_trip("i8", &["127"]), vec!["127"]);
    assert_eq!(round_trip("i8", &["-128"]), vec!["-128"]);
    assert!(enc_err("i8", &["128"]).contains("out of range"));
    assert!(enc_err("i8", &["-129"]).contains("out of range"));
}

// ── Specific byte-order verification ─────────────────────────────────────────

#[test]
fn byte_order_u16_le_vs_be() {
    let le = enc("u16", &["256"]);
    let be = enc(">u16", &["256"]);
    // 256 = 0x0100
    assert_eq!(le, vec![0x00, 0x01]); // LE: low byte first
    assert_eq!(be, vec![0x01, 0x00]); // BE: high byte first
}

#[test]
fn byte_order_u32_le_vs_be() {
    let le = enc("u32", &["0x01020304"]);
    let be = enc(">u32", &["0x01020304"]);
    assert_eq!(le, vec![0x04, 0x03, 0x02, 0x01]);
    assert_eq!(be, vec![0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn byte_order_i32_negative() {
    // -1 in two's complement is all 0xFF bytes regardless of endianness
    let le = enc("i32", &["-1"]);
    let be = enc(">i32", &["-1"]);
    assert_eq!(le, vec![0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(be, vec![0xFF, 0xFF, 0xFF, 0xFF]);
    // -256 = 0xFFFFFF00; LE = [0x00, 0xFF, 0xFF, 0xFF]; BE = [0xFF, 0xFF, 0xFF, 0x00]
    let le = enc("i32", &["-256"]);
    let be = enc(">i32", &["-256"]);
    assert_eq!(le, vec![0x00, 0xFF, 0xFF, 0xFF]);
    assert_eq!(be, vec![0xFF, 0xFF, 0xFF, 0x00]);
}

// ── parse_hex ─────────────────────────────────────────────────────────────────

#[test]
fn parse_hex_uppercase() {
    assert_eq!(parse_hex("DEADBEEF", "t").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn parse_hex_lowercase() {
    assert_eq!(parse_hex("deadbeef", "t").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
}

#[test]
fn parse_hex_empty() {
    assert_eq!(parse_hex("", "t").unwrap(), Vec::<u8>::new());
}

#[test]
fn parse_hex_strips_whitespace() {
    assert_eq!(parse_hex("  AABB  ", "t").unwrap(), vec![0xAA, 0xBB]);
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_string_rest() {
    assert_eq!(enc("s", &[""]), Vec::<u8>::new());
    assert_eq!(dec("s", &[]), vec![""]);
}

#[test]
fn multiple_bit_field_bytes() {
    // two separate bit-field bytes: b8 not valid; use b4+b4 twice
    let types = "b4,b4,b3,b5";
    let values = ["1111", "0000", "101", "10101"];
    let bytes = enc(types, &values);
    assert_eq!(bytes.len(), 2);
    assert_eq!(bytes[0], 0xF0);
    assert_eq!(bytes[1], 0b101_10101);
    assert_eq!(dec(types, &bytes), vec!["1111", "0000", "101", "10101"]);
}

#[test]
fn hex_in_numeric_value() {
    assert_eq!(round_trip("u8", &["0xAB"]), vec!["171"]);
    assert_eq!(round_trip("u32", &["0xDEADBEEF"]), vec!["3735928559"]);
}

#[test]
fn decode_result_type_names_match() {
    let types = parse_type_list("u8,>u16,b4,s4").unwrap();
    let bytes = enc("u8,>u16,b4,b4,s4", &["1", "2", "1010", "0101", "hi"]);
    // use matching types for decode
    let types2 = parse_type_list("u8,>u16,b4,b4,s4").unwrap();
    let results = decode_fields(&types2, &bytes).unwrap();
    assert_eq!(results[0].type_name, "u8");
    assert_eq!(results[1].type_name, ">u16");
    assert_eq!(results[2].type_name, "b4");
    drop(types);
}
