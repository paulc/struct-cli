#![cfg(feature = "rquickjs")]

use rquickjs::{Context, Runtime};
use struct_cli::register_struct;

fn with_ctx<F: FnOnce(rquickjs::Ctx<'_>)>(f: F) {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();
    ctx.with(f);
}

#[test]
fn encode_basic() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let result: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("u8,u16,bool", [42, 1000, true])"#)
            .unwrap();
        assert_eq!(result.as_bytes().unwrap(), &[0x2A, 0xE8, 0x03, 0x01]);
    });
}

#[test]
fn decode_basic() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        // Build an ArrayBuffer from bytes, then decode
        let bytes: rquickjs::ArrayBuffer = ctx
            .eval(r#"new Uint8Array([0x2A, 0xE8, 0x03, 0x01]).buffer"#)
            .unwrap();
        ctx.globals().set("buf", bytes).unwrap();
        let result: String = ctx
            .eval(r#"JSON.stringify(decodeFields("u8,u16,bool", buf))"#)
            .unwrap();
        assert_eq!(result, "[42,1000,true]");
    });
}

#[test]
fn encode_typelist_as_array() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let result: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields(["u8",">u16"], [42, 1000])"#)
            .unwrap();
        assert_eq!(result.as_bytes().unwrap(), &[0x2A, 0x03, 0xE8]);
    });
}

#[test]
fn encode_hex_field_arraybuffer() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        // x4 field from ArrayBuffer
        let result: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("u8,x4", [1, new Uint8Array([0xDE,0xAD,0xBE,0xEF]).buffer])"#)
            .unwrap();
        assert_eq!(
            result.as_bytes().unwrap(),
            &[0x01, 0xDE, 0xAD, 0xBE, 0xEF]
        );
    });
}

#[test]
fn encode_hex_field_hex_string() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let result: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("u8,x4", [1, "DEADBEEF"])"#)
            .unwrap();
        assert_eq!(
            result.as_bytes().unwrap(),
            &[0x01, 0xDE, 0xAD, 0xBE, 0xEF]
        );
    });
}

#[test]
fn decode_hex_field_returns_arraybuffer() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let bytes: rquickjs::ArrayBuffer = ctx
            .eval(r#"new Uint8Array([0x01, 0xDE, 0xAD, 0xBE, 0xEF]).buffer"#)
            .unwrap();
        ctx.globals().set("buf", bytes).unwrap();
        // x4 field should decode to ArrayBuffer; check its byte length
        let len: i32 = ctx
            .eval(r#"decodeFields("u8,x4", buf)[1].byteLength"#)
            .unwrap();
        assert_eq!(len, 4);
        let first_byte: i32 = ctx
            .eval(r#"new Uint8Array(decodeFields("u8,x4", buf)[1])[0]"#)
            .unwrap();
        assert_eq!(first_byte, 0xDE);
    });
}

#[test]
fn encode_decode_skip_fields() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        // z2 is transparent - only 2 values needed
        let encoded: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("u8,z2,u8", [10, 20])"#)
            .unwrap();
        assert_eq!(encoded.as_bytes().unwrap(), &[10, 0, 0, 20]);

        ctx.globals().set("buf", encoded).unwrap();
        let result: String = ctx
            .eval(r#"JSON.stringify(decodeFields("u8,z2,u8", buf))"#)
            .unwrap();
        assert_eq!(result, "[10,20]");
    });
}

#[test]
fn encode_decode_group() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let encoded: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("u8,[i8,i8],u8", [1, [-1, 2], 3])"#)
            .unwrap();
        assert_eq!(encoded.as_bytes().unwrap(), &[0x01, 0xFF, 0x02, 0x03]);

        ctx.globals().set("buf", encoded).unwrap();
        let result: String = ctx
            .eval(r#"JSON.stringify(decodeFields("u8,[i8,i8],u8", buf))"#)
            .unwrap();
        assert_eq!(result, "[1,[-1,2],3]");
    });
}

#[test]
fn encode_decode_group_typelist_as_array() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let encoded: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields(["u8",["i8","i8"],"u8"], [1, [-1, 2], 3])"#)
            .unwrap();
        assert_eq!(encoded.as_bytes().unwrap(), &[0x01, 0xFF, 0x02, 0x03]);

        ctx.globals().set("buf", encoded).unwrap();
        let result: String = ctx
            .eval(r#"JSON.stringify(decodeFields(["u8",["i8","i8"],"u8"], buf))"#)
            .unwrap();
        assert_eq!(result, "[1,[-1,2],3]");
    });
}

#[test]
fn encode_decode_bit_fields() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        // b4 fields are binary strings
        let encoded: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("b4,b4", ["1010", "0101"])"#)
            .unwrap();
        assert_eq!(encoded.as_bytes().unwrap(), &[0xA5]);

        ctx.globals().set("buf", encoded).unwrap();
        let result: String = ctx
            .eval(r#"JSON.stringify(decodeFields("b4,b4", buf))"#)
            .unwrap();
        // bit fields decode to integers: 0b1010=10, 0b0101=5
        assert_eq!(result, "[10,5]");
    });
}

#[test]
fn encode_decode_u128_bigint() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        // u128 uses BigInt in JS
        let encoded: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("u128", [1n])"#)
            .unwrap();
        let bytes = encoded.as_bytes().unwrap();
        assert_eq!(bytes[0], 1); // little-endian: byte 0 = 1
        assert_eq!(&bytes[1..], &[0u8; 15]);

        ctx.globals().set("buf", encoded).unwrap();
        // Decode should return BigInt - check with typeof
        let type_str: String = ctx
            .eval(r#"typeof decodeFields("u128", buf)[0]"#)
            .unwrap();
        assert_eq!(type_str, "bigint");

        let val: String = ctx
            .eval(r#"String(decodeFields("u128", buf)[0])"#)
            .unwrap();
        assert_eq!(val, "1");
    });
}

#[test]
fn encode_decode_i128_bigint() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        // i128 negative value
        let encoded: rquickjs::ArrayBuffer = ctx
            .eval(r#"encodeFields("i128", [-1n])"#)
            .unwrap();
        assert_eq!(encoded.as_bytes().unwrap(), &[0xFFu8; 16]);

        ctx.globals().set("buf", encoded).unwrap();
        let type_str: String = ctx
            .eval(r#"typeof decodeFields("i128", buf)[0]"#)
            .unwrap();
        assert_eq!(type_str, "bigint");

        let val: String = ctx
            .eval(r#"String(decodeFields("i128", buf)[0])"#)
            .unwrap();
        assert_eq!(val, "-1");
    });
}

#[test]
fn encode_returns_arraybuffer() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let is_arraybuffer: bool = ctx
            .eval(r#"encodeFields("u8", [42]) instanceof ArrayBuffer"#)
            .unwrap();
        assert!(is_arraybuffer);
    });
}

#[test]
fn encode_error_bad_type() {
    with_ctx(|ctx| {
        register_struct(&ctx).unwrap();
        let caught: bool = ctx
            .eval(r#"
                try { encodeFields("notatype", [1]); false }
                catch(e) { true }
            "#)
            .unwrap();
        assert!(caught);
    });
}
