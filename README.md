# struct-cli

Pack and unpack binary structs from the command line or from Rust code.

## Field Types

Run `struct-cli types` for a full reference. Summary:

| Type | Description |
|------|-------------|
| `u8`/`u16`/`u32`/`u64`/`u128` | Unsigned integer |
| `i8`/`i16`/`i32`/`i64`/`i128` | Signed integer |
| `bool` | 1-byte boolean (0=false, non-zero=true) |
| `b1`..`b7` | Bit field of N bits, packed MSB-first within one byte |
| `sN` | Fixed N-byte UTF-8 string, zero-padded |
| `s` | Unbounded UTF-8 string, consumes rest of input |
| `p` | Pascal string: 1-byte length prefix + data (max 255 bytes) |
| `xN` | Exactly N raw bytes, hex-encoded |
| `x` | Unbounded raw bytes, consumes rest of input |

## Endianness

All numeric types default to **little-endian**. Prefix a type with `>` for big-endian or `<` for explicit little-endian.

```
u16    little-endian (default)
>u16   big-endian
<u16   explicit little-endian
```

Endianness applies to multi-byte integer types. Single-byte types (`u8`, `i8`, `bool`) and non-numeric types are unaffected.

## CLI Usage

```
Usage: struct-cli [--dump-json] <command> [<args>]

Pack and unpack binary structs.

Options:
  --dump-json       print effective parameters as JSON and exit without
                    executing
  --help, help      display usage information

Commands:
  decode            Decode binary data into typed fields.
  encode            Encode typed fields into binary data.
  run               Execute encode or decode using a saved JSON config file.
  types             Show supported field types and value syntax.
```

### decode

```
Usage: struct-cli decode [-t <types>] [--types-json <types-json>] [-x <hex>] [-n <numeric>] [--stdin-hex] [-o <output>] [-d <delimiter>]

Decode binary data into typed fields.

Options:
  -t, --types       struct definition: comma-separated types (e.g. u8,>u16,s8)
  --types-json      struct definition as JSON array (e.g. '["u8",">u16","s8"]')
  -x, --hex         hex-encoded input bytes on the command line
  -n, --numeric     numeric value with type prefix (e.g. u64::123456)
  --stdin-hex       read hex-encoded bytes from stdin instead of raw bytes
  -o, --output      output format: delimited (default), json, json-detailed
  -d, --delimiter   field delimiter for delimited output (default: ,)
  --help, help      display usage information
```

Input priority: `-x` (hex string) > `-n` (numeric) > stdin (raw by default, hex with `--stdin-hex`).

Output formats:
- `delimited` - values separated by delimiter (default `,`)
- `json` - JSON array of strings
- `json-detailed` - JSON array of `{"type": ..., "value": ...}` objects

### encode

```
Usage: struct-cli encode [-t <types>] [--types-json <types-json>] [-v <values>] [--values-json <values-json>] [-f <fields>] [--fields-json <fields-json>] [--stdin-values] [--stdin-format <stdin-format>] [--stdin-delimiter <stdin-delimiter>] [-o <output>]

Encode typed fields into binary data.

Options:
  -t, --types       struct definition: comma-separated types
  --types-json      struct definition as JSON array
  -v, --values      values as comma-separated list (positional match to types)
  --values-json     values as JSON array of strings
  -f, --fields      merged type::value pairs, comma-separated (e.g.
                    u8::42,>u16::1000)
  --fields-json     merged fields as JSON: array of {"type":...,"value":...}
                    or {"types":[...],"values":[...]}
  --stdin-values    read values from stdin (requires -t/--types or --types-json)
  --stdin-format    stdin values format: delimited (default) or json
  --stdin-delimiter delimiter for delimited stdin values (default: ,)
  -o, --output      output format: hex (default), raw, u8, u16, u32, u64, u128
  --help, help      display usage information
```

Input priority: `--fields-json` > `-f` (merged fields) > `-t`/`-v` (separate types+values).

Note: the `-f` merged format uses commas to separate fields. String values containing commas require `-t`/`-v` or `--fields-json` instead.

Output formats:
- `hex` - uppercase hex to stdout (default)
- `raw` - raw bytes to stdout
- `u8`/`u16`/`u32`/`u64`/`u128` - interpret bytes as little-endian and print decimal

### types

```
struct-cli types
```

Prints a reference of all supported field types and their value syntax.

### run

```
Usage: struct-cli run <config-file>
```

Reads a JSON config file and executes the encode or decode operation it
describes. The config must have a `mode` field set to `"encode"` or `"decode"`.

## JSON Config

Generate a config file from any command with `--dump-json` (a root-level flag
that must appear before the subcommand name):

```sh
struct-cli --dump-json decode -t "u8,>u16,bool" -x "2A03E801" -o json > my-struct.json
```

```json
{
  "mode": "decode",
  "types": ["u8", ">u16", "bool"],
  "hex_data": "2A03E801",
  "output": "json",
  "delimiter": ","
}
```

Then run it directly:

```sh
struct-cli run my-struct.json
```

`--dump-json` also works with `run` to inspect the loaded config before executing:

```sh
struct-cli --dump-json run my-struct.json
```

Config file keys: `mode`, `types`, `values`, `fields`, `stdin_values`, `stdin_format`, `stdin_delimiter`, `input`, `hex_data`, `numeric`, `output`, `delimiter`, `encode_output`.

A full encode config example:

```json
{
  "mode": "encode",
  "types": ["u8", ">u16", "bool"],
  "values": ["42", "1000", "true"],
  "encode_output": "hex"
}
```

## Examples

### Encode

Merged type::value format:

```sh
struct-cli encode -f "u8::42,u16::1000,bool::true"
# 2AE80301
```

Separate types and values:

```sh
struct-cli encode -t "u8,u16,bool" -v "42,1000,true"
# 2AE80301
```

Big-endian fields:

```sh
struct-cli encode -f "u8::42,>u16::1000,bool::true"
# 2A03E801
```

Mixed endianness:

```sh
struct-cli encode -f "u16::1,>u16::1"
# 01000100
```

Values from stdin:

```sh
echo "42,1000,true" | struct-cli encode -t "u8,u16,bool" --stdin-values
printf "42\n1000\ntrue" | struct-cli encode -t "u8,u16,bool" --stdin-values --stdin-delimiter '\n'
echo '["42","1000","true"]' | struct-cli encode -t "u8,u16,bool" --stdin-values --stdin-format json
```

JSON fields input:

```sh
struct-cli encode --fields-json '[{"type":"u8","value":"42"},{"type":">u16","value":"1000"}]'
struct-cli encode --fields-json '{"types":["u8",">u16"],"values":["42","1000"]}'
```

Numeric output:

```sh
struct-cli encode -f "u32::305419896" -o u32
# 305419896
```

### Decode

From hex on command line:

```sh
struct-cli decode -t "u8,u16,bool" -x "2AE80301"
# 42,1000,true

struct-cli decode -t "u8,>u16,bool" -x "2A03E801"
# 42,1000,true
```

Custom delimiter:

```sh
struct-cli decode -t "u8,u16,bool" -x "2AE80301" -d "|"
# 42|1000|true
```

JSON output:

```sh
struct-cli decode -t "u8,u16,bool" -x "2AE80301" -o json
# ["42","1000","true"]

struct-cli decode -t "u8,u16,bool" -x "2AE80301" -o json-detailed
# [{"type":"u8","value":"42"},{"type":"u16","value":"1000"},{"type":"bool","value":"true"}]
```

Hex from stdin:

```sh
echo "2AE80301" | struct-cli decode -t "u8,u16,bool" --stdin-hex
```

Raw bytes from stdin (pipe from encode):

```sh
struct-cli encode -f "u8::42,u16::1000" | struct-cli decode -t "u8,u16"
# 42,1000
```

Numeric input:

```sh
struct-cli decode -t "u32" -n "u32::305419896"
# 305419896
```

### Bit fields

Bit fields are packed MSB-first within a byte. A contiguous run of bit fields must total 8 bits or fewer.

```sh
struct-cli encode -f "b4::1010,b4::0101"
# A5

struct-cli decode -t "b4,b4" -x "A5"
# 1010,0101

struct-cli encode -f "b3::101,b2::11,b3::010"
# BA

struct-cli decode -t "b3,b2,b3" -x "BA"
# 101,11,010
```

### Strings

```sh
# Fixed-length string (zero-padded to 8 bytes)
struct-cli encode -f "s8::hello"
struct-cli decode -t "s8" -x "68656C6C6F000000"
# hello

# Pascal string
struct-cli encode -f "p::hello world"
struct-cli decode -t "p" -x "0B68656C6C6F20776F726C64"
# hello world

# Signed integer and rest string
struct-cli encode -f "i16::-1234,s::payload data"
struct-cli decode -t "i16,s" -x "2EFBCE7061796C6F61642064617461"
# -1234,payload data
```

### Hex bytes

```sh
struct-cli encode -f "x4::DEADBEEF"
# DEADBEEF

struct-cli decode -t "x4" -x "DEADBEEF"
# DEADBEEF

# Unbounded hex at end
struct-cli encode -f "u8::1,x::CAFEBABE"
struct-cli decode -t "u8,x" -x "01CAFEBABE"
# 1,CAFEBABE
```

## Library API

Add to `Cargo.toml`:

```toml
[dependencies]
struct-cli = { path = "..." }
```

Core functions:

```rust
use struct_cli::{encode_fields, decode_fields, parse_type_list, DecodeResult};

// Encode
let types = parse_type_list("u8,>u16,bool").unwrap();
let values = vec!["42".to_string(), "1000".to_string(), "true".to_string()];
let bytes: Vec<u8> = encode_fields(&types, &values).unwrap();

// Decode
let results: Vec<DecodeResult> = decode_fields(&types, &bytes).unwrap();
for r in &results {
    println!("{}: {}", r.type_name, r.value);
}
```

See `cargo doc --open` for full API documentation.
