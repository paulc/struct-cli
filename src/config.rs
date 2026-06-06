use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::types::{FieldType, parse_type};

/// JSON-serialisable representation of all command parameters.
///
/// Used for `--dump-json` output and `-c`/`--config` input.
/// Keys that are `None` are omitted from serialisation.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Type list as strings (e.g. `["u8", ">u16", "s8"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,

    // Encode inputs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<TypeValue>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_values: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_format: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdin_delimiter: Option<String>,

    // Decode input
    /// `"stdin-raw"`, `"stdin-hex"`, `"hex"`, or `"numeric"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub hex_data: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric: Option<String>,

    // Decode output
    /// `"delimited"`, `"json"`, or `"json-detailed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,

    // Encode output: `"hex"`, `"raw"`, or `"numeric::u64"` etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encode_output: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TypeValue {
    #[serde(rename = "type")]
    pub type_name: String,
    pub value: String,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read config file '{path}': {e}"))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("invalid JSON in '{path}': {e}"))
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }
}

/// Parse a JSON array of type strings into `FieldType` values.
pub fn parse_json_types(v: &Value) -> Result<Vec<FieldType>, String> {
    let arr = v.as_array().ok_or("types must be a JSON array")?;
    arr.iter()
        .enumerate()
        .map(|(i, item)| {
            let s = item.as_str().ok_or_else(|| format!("types[{i}] must be a string"))?;
            parse_type(s).map_err(|e| format!("types[{i}]: {e}"))
        })
        .collect()
}

/// Parse JSON fields into `(types, values)`. Accepts two formats:
///
/// - Array of `{"type": "...", "value": "..."}` objects
/// - Object `{"types": [...], "values": [...]}`
pub fn parse_json_fields(v: &Value) -> Result<(Vec<FieldType>, Vec<String>), String> {
    if let Some(arr) = v.as_array() {
        let mut types = Vec::new();
        let mut values = Vec::new();
        for (i, item) in arr.iter().enumerate() {
            let ts = item["type"]
                .as_str()
                .ok_or_else(|| format!("fields[{i}].type must be a string"))?;
            let vs = item["value"]
                .as_str()
                .ok_or_else(|| format!("fields[{i}].value must be a string"))?;
            types.push(parse_type(ts).map_err(|e| format!("fields[{i}]: {e}"))?);
            values.push(vs.to_string());
        }
        Ok((types, values))
    } else if v.is_object() {
        let tv = v.get("types").ok_or("fields object must have a 'types' key")?;
        let vv = v.get("values").ok_or("fields object must have a 'values' key")?;
        let types = parse_json_types(tv)?;
        let values = vv
            .as_array()
            .ok_or("values must be a JSON array")?
            .iter()
            .enumerate()
            .map(|(i, x)| {
                x.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("values[{i}] must be a string"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((types, values))
    } else {
        Err("fields must be a JSON array or object".into())
    }
}
