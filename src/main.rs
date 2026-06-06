use argh::FromArgs;
use std::io::{self, Read, Write};
use struct_cli::{
    config::{self, Config},
    decode::{decode_fields, DecodeResult},
    encode::{encode_fields, parse_hex},
    types::{parse_type, parse_type_list, FieldType},
};

// -- CLI definition

#[derive(FromArgs, Debug)]
/// Pack and unpack binary structs.
struct Args {
    #[argh(subcommand)]
    cmd: SubCommand,

    /// print effective parameters as JSON and exit without executing
    #[argh(switch)]
    dump_json: bool,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum SubCommand {
    Decode(DecodeArgs),
    Encode(EncodeArgs),
    Run(RunArgs),
    Types(TypesArgs),
}

#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "decode")]
/// Decode binary data into typed fields.
struct DecodeArgs {
    /// struct definition: comma-separated types (e.g. u8,>u16,s8)
    #[argh(option, short = 't')]
    types: Option<String>,

    /// struct definition as JSON array (e.g. '["u8",">u16","s8"]')
    #[argh(option)]
    types_json: Option<String>,

    /// hex-encoded input bytes on the command line
    #[argh(option, short = 'x')]
    hex: Option<String>,

    /// numeric value with type prefix (e.g. u64::123456)
    #[argh(option, short = 'n')]
    numeric: Option<String>,

    /// read hex-encoded bytes from stdin instead of raw bytes
    #[argh(switch)]
    stdin_hex: bool,

    /// output format: delimited (default), json, json-detailed
    #[argh(option, short = 'o', default = "\"delimited\".into()")]
    output: String,

    /// field delimiter for delimited output (default: ,)
    #[argh(option, short = 'd', default = "\",\".into()")]
    delimiter: String,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "encode")]
/// Encode typed fields into binary data.
struct EncodeArgs {
    /// struct definition: comma-separated types
    #[argh(option, short = 't')]
    types: Option<String>,

    /// struct definition as JSON array
    #[argh(option)]
    types_json: Option<String>,

    /// values as comma-separated list (positional match to types)
    #[argh(option, short = 'v')]
    values: Option<String>,

    /// values as JSON array of strings
    #[argh(option)]
    values_json: Option<String>,

    /// merged type::value pairs, comma-separated (e.g. u8::42,>u16::1000)
    #[argh(option, short = 'f')]
    fields: Option<String>,

    /// merged fields as JSON: array of {"type":...,"value":...} or {"types":[...],"values":[...]}
    #[argh(option)]
    fields_json: Option<String>,

    /// read values from stdin (requires -t/--types or --types-json)
    #[argh(switch)]
    stdin_values: bool,

    /// stdin values format: delimited (default) or json
    #[argh(option, default = "\"delimited\".into()")]
    stdin_format: String,

    /// delimiter for delimited stdin values (default: ,)
    #[argh(option)]
    stdin_delimiter: Option<String>,

    /// output format: hex (default), raw, u8, u16, u32, u64, u128
    #[argh(option, short = 'o', default = "\"hex\".into()")]
    output: String,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "run")]
/// Execute encode or decode using a saved JSON config file.
struct RunArgs {
    /// path to the JSON config file
    #[argh(positional)]
    config_file: String,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand, name = "types")]
/// Show supported field types and value syntax.
struct TypesArgs {}

// -- Config merge

fn merge_config_decode(args: &mut DecodeArgs, path: &str) -> Result<(), String> {
    let cfg = Config::from_file(path)?;
    if args.types.is_none() && args.types_json.is_none() {
        if let Some(ts) = &cfg.types {
            args.types_json = Some(serde_json::to_string(ts).unwrap());
        }
    }
    if args.hex.is_none() {
        if let Some(h) = cfg.hex_data { args.hex = Some(h); }
    }
    if args.numeric.is_none() {
        if let Some(n) = cfg.numeric { args.numeric = Some(n); }
    }
    if !args.stdin_hex && matches!(cfg.input.as_deref(), Some("stdin-hex")) {
        args.stdin_hex = true;
    }
    if args.output == "delimited" {
        if let Some(o) = cfg.output { args.output = o; }
    }
    if args.delimiter == "," {
        if let Some(d) = cfg.delimiter { args.delimiter = d; }
    }
    Ok(())
}

fn merge_config_encode(args: &mut EncodeArgs, path: &str) -> Result<(), String> {
    let cfg = Config::from_file(path)?;
    if args.types.is_none() && args.types_json.is_none() {
        if let Some(ts) = &cfg.types {
            args.types_json = Some(serde_json::to_string(ts).unwrap());
        }
    }
    if args.values.is_none() && args.values_json.is_none() {
        if let Some(vs) = &cfg.values {
            args.values_json = Some(serde_json::to_string(vs).unwrap());
        }
    }
    if args.fields.is_none() && args.fields_json.is_none() {
        if let Some(fs) = cfg.fields {
            args.fields_json = Some(serde_json::to_string(&fs).unwrap());
        }
    }
    if !args.stdin_values {
        if let Some(sv) = cfg.stdin_values { args.stdin_values = sv; }
    }
    if args.stdin_format == "delimited" {
        if let Some(sf) = cfg.stdin_format { args.stdin_format = sf; }
    }
    if args.stdin_delimiter.is_none() {
        args.stdin_delimiter = cfg.stdin_delimiter;
    }
    if args.output == "hex" {
        if let Some(o) = cfg.encode_output { args.output = o; }
    }
    Ok(())
}

// -- Parsing helpers

fn parse_types_from_args(
    types: Option<&String>,
    types_json: Option<&String>,
) -> Result<Vec<FieldType>, String> {
    if let Some(tj) = types_json {
        let v: serde_json::Value =
            serde_json::from_str(tj).map_err(|e| format!("invalid types JSON: {e}"))?;
        config::parse_json_types(&v)
    } else if let Some(ts) = types {
        parse_type_list(ts)
    } else {
        Err("no type definition provided (use -t or --types-json)".into())
    }
}

fn get_input_bytes(args: &DecodeArgs) -> Result<Vec<u8>, String> {
    if let Some(hex) = &args.hex {
        return parse_hex(hex, "hex input");
    }
    if let Some(num) = &args.numeric {
        return parse_numeric_input(num);
    }
    let mut buf = Vec::new();
    io::stdin()
        .read_to_end(&mut buf)
        .map_err(|e| format!("error reading stdin: {e}"))?;
    if args.stdin_hex {
        let hex_str = String::from_utf8(buf)
            .map_err(|e| format!("stdin hex is not valid UTF-8: {e}"))?;
        parse_hex(hex_str.trim(), "stdin hex")
    } else {
        Ok(buf)
    }
}

fn parse_numeric_input(s: &str) -> Result<Vec<u8>, String> {
    let (type_str, val_str) = s.split_once("::").ok_or_else(|| {
        format!("numeric input must be type::value (e.g. u64::123456), got: {s}")
    })?;
    let ft = parse_type(type_str)?;
    encode_fields(&[ft], &[val_str.to_string()])
}

fn get_encode_fields(args: &EncodeArgs) -> Result<(Vec<FieldType>, Vec<String>), String> {
    if let Some(fj) = &args.fields_json {
        if args.stdin_values {
            return Err("--fields-json and --stdin-values are mutually exclusive".into());
        }
        let v: serde_json::Value =
            serde_json::from_str(fj).map_err(|e| format!("invalid fields JSON: {e}"))?;
        return config::parse_json_fields(&v);
    }
    if let Some(f) = &args.fields {
        if args.stdin_values {
            return Err("--fields and --stdin-values are mutually exclusive".into());
        }
        return parse_merged_fields(f);
    }
    let types = parse_types_from_args(args.types.as_ref(), args.types_json.as_ref())?;
    if args.stdin_values {
        let values = read_stdin_values(&args.stdin_format, args.stdin_delimiter.as_deref())?;
        return Ok((types, values));
    }
    let values = if let Some(vj) = &args.values_json {
        let v: serde_json::Value =
            serde_json::from_str(vj).map_err(|e| format!("invalid values JSON: {e}"))?;
        v.as_array()
            .ok_or("values JSON must be an array")?
            .iter()
            .enumerate()
            .map(|(i, x)| {
                x.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("values[{i}] must be a string"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else if let Some(vs) = &args.values {
        vs.split(',').map(|s| s.to_string()).collect()
    } else {
        return Err(
            "no values provided (use -v, --values-json, -f, --fields-json, or --stdin-values)"
                .into(),
        );
    };
    Ok((types, values))
}

fn parse_merged_fields(s: &str) -> Result<(Vec<FieldType>, Vec<String>), String> {
    let mut types = Vec::new();
    let mut values = Vec::new();
    for (i, part) in s.split(',').enumerate() {
        let (type_str, val_str) = part.split_once("::").ok_or_else(|| {
            format!("field {i}: expected type::value, got: '{part}'")
        })?;
        types.push(parse_type(type_str).map_err(|e| format!("field {i}: {e}"))?);
        values.push(val_str.to_string());
    }
    Ok((types, values))
}

fn read_stdin_values(format: &str, delimiter: Option<&str>) -> Result<Vec<String>, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("error reading stdin: {e}"))?;
    match format {
        "json" => {
            let v: serde_json::Value = serde_json::from_str(input.trim())
                .map_err(|e| format!("invalid JSON from stdin: {e}"))?;
            v.as_array()
                .ok_or("stdin JSON must be an array")?
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    x.as_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| format!("stdin values[{i}] must be a string"))
                })
                .collect()
        }
        "delimited" => {
            let raw = delimiter.unwrap_or(",");
            let delim = raw.replace("\\n", "\n").replace("\\t", "\t");
            Ok(input.trim().split(delim.as_str()).map(|s| s.to_string()).collect())
        }
        _ => Err(format!("unknown stdin format: '{format}'. Valid: delimited, json")),
    }
}

// -- Output formatting

fn format_decode_output(
    results: &[DecodeResult],
    output: &str,
    delimiter: &str,
) -> Result<String, String> {
    match output {
        "delimited" => Ok(results
            .iter()
            .map(|r| r.value.clone())
            .collect::<Vec<_>>()
            .join(delimiter)),
        "json" => {
            let vals: Vec<serde_json::Value> = results
                .iter()
                .map(|r| serde_json::Value::String(r.value.clone()))
                .collect();
            serde_json::to_string(&vals).map_err(|e| e.to_string())
        }
        "json-detailed" => {
            let objs: Vec<serde_json::Value> = results
                .iter()
                .map(|r| serde_json::json!({ "type": r.type_name, "value": r.value }))
                .collect();
            serde_json::to_string(&objs).map_err(|e| e.to_string())
        }
        _ => Err(format!(
            "unknown decode output: '{output}'. Valid: delimited, json, json-detailed"
        )),
    }
}

fn run_encode_output(output: &str, bytes: &[u8]) -> Result<(), String> {
    if output == "raw" {
        io::stdout()
            .write_all(bytes)
            .map_err(|e| format!("write error: {e}"))?;
        return Ok(());
    }
    if output == "hex" {
        let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
        println!("{hex}");
        return Ok(());
    }
    if matches!(output, "u8" | "u16" | "u32" | "u64" | "u128") {
        let val = numeric_bytes_to_string(bytes, output)?;
        println!("{val}");
        return Ok(());
    }
    Err(format!(
        "unknown encode output: '{output}'. Valid: hex, raw, u8, u16, u32, u64, u128"
    ))
}

fn numeric_bytes_to_string(bytes: &[u8], nt: &str) -> Result<String, String> {
    macro_rules! fixed {
        ($n:literal, $ty:ty) => {{
            if bytes.len() != $n {
                return Err(format!("{nt} needs {} bytes, got {}", $n, bytes.len()));
            }
            let arr: [u8; $n] = bytes.try_into().unwrap();
            Ok(<$ty>::from_le_bytes(arr).to_string())
        }};
    }
    match nt {
        "u8" => {
            if bytes.len() != 1 {
                return Err(format!("u8 needs 1 byte, got {}", bytes.len()));
            }
            Ok(bytes[0].to_string())
        }
        "u16" => fixed!(2, u16),
        "u32" => fixed!(4, u32),
        "u64" => fixed!(8, u64),
        "u128" => fixed!(16, u128),
        _ => Err(format!(
            "unsupported numeric type: '{nt}'. Valid: u8, u16, u32, u64, u128"
        )),
    }
}

// -- Dump JSON

fn dump_decode_json(args: &DecodeArgs, types: Option<&[FieldType]>) {
    let mut cfg = Config {
        mode: Some("decode".into()),
        output: Some(args.output.clone()),
        delimiter: Some(args.delimiter.clone()),
        ..Default::default()
    };
    if let Some(ts) = types {
        cfg.types = Some(ts.iter().map(|t| t.type_name()).collect());
    }
    if let Some(h) = &args.hex { cfg.hex_data = Some(h.clone()); }
    if let Some(n) = &args.numeric { cfg.numeric = Some(n.clone()); }
    if args.stdin_hex { cfg.input = Some("stdin-hex".into()); }
    println!("{}", cfg.to_json_pretty());
}

fn dump_encode_json(args: &EncodeArgs, types: Option<&[FieldType]>, values: Option<&[String]>) {
    let mut cfg = Config {
        mode: Some("encode".into()),
        encode_output: Some(args.output.clone()),
        ..Default::default()
    };
    if let Some(ts) = types {
        cfg.types = Some(ts.iter().map(|t| t.type_name()).collect());
    }
    if let Some(vs) = values {
        cfg.values = Some(vs.to_vec());
    }
    if args.stdin_values {
        cfg.stdin_values = Some(true);
        cfg.stdin_format = Some(args.stdin_format.clone());
        cfg.stdin_delimiter = args.stdin_delimiter.clone();
    }
    println!("{}", cfg.to_json_pretty());
}

// -- Execution helpers (shared by direct subcommands and run)

fn exec_decode(da: &DecodeArgs, dump_json: bool) -> Result<(), String> {
    let types_result = parse_types_from_args(da.types.as_ref(), da.types_json.as_ref());
    if dump_json {
        dump_decode_json(da, types_result.ok().as_deref());
        return Ok(());
    }
    let types = types_result?;
    let data = get_input_bytes(da)?;
    let results = decode_fields(&types, &data)?;
    let delimiter = da.delimiter.replace("\\n", "\n").replace("\\t", "\t").replace("\\0", "\0");
    let out = format_decode_output(&results, &da.output, &delimiter)?;
    println!("{out}");
    Ok(())
}

fn exec_encode(ea: &EncodeArgs, dump_json: bool) -> Result<(), String> {
    if dump_json {
        let fields = get_encode_fields(ea).ok();
        let (types, values) = fields
            .map(|(t, v)| (Some(t), Some(v)))
            .unwrap_or((None, None));
        dump_encode_json(ea, types.as_deref(), values.as_deref());
        return Ok(());
    }
    let (types, values) = get_encode_fields(ea)?;
    let bytes = encode_fields(&types, &values)?;
    run_encode_output(&ea.output, &bytes)
}

// -- Types help

fn print_types_help() {
    println!("\
Field Types
-----------

Integers (little-endian by default; prefix > for big-endian, < for explicit LE):
  u8  u16  u32  u64  u128     Unsigned integer
  i8  i16  i32  i64  i128     Signed integer

  Endian examples: u32 (LE), >u32 (BE), <u32 (explicit LE)
  Values: decimal (42, -7) or 0x-prefixed hex (0xFF)

Boolean:
  bool                         1 byte; 0=false, non-zero=true
  Values: true/false, 1/0, yes/no

Bit fields (packed MSB-first within one byte; must not cross byte boundaries):
  b1 b2 b3 b4 b5 b6 b7        N-bit field
  Values: binary string matching N chars (e.g. 1010 for b4) or decimal/hex

Strings:
  sN                           Fixed N-byte UTF-8 field, zero-padded (e.g. s8)
  s                            Unbounded UTF-8, consumes rest of input
  p                            Pascal string: 1-byte length + data (max 255 bytes)
  Values: UTF-8 text

Raw bytes:
  xN                           Exactly N bytes, hex-encoded (e.g. x4::DEADBEEF)
  x                            Unbounded raw bytes, consumes rest of input
  Values: even-length hex string (e.g. DEADBEEF or deadbeef)

Notes:
  - All integers are big-endian when prefixed with >, little-endian otherwise.
  - Bit fields in a contiguous run must total <= 8 bits (one byte).
  - The merged field format (-f) uses commas as field separators;
    string values containing commas require -v/--values-json instead.");
}

// -- Entry point

fn main() {
    let args: Args = argh::from_env();
    if let Err(e) = run(args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    let dump_json = args.dump_json;

    match args.cmd {
        SubCommand::Decode(da) => exec_decode(&da, dump_json),

        SubCommand::Encode(ea) => exec_encode(&ea, dump_json),

        SubCommand::Run(ra) => {
            let cfg = Config::from_file(&ra.config_file)?;
            match cfg.mode.as_deref() {
                Some("decode") => {
                    let mut da = DecodeArgs {
                        types: None,
                        types_json: None,
                        hex: None,
                        numeric: None,
                        stdin_hex: false,
                        output: "delimited".into(),
                        delimiter: ",".into(),
                    };
                    merge_config_decode(&mut da, &ra.config_file)?;
                    exec_decode(&da, dump_json)
                }
                Some("encode") => {
                    let mut ea = EncodeArgs {
                        types: None,
                        types_json: None,
                        values: None,
                        values_json: None,
                        fields: None,
                        fields_json: None,
                        stdin_values: false,
                        stdin_format: "delimited".into(),
                        stdin_delimiter: None,
                        output: "hex".into(),
                    };
                    merge_config_encode(&mut ea, &ra.config_file)?;
                    exec_encode(&ea, dump_json)
                }
                Some(m) => Err(format!(
                    "unknown mode '{m}' in config file. Valid: encode, decode"
                )),
                None => Err(format!(
                    "config file '{}' must have a 'mode' field (encode or decode)",
                    ra.config_file
                )),
            }
        }

        SubCommand::Types(_) => {
            print_types_help();
            Ok(())
        }
    }
}
