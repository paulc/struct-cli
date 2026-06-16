use rquickjs::{Array, ArrayBuffer, Ctx, Exception, Function, IntoJs, String as JsString, Type, Value};

use crate::{
    config::parse_json_types,
    decode::decode_fields,
    encode::encode_fields,
    types::{parse_type_list, FieldType},
};

/// Register `encodeFields` and `decodeFields` as global functions in the given QuickJS context.
///
/// `encodeFields(typeList, data)`:
/// - `typeList` - struct type definition as a string (e.g. `"u8,[i8,i8],z1"`) or a nested JS
///   array (same format as `--types-json`)
/// - `data` - JS array of values; `xN`/`x` fields accept `ArrayBuffer` or hex string; `u128`/`i128`
///   fields accept `BigInt` or string
/// - Returns `ArrayBuffer`
///
/// `decodeFields(typeList, data)`:
/// - `typeList` - same as encode
/// - `data` - `ArrayBuffer` containing the binary data
/// - Returns a JS array; `xN`/`x` fields become `ArrayBuffer`, `u128`/`i128` become `BigInt`,
///   bit fields remain binary strings, group fields become nested arrays
pub fn register_struct(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    globals.set("encodeFields", Function::new(ctx.clone(), js_encode)?)?;
    globals.set("decodeFields", Function::new(ctx.clone(), js_decode)?)?;
    Ok(())
}

fn js_encode<'js>(
    ctx: Ctx<'js>,
    type_list: Value<'js>,
    data: Value<'js>,
) -> rquickjs::Result<ArrayBuffer<'js>> {
    let types = parse_types_from_js(&ctx, &type_list)?;
    let json_vals = js_encode_array_to_json(&ctx, &data, &types)?;
    let bytes = encode_fields(&types, &json_vals).map_err(|e| js_error(&ctx, &e))?;
    ArrayBuffer::new(ctx, bytes)
}

fn js_decode<'js>(
    ctx: Ctx<'js>,
    type_list: Value<'js>,
    data: ArrayBuffer<'js>,
) -> rquickjs::Result<Array<'js>> {
    let types = parse_types_from_js(&ctx, &type_list)?;
    let bytes = data
        .as_bytes()
        .ok_or_else(|| js_error(&ctx, "ArrayBuffer is detached"))?;
    let json_result = decode_fields(&types, bytes).map_err(|e| js_error(&ctx, &e))?;
    let json_arr = json_result
        .as_array()
        .ok_or_else(|| js_error(&ctx, "internal error: decode did not return array"))?
        .clone();
    json_array_to_js(&ctx, &json_arr, &types)
}

fn parse_types_from_js<'js>(ctx: &Ctx<'js>, val: &Value<'js>) -> rquickjs::Result<Vec<FieldType>> {
    if val.is_string() {
        let s: String = val.get()?;
        parse_type_list(&s).map_err(|e| js_error(ctx, &e))
    } else if val.is_array() {
        let json_val = js_type_array_to_serde_json(ctx, val)?;
        parse_json_types(&json_val).map_err(|e| js_error(ctx, &e))
    } else {
        Err(js_error(ctx, "typeList must be a string or array"))
    }
}

fn js_type_array_to_serde_json<'js>(
    ctx: &Ctx<'js>,
    val: &Value<'js>,
) -> rquickjs::Result<serde_json::Value> {
    let arr = val
        .clone()
        .into_array()
        .ok_or_else(|| js_error(ctx, "type list must be an array"))?;
    let mut result = Vec::new();
    for i in 0..arr.len() {
        let elem: Value = arr.get(i)?;
        if elem.is_string() {
            let s: String = elem.get()?;
            result.push(serde_json::Value::String(s));
        } else if elem.is_array() {
            result.push(js_type_array_to_serde_json(ctx, &elem)?);
        } else {
            return Err(js_error(ctx, "type list elements must be strings or arrays"));
        }
    }
    Ok(serde_json::Value::Array(result))
}

fn js_encode_array_to_json<'js>(
    ctx: &Ctx<'js>,
    val: &Value<'js>,
    types: &[FieldType],
) -> rquickjs::Result<serde_json::Value> {
    let arr = val
        .clone()
        .into_array()
        .ok_or_else(|| js_error(ctx, "encode data must be an array"))?;

    let expected = non_skip_count(types);
    if arr.len() != expected {
        return Err(js_error(
            ctx,
            &format!("expected {} values, got {}", expected, arr.len()),
        ));
    }

    let mut result = Vec::new();
    let mut arr_idx = 0usize;
    for ft in types {
        if matches!(ft, FieldType::Skip(_)) {
            continue;
        }
        let elem: Value = arr.get(arr_idx)?;
        arr_idx += 1;
        result.push(js_encode_val_to_json(ctx, &elem, ft)?);
    }

    Ok(serde_json::Value::Array(result))
}

fn js_encode_val_to_json<'js>(
    ctx: &Ctx<'js>,
    val: &Value<'js>,
    ft: &FieldType,
) -> rquickjs::Result<serde_json::Value> {
    match ft {
        FieldType::HexBytes(_) | FieldType::HexBytesRest => {
            if val.is_string() {
                let s: String = val.get()?;
                Ok(serde_json::Value::String(s))
            } else {
                let buf: ArrayBuffer = val
                    .get()
                    .map_err(|_| js_error(ctx, "xN/x field expects ArrayBuffer or hex string"))?;
                let bytes = buf
                    .as_bytes()
                    .ok_or_else(|| js_error(ctx, "ArrayBuffer is detached"))?;
                let hex: String = bytes.iter().map(|b| format!("{:02X}", b)).collect();
                Ok(serde_json::Value::String(hex))
            }
        }
        FieldType::U128(_) | FieldType::I128(_) => match val.type_of() {
            Type::BigInt => {
                let s = bigint_to_decimal_string(ctx, val)?;
                Ok(serde_json::Value::String(s))
            }
            Type::String => {
                let s: String = val.get()?;
                Ok(serde_json::Value::String(s))
            }
            Type::Int => {
                let n: i32 = val.get()?;
                Ok(serde_json::Value::String(n.to_string()))
            }
            Type::Float => {
                let n: f64 = val.get()?;
                Ok(serde_json::Value::String(format!("{}", n as i64)))
            }
            _ => Err(js_error(ctx, "u128/i128 field expects BigInt or string")),
        },
        FieldType::Group(inner) => js_encode_array_to_json(ctx, val, inner),
        _ => js_generic_to_json(ctx, val),
    }
}

fn js_generic_to_json<'js>(
    ctx: &Ctx<'js>,
    val: &Value<'js>,
) -> rquickjs::Result<serde_json::Value> {
    match val.type_of() {
        Type::Bool => {
            let b: bool = val.get()?;
            Ok(serde_json::Value::Bool(b))
        }
        Type::Int => {
            let n: i32 = val.get()?;
            Ok(serde_json::json!(n))
        }
        Type::Float => {
            let n: f64 = val.get()?;
            if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
                Ok(serde_json::json!(n as i64))
            } else {
                Ok(serde_json::json!(n))
            }
        }
        Type::String => {
            let s: String = val.get()?;
            Ok(serde_json::Value::String(s))
        }
        _ => Err(js_error(ctx, "unsupported value type for encode field")),
    }
}

fn json_array_to_js<'js>(
    ctx: &Ctx<'js>,
    vals: &[serde_json::Value],
    types: &[FieldType],
) -> rquickjs::Result<Array<'js>> {
    let js_arr = Array::new(ctx.clone())?;
    let mut val_idx = 0usize;
    let mut js_idx = 0usize;

    for ft in types {
        if matches!(ft, FieldType::Skip(_)) {
            continue;
        }
        let val = &vals[val_idx];
        val_idx += 1;
        let js_val = json_to_js_val(ctx, val, ft)?;
        js_arr.set(js_idx, js_val)?;
        js_idx += 1;
    }

    Ok(js_arr)
}

fn json_to_js_val<'js>(
    ctx: &Ctx<'js>,
    val: &serde_json::Value,
    ft: &FieldType,
) -> rquickjs::Result<Value<'js>> {
    match ft {
        FieldType::HexBytes(_) | FieldType::HexBytesRest => {
            let hex = val
                .as_str()
                .ok_or_else(|| js_error(ctx, "internal: hex value not a string"))?;
            let bytes = parse_hex_bytes(hex);
            ArrayBuffer::new(ctx.clone(), bytes)?.into_js(ctx)
        }
        FieldType::U128(_) | FieldType::I128(_) => {
            let decimal = val
                .as_str()
                .ok_or_else(|| js_error(ctx, "internal: u128/i128 value not a string"))?;
            decimal_to_bigint(ctx, decimal)
        }
        FieldType::Group(inner) => {
            let arr_vals = val
                .as_array()
                .ok_or_else(|| js_error(ctx, "internal: group value not array"))?;
            json_array_to_js(ctx, arr_vals, inner)?.into_js(ctx)
        }
        FieldType::Bits(_) => {
            let s = val
                .as_str()
                .ok_or_else(|| js_error(ctx, "internal: bits value not a string"))?;
            s.into_js(ctx)
        }
        _ => json_generic_to_js(ctx, val),
    }
}

fn json_generic_to_js<'js>(
    ctx: &Ctx<'js>,
    val: &serde_json::Value,
) -> rquickjs::Result<Value<'js>> {
    match val {
        serde_json::Value::Bool(b) => b.into_js(ctx),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_js(ctx)
            } else if let Some(u) = n.as_u64() {
                (u as i64).into_js(ctx)
            } else if let Some(f) = n.as_f64() {
                f.into_js(ctx)
            } else {
                Err(js_error(ctx, "cannot convert number to JS"))
            }
        }
        serde_json::Value::String(s) => s.as_str().into_js(ctx),
        _ => Err(js_error(ctx, "unsupported JSON value for decode output")),
    }
}

fn bigint_to_decimal_string<'js>(ctx: &Ctx<'js>, val: &Value<'js>) -> rquickjs::Result<String> {
    let globals = ctx.globals();
    let string_fn: Function = globals.get("String")?;
    let s: JsString = string_fn.call((val.clone(),))?;
    s.to_string()
}

fn decimal_to_bigint<'js>(ctx: &Ctx<'js>, decimal: &str) -> rquickjs::Result<Value<'js>> {
    let globals = ctx.globals();
    let bigint_fn: Function = globals.get("BigInt")?;
    bigint_fn.call((decimal,))
}

fn parse_hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0))
        .collect()
}

fn js_error<'js>(ctx: &Ctx<'js>, msg: &str) -> rquickjs::Error {
    Exception::throw_type(ctx, msg)
}

fn non_skip_count(types: &[FieldType]) -> usize {
    types
        .iter()
        .filter(|t| !matches!(t, FieldType::Skip(_)))
        .count()
}
