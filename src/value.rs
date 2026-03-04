//! Value module - Custom value type with eager parsing
//!
//! Replaces the serde_value::Value type alias with a custom enum
//! that supports physical quantities, time systems, tables, markdown,
//! and project metadata in later phases.
//!
//! Phase 1: Core scalars (Float, Integer, Bool, String) and compound types (Map, Seq).
//! Phase 2: Quantity integration (Value::Quantity with eager parsing).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Serialize};
use serde_yaml::Deserializer;

use crate::error::VVError;
use crate::source::{self, FileSource, OutputSink};
use crate::units::{self, Unit};

// =============================================================================
// Annotation Support Types
// =============================================================================

/// Type of annotation on a data value or markdown line.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum AnnotationType {
    Comment,
    Question,
    Issue,
    Suggestion { suggested: String },
}

/// Status of an annotation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Status {
    Open,
    InProgress,
    Resolved,
    Accepted,
    Rejected,
}

/// A reply to an annotation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Reply {
    pub author: String,
    pub text: String,
    pub created: String,
}

/// Deserializable annotation for data values (from RON sidecar files).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename = "Annotation")]
pub struct AnnotationData {
    pub ann_type: AnnotationType,
    pub author: String,
    pub text: String,
    pub status: Status,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub replies: Vec<Reply>,
}

/// Deserializable annotation for markdown lines (from RON sidecar files).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename = "MarkdownAnnotation")]
pub struct MarkdownAnnotationData {
    pub ann_type: AnnotationType,
    pub author: String,
    pub text: String,
    pub status: Status,
    pub line: usize,
    #[serde(default)]
    pub line_end: Option<usize>,
    #[serde(default)]
    pub char_start: Option<usize>,
    #[serde(default)]
    pub char_end: Option<usize>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub replies: Vec<Reply>,
}

// =============================================================================
// Value Enum
// =============================================================================

/// Custom value type for all data formats.
///
/// All formats are eagerly parsed into this type at load time.
/// Keys prefixed with `_` in Maps are filtered out during serialization
/// to Minijinja (non-serializing metadata convention).
#[derive(Debug, Clone)]
pub enum Value {
    // === Scalars ===
    Float(f64),
    Integer(i64),
    Bool(bool),
    String(String),

    // === Physical quantity (float-only; integers auto-promote to f64) ===
    Quantity {
        value: f64,
        unit: Unit,
    },

    // === Time (f64 days after J2000.0 epoch = 2000-01-01 12:00:00 TT) ===
    Utc(f64),
    Tdb(f64),

    // === Table (first-class, heterogeneous cells) ===
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<Value>>,
    },

    // === Markdown (raw content + optional YAML front matter) ===
    Markdown {
        content: String,
        front_matter: Option<Box<Value>>,
    },

    // === Non-serializing metadata (stored under `_`-prefixed keys) ===
    Source {
        path: PathBuf,
    },
    Annotation(AnnotationData),
    MarkdownAnnotation(MarkdownAnnotationData),

    // === Compound ===
    Map(HashMap<String, Value>),
    Seq(Vec<Value>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Integer(a), Value::Integer(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (
                Value::Quantity {
                    value: v1,
                    unit: u1,
                },
                Value::Quantity {
                    value: v2,
                    unit: u2,
                },
            ) => v1 == v2 && u1 == u2,
            (Value::Utc(a), Value::Utc(b)) => a == b,
            (Value::Tdb(a), Value::Tdb(b)) => a == b,
            (
                Value::Table {
                    headers: h1,
                    rows: r1,
                },
                Value::Table {
                    headers: h2,
                    rows: r2,
                },
            ) => h1 == h2 && r1 == r2,
            (
                Value::Markdown {
                    content: c1,
                    front_matter: f1,
                },
                Value::Markdown {
                    content: c2,
                    front_matter: f2,
                },
            ) => c1 == c2 && f1 == f2,
            (Value::Source { path: a }, Value::Source { path: b }) => a == b,
            (Value::Annotation(a), Value::Annotation(b)) => a == b,
            (Value::MarkdownAnnotation(a), Value::MarkdownAnnotation(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Seq(a), Value::Seq(b)) => a == b,
            _ => false,
        }
    }
}

// =============================================================================
// Serialize Implementation (with `_`-prefix filtering)
// =============================================================================

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::Integer(i) => serializer.serialize_i64(*i),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::String(s) => serializer.serialize_str(s),
            Value::Quantity { value, unit } => {
                // Serialize as string "value unit" for template rendering
                serializer.serialize_str(&format!("{} {}", value, unit))
            }
            Value::Source { .. } | Value::Annotation(_) | Value::MarkdownAnnotation(_) => {
                // Non-serializing metadata — normally under `_`-prefixed keys
                // and filtered out by Map serialization, but handle gracefully
                serializer.serialize_unit()
            }
            Value::Utc(days) => {
                serializer.serialize_str(&crate::time::j2000_days_to_utc_string(*days))
            }
            Value::Tdb(days) => {
                serializer.serialize_str(&crate::time::j2000_days_to_tdb_string(*days))
            }
            Value::Markdown {
                content,
                front_matter,
            } => {
                let mut len = 1;
                if front_matter.is_some() {
                    len += 1;
                }
                let mut m = serializer.serialize_map(Some(len))?;
                m.serialize_entry("content", content)?;
                if let Some(fm) = front_matter {
                    m.serialize_entry("front_matter", fm.as_ref())?;
                }
                m.end()
            }
            Value::Table { headers, rows } => {
                // Serialize as backward-compatible {row: [...], col: {...}} structure
                let mut m = serializer.serialize_map(Some(2))?;

                // "row" key: array of row arrays
                let row_values: Vec<&Vec<Value>> = rows.iter().collect();
                m.serialize_entry("row", &row_values)?;

                // "col" key: map of column name → array of column values
                let mut col_map = HashMap::new();
                for (i, header) in headers.iter().enumerate() {
                    let col_values: Vec<&Value> =
                        rows.iter().filter_map(|row| row.get(i)).collect();
                    col_map.insert(header.as_str(), col_values);
                }
                m.serialize_entry("col", &col_map)?;

                // "headers" key: array of column names
                m.serialize_entry("headers", headers)?;

                m.end()
            }
            Value::Map(map) => {
                // Filter out `_`-prefixed keys (non-serializing metadata)
                let visible: Vec<(&String, &Value)> =
                    map.iter().filter(|(k, _)| !k.starts_with('_')).collect();
                let mut m = serializer.serialize_map(Some(visible.len()))?;
                for (k, v) in visible {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
            Value::Seq(seq) => {
                let mut s = serializer.serialize_seq(Some(seq.len()))?;
                for item in seq {
                    s.serialize_element(item)?;
                }
                s.end()
            }
        }
    }
}

// =============================================================================
// Conversion from serde_value::Value
// =============================================================================

/// Converts a serde_value::Value into our custom Value enum.
///
/// Normalizes all integer types to i64, all float types to f64.
/// Maps with non-string keys have their keys converted to strings.
///
/// Eager parsing rules:
/// - Strings are tested for quantity patterns ("100 N", "5.5 km/s") → `Value::Quantity`
/// - Maps with exactly `{value: N, unit: "X"}` → `Value::Quantity`
/// - All other values are converted to their base types
pub fn from_serde_value(sv: serde_value::Value) -> Value {
    match sv {
        // Integers -> Value::Integer(i64)
        serde_value::Value::U8(v) => Value::Integer(v as i64),
        serde_value::Value::U16(v) => Value::Integer(v as i64),
        serde_value::Value::U32(v) => Value::Integer(v as i64),
        serde_value::Value::U64(v) => Value::Integer(v as i64),
        serde_value::Value::I8(v) => Value::Integer(v as i64),
        serde_value::Value::I16(v) => Value::Integer(v as i64),
        serde_value::Value::I32(v) => Value::Integer(v as i64),
        serde_value::Value::I64(v) => Value::Integer(v),

        // Floats -> Value::Float(f64)
        serde_value::Value::F32(v) => Value::Float(v as f64),
        serde_value::Value::F64(v) => Value::Float(v),

        // Bool -> Value::Bool
        serde_value::Value::Bool(v) => Value::Bool(v),

        // String/Char -> try quantity parsing first, then fall back to String
        serde_value::Value::String(s) => try_parse_string(s),
        serde_value::Value::Char(c) => Value::String(c.to_string()),

        // Unit/None -> empty string (best effort)
        serde_value::Value::Unit => Value::String(String::new()),
        serde_value::Value::Option(None) => Value::String(String::new()),
        serde_value::Value::Option(Some(v)) => from_serde_value(*v),

        // Bytes -> string (best effort)
        serde_value::Value::Bytes(bytes) => {
            Value::String(String::from_utf8_lossy(&bytes).to_string())
        }

        // Newtype -> unwrap
        serde_value::Value::Newtype(v) => from_serde_value(*v),

        // Sequences -> Value::Seq
        serde_value::Value::Seq(seq) => Value::Seq(seq.into_iter().map(from_serde_value).collect()),

        // Maps -> check for structured quantity, then convert normally
        serde_value::Value::Map(map) => {
            // First convert to HashMap<String, Value> so we can inspect keys
            let converted: HashMap<String, Value> = map
                .into_iter()
                .map(|(k, v)| {
                    let key = serde_value_key_to_string(k);
                    let val = from_serde_value(v);
                    (key, val)
                })
                .collect();

            // Check for structured quantity notation: {value: N, unit: "X"}
            if let Some(qty) = try_parse_structured_quantity(&converted) {
                return qty;
            }

            Value::Map(converted)
        }
    }
}

/// Tries to parse a string as a time epoch or quantity, falling back to Value::String.
///
/// Parsing order:
/// 1. Time epoch: strings ending with ` UTC`, ` TDB`, ` TT`, ` TAI` → `Value::Utc` or `Value::Tdb`
/// 2. Quantity: "100 N", "5.5 km/s", "-3.14 rad" → `Value::Quantity`
/// 3. All other strings → `Value::String`
fn try_parse_string(s: String) -> Value {
    // Try to parse as time epoch (suffix-based: UTC, TDB, TT, TAI)
    if let Some(Ok(parsed)) = crate::time::try_parse_epoch(&s) {
        return match parsed.system {
            crate::time::TimeSystem::Utc | crate::time::TimeSystem::Tai => {
                Value::Utc(parsed.days_j2000)
            }
            crate::time::TimeSystem::Tdb | crate::time::TimeSystem::Tt => {
                Value::Tdb(parsed.days_j2000)
            }
        };
    }

    // Try to parse as quantity
    if let Ok(qty) = units::parse_quantity(&s) {
        return Value::Quantity {
            value: qty.value,
            unit: qty.unit,
        };
    }

    Value::String(s)
}

/// Tries to parse a structured map as a quantity.
///
/// Recognized pattern: `{value: N, unit: "X"}` where:
/// - `value` is a Float or Integer
/// - `unit` is a String that parses as a valid unit
/// - The map has exactly these two keys
fn try_parse_structured_quantity(map: &HashMap<String, Value>) -> Option<Value> {
    if map.len() != 2 {
        return None;
    }

    let val = map.get("value")?;
    let unit_str = match map.get("unit")? {
        Value::String(s) => s,
        _ => return None,
    };

    let numeric = match val {
        Value::Float(f) => *f,
        Value::Integer(i) => *i as f64,
        _ => return None,
    };

    let unit = units::parse_unit(unit_str).ok()?;

    Some(Value::Quantity {
        value: numeric,
        unit,
    })
}

/// Converts a serde_value::Value used as a map key to a String.
fn serde_value_key_to_string(key: serde_value::Value) -> String {
    match key {
        serde_value::Value::String(s) => s,
        serde_value::Value::I64(i) => i.to_string(),
        serde_value::Value::U64(u) => u.to_string(),
        serde_value::Value::Bool(b) => b.to_string(),
        serde_value::Value::F64(f) => f.to_string(),
        serde_value::Value::Char(c) => c.to_string(),
        other => format!("{:?}", other),
    }
}

// =============================================================================
// Pure Functions for File Type Detection
// =============================================================================

/// Supported data file extensions (text, binary, and tabular formats)
const DATA_EXTENSIONS: &[&str] = &[
    // Text formats
    "json", "toml", "ron", "yaml", "yml", // Binary formats
    "msgpack", "mp", "pickle", "pkl", "cbor", "bson", // Tabular formats
    "csv", "xlsx", // Markdown
    "md",
];

/// Checks if a path has a supported data file extension
pub fn is_supported_data_file_type(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| DATA_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Extracts the file extension as a string
fn get_extension(path: &Path) -> Option<&str> {
    path.extension().and_then(|ext| ext.to_str())
}

// =============================================================================
// File Loading
// =============================================================================

/// Type alias for loader functions
type Loader = fn(&Path) -> Result<Value, VVError>;

/// Maps extensions to their loader functions
fn get_loader(ext: &str) -> Option<Loader> {
    match ext {
        // Text formats
        "json" => Some(load_json),
        "toml" => Some(load_toml),
        "ron" => Some(load_ron),
        "yaml" | "yml" => Some(load_yaml),
        // Binary formats
        "msgpack" | "mp" => Some(load_msgpack),
        "pickle" | "pkl" => Some(load_pickle),
        "cbor" => Some(load_cbor),
        "bson" => Some(load_bson),
        // Tabular formats
        "csv" => Some(load_csv),
        "xlsx" => Some(load_xlsx),
        // Markdown
        "md" => Some(load_markdown),
        _ => None,
    }
}

/// Loads a value from a file path, attaching `_source` provenance to Map values.
pub fn load_file(path: &Path) -> Result<Value, VVError> {
    let mut value = get_extension(path)
        .and_then(get_loader)
        .map(|loader| loader(path))
        .unwrap_or_else(|| Err(VVError::UnsupportedFileType(path.to_path_buf())))?;
    attach_source(&mut value, path);
    Ok(value)
}

/// Loads a value from a FileSource, attaching `_source` provenance and annotations.
pub fn load_from_source(source: &FileSource, path: &Path) -> Result<Value, VVError> {
    let ext = get_extension(path).ok_or_else(|| VVError::NoValidExtension(path.to_path_buf()))?;
    let bytes = source::read_file(source, path)?;
    let mut value = parse_bytes(&bytes, ext, path)?;
    attach_source(&mut value, path);
    load_and_merge_annotations(&mut value, source, path);
    Ok(value)
}

/// Attaches `_source` provenance metadata to Map values.
///
/// Inserts a `_source` key with the file path into Map values.
/// Non-Map values (scalars, sequences, tables) are not modified.
fn attach_source(value: &mut Value, path: &Path) {
    if let Value::Map(map) = value {
        map.insert(
            "_source".to_string(),
            Value::Source {
                path: path.to_path_buf(),
            },
        );
    }
}

// =============================================================================
// Annotation Sidecar Loading
// =============================================================================

/// Returns the annotation sidecar path for a data file.
///
/// For data files: `propulsion.yaml` → `propulsion.annotations.ron`
/// For markdown: `document.md` → `document.md.annotations.ron`
fn annotation_sidecar_path(data_path: &Path) -> PathBuf {
    let ext = data_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "md" {
        // Markdown: append .annotations.ron to full filename
        let mut p = data_path.as_os_str().to_owned();
        p.push(".annotations.ron");
        PathBuf::from(p)
    } else {
        // Data files: replace extension with .annotations.ron
        let stem = data_path.file_stem().unwrap_or_default();
        let ann_filename = format!("{}.annotations.ron", stem.to_str().unwrap_or(""));
        data_path.with_file_name(ann_filename)
    }
}

/// Checks if a filename is an annotation sidecar file.
pub fn is_annotation_sidecar(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.ends_with(".annotations.ron"))
        .unwrap_or(false)
}

/// Parses a data annotation sidecar file and merges into the data value.
///
/// The sidecar contains a RON HashMap mapping data keys to annotation lists.
/// Annotations are stored as a `_annotations` Map inside the data value.
pub fn load_and_merge_annotations(value: &mut Value, source: &FileSource, data_path: &Path) {
    let sidecar = annotation_sidecar_path(data_path);
    if !source::is_file(source, &sidecar) {
        return;
    }

    let bytes = match source::read_file(source, &sidecar) {
        Ok(b) => b,
        Err(_) => return,
    };

    let ext = data_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "md" {
        merge_markdown_annotations(value, &bytes);
    } else {
        merge_data_annotations(value, &bytes);
    }
}

/// Merges data annotations from a sidecar into a Value::Map.
fn merge_data_annotations(value: &mut Value, bytes: &[u8]) {
    let content = match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => return,
    };

    let annotations: HashMap<String, Vec<AnnotationData>> = match ron::from_str(&content) {
        Ok(a) => a,
        Err(_) => return,
    };

    if let Value::Map(map) = value {
        // Build _annotations Map: key → Seq of Annotation values
        let mut ann_map = HashMap::new();
        for (key, ann_list) in annotations {
            let ann_values: Vec<Value> = ann_list.into_iter().map(Value::Annotation).collect();
            ann_map.insert(key, Value::Seq(ann_values));
        }
        if !ann_map.is_empty() {
            map.insert("_annotations".to_string(), Value::Map(ann_map));
        }
    }
}

/// Merges markdown annotations from a sidecar into a Value::Markdown.
///
/// Stores as a `_markdown_annotations` Seq inside the value if it's a Map,
/// or alongside the Markdown content in the parent Map.
fn merge_markdown_annotations(value: &mut Value, bytes: &[u8]) {
    let content = match String::from_utf8(bytes.to_vec()) {
        Ok(s) => s,
        Err(_) => return,
    };

    let annotations: Vec<MarkdownAnnotationData> = match ron::from_str(&content) {
        Ok(a) => a,
        Err(_) => return,
    };

    // For Markdown values, we can't insert keys directly (not a Map).
    // The caller should handle this at the directory level.
    // For now, store as a conversion helper — the merge happens in node.rs.
    if annotations.is_empty() {
        return;
    }

    // If the value was somehow wrapped in a Map (e.g., from DataTree), merge there.
    // Otherwise this is a no-op — markdown annotation merging happens at the parent level.
    let _ = (value, annotations);
}

/// Parses markdown annotation sidecar bytes into a Value::Seq of MarkdownAnnotation.
pub fn parse_markdown_annotations(bytes: &[u8]) -> Option<Value> {
    let content = String::from_utf8(bytes.to_vec()).ok()?;
    let annotations: Vec<MarkdownAnnotationData> = ron::from_str(&content).ok()?;
    if annotations.is_empty() {
        return None;
    }
    let values: Vec<Value> = annotations
        .into_iter()
        .map(Value::MarkdownAnnotation)
        .collect();
    Some(Value::Seq(values))
}

/// Parses bytes into a Value based on the file extension
fn parse_bytes(bytes: &[u8], ext: &str, path: &Path) -> Result<Value, VVError> {
    match ext {
        // Text formats (need string conversion)
        "json" => parse_json(bytes),
        "toml" => parse_toml(bytes),
        "ron" => parse_ron(bytes),
        "yaml" | "yml" => parse_yaml(bytes),
        // Binary formats
        "msgpack" | "mp" => parse_msgpack(bytes),
        "pickle" | "pkl" => parse_pickle(bytes),
        "cbor" => parse_cbor(bytes),
        "bson" => parse_bson(bytes),
        // Tabular formats
        "csv" => parse_csv(bytes),
        "xlsx" => parse_xlsx(bytes),
        // Markdown
        "md" => parse_markdown(bytes),
        _ => Err(VVError::UnsupportedFileType(path.to_path_buf())),
    }
}

// =============================================================================
// Parse Functions (Pure: bytes -> Value)
// =============================================================================

/// Parses JSON bytes into a Value
fn parse_json(bytes: &[u8]) -> Result<Value, VVError> {
    let content = String::from_utf8(bytes.to_vec())?;
    let raw_json: serde_json::Value = serde_json::from_str(&content)?;
    let sv = serde_value::to_value(&raw_json)?;
    Ok(from_serde_value(sv))
}

/// Parses YAML bytes into a Value (handles front matter)
fn parse_yaml(bytes: &[u8]) -> Result<Value, VVError> {
    let content = String::from_utf8(bytes.to_vec())?;

    // Check if this looks like front matter format
    if let Some(yaml_content) = extract_front_matter(&content) {
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml_content)?;
        let sv = serde_value::to_value(&value)?;
        return Ok(from_serde_value(sv));
    }

    // Standard YAML parsing (may be multi-document)
    let docs: Vec<serde_yaml::Value> = Deserializer::from_str(&content)
        .map(serde_yaml::Value::deserialize)
        .collect::<Result<Vec<_>, _>>()?;

    match docs.len() {
        0 => Err(VVError::EmptyDataFile("yaml".into())),
        1 => {
            let sv = serde_value::to_value(&docs[0])?;
            Ok(from_serde_value(sv))
        }
        _ => {
            let sv = serde_value::to_value(&docs)?;
            Ok(from_serde_value(sv))
        }
    }
}

/// Parses TOML bytes into a Value
fn parse_toml(bytes: &[u8]) -> Result<Value, VVError> {
    let content = String::from_utf8(bytes.to_vec())?;
    let raw_toml: toml::Value = toml::from_str(&content)?;
    let sv = serde_value::to_value(&raw_toml)?;
    Ok(from_serde_value(sv))
}

/// Parses RON bytes into a Value
fn parse_ron(bytes: &[u8]) -> Result<Value, VVError> {
    let content = String::from_utf8(bytes.to_vec())?;
    let raw_ron: ron::Value = ron::from_str(&content)?;
    let sv = serde_value::to_value(&raw_ron)?;
    Ok(from_serde_value(sv))
}

/// Parses MessagePack bytes into a Value
fn parse_msgpack(bytes: &[u8]) -> Result<Value, VVError> {
    let sv: serde_value::Value = rmp_serde::from_slice(bytes)?;
    Ok(from_serde_value(sv))
}

/// Parses Pickle bytes into a Value
fn parse_pickle(bytes: &[u8]) -> Result<Value, VVError> {
    let sv: serde_value::Value = serde_pickle::from_slice(bytes, Default::default())?;
    Ok(from_serde_value(sv))
}

/// Parses CBOR bytes into a Value
fn parse_cbor(bytes: &[u8]) -> Result<Value, VVError> {
    let sv: serde_value::Value = ciborium::from_reader(bytes)?;
    Ok(from_serde_value(sv))
}

/// Parses BSON bytes into a Value
fn parse_bson(bytes: &[u8]) -> Result<Value, VVError> {
    let doc: bson::Document = bson::from_slice(bytes)?;
    let sv = serde_value::to_value(&doc)?;
    Ok(from_serde_value(sv))
}

/// Parses CSV bytes into a Value::Table
fn parse_csv(bytes: &[u8]) -> Result<Value, VVError> {
    use std::io::Cursor;

    let mut reader = csv::Reader::from_reader(Cursor::new(bytes));
    let headers: Vec<String> = reader.headers()?.iter().map(sanitize_column_name).collect();

    let mut rows: Vec<Vec<Value>> = Vec::new();

    for result in reader.records() {
        let record = result?;
        let row: Vec<Value> = record.iter().map(infer_csv_value_type).collect();
        rows.push(row);
    }

    Ok(Value::Table { headers, rows })
}

/// Parses XLSX bytes into a Value
fn parse_xlsx(bytes: &[u8]) -> Result<Value, VVError> {
    use std::io::Cursor;

    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    // 1. Load shared strings table
    let shared_strings = load_shared_strings(&mut archive).unwrap_or_default();

    // 2. Get sheet names and their XML paths
    let sheets = get_sheet_paths(&mut archive)?;

    // 3. Parse each sheet into row/col structure
    let mut result = HashMap::new();
    for (sheet_name, xml_path) in sheets {
        let sheet_data = parse_worksheet(&mut archive, &xml_path, &shared_strings)?;
        let sanitized_name = sanitize_column_name(&sheet_name);
        result.insert(sanitized_name, sheet_data);
    }

    Ok(Value::Map(result))
}

// =============================================================================
// Text Format Loaders
// =============================================================================

/// Loads JSON file and normalizes to Value
fn load_json(path: &Path) -> Result<Value, VVError> {
    let content = fs::read_to_string(path)?;
    let raw_json: serde_json::Value = serde_json::from_str(&content)?;
    let sv = serde_value::to_value(&raw_json)?;
    Ok(from_serde_value(sv))
}

/// Loads YAML file (handles multi-document YAML and YAML+markdown front matter)
fn load_yaml(path: &Path) -> Result<Value, VVError> {
    let content = fs::read_to_string(path)?;

    // Check if this looks like front matter format
    if let Some(yaml_content) = extract_front_matter(&content) {
        let value: serde_yaml::Value = serde_yaml::from_str(&yaml_content)?;
        let sv = serde_value::to_value(&value)?;
        return Ok(from_serde_value(sv));
    }

    // Standard YAML parsing (may be multi-document)
    let docs: Vec<serde_yaml::Value> = Deserializer::from_str(&content)
        .map(serde_yaml::Value::deserialize)
        .collect::<Result<Vec<_>, _>>()?;

    match docs.len() {
        0 => Err(VVError::EmptyDataFile(path.into())),
        1 => {
            let sv = serde_value::to_value(&docs[0])?;
            Ok(from_serde_value(sv))
        }
        _ => {
            let sv = serde_value::to_value(&docs)?;
            Ok(from_serde_value(sv))
        }
    }
}

/// Extracts YAML front matter content from a file that may have trailing markdown.
fn extract_front_matter(content: &str) -> Option<String> {
    extract_front_matter_and_body(content).map(|(yaml, _)| yaml)
}

// =============================================================================
// Markdown Loader
// =============================================================================

/// Loads a Markdown file with optional YAML front matter.
fn load_markdown(path: &Path) -> Result<Value, VVError> {
    let bytes = fs::read(path)?;
    parse_markdown(&bytes)
}

/// Parses Markdown bytes into a Value::Markdown.
///
/// If the content has YAML front matter (delimited by `---`), the front matter
/// is parsed as YAML and stored in `front_matter`, while the remaining body
/// becomes `content`. Otherwise, the entire content is stored as `content`.
fn parse_markdown(bytes: &[u8]) -> Result<Value, VVError> {
    let content = String::from_utf8(bytes.to_vec())?;

    if let Some((yaml_str, body)) = extract_front_matter_and_body(&content) {
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&yaml_str)?;
        let sv = serde_value::to_value(&yaml_value)?;
        let front_matter = from_serde_value(sv);
        Ok(Value::Markdown {
            content: body,
            front_matter: Some(Box::new(front_matter)),
        })
    } else {
        Ok(Value::Markdown {
            content,
            front_matter: None,
        })
    }
}

/// Extracts YAML front matter and body content from a markdown file.
///
/// Returns (yaml_content, body_content) if front matter is present.
fn extract_front_matter_and_body(content: &str) -> Option<(String, String)> {
    let trimmed = content.trim_start();

    // Front matter must start with ---
    if !trimmed.starts_with("---") {
        return None;
    }

    // Skip the first --- and any trailing whitespace on that line
    let after_first = trimmed[3..].trim_start_matches([' ', '\t']);
    let after_first = if let Some(stripped) = after_first.strip_prefix('\n') {
        stripped
    } else if let Some(stripped) = after_first.strip_prefix("\r\n") {
        stripped
    } else {
        return None;
    };

    // Find the closing --- delimiter (must be at start of a line)
    let second_delimiter_pos = after_first
        .find("\n---")
        .or_else(|| after_first.find("\r\n---"))?;

    let yaml_content = &after_first[..second_delimiter_pos];

    if !yaml_content.contains(':') {
        return None;
    }

    // Extract the body after the closing ---
    let after_yaml = &after_first[second_delimiter_pos..];
    let body_start = if after_yaml.starts_with("\r\n---") {
        5 // \r\n---
    } else {
        4 // \n---
    };
    let body = &after_yaml[body_start..];
    // Skip any remaining chars on the --- line and the newline
    let body = body.trim_start_matches([' ', '\t']);
    let body = if let Some(stripped) = body.strip_prefix('\n') {
        stripped
    } else if let Some(stripped) = body.strip_prefix("\r\n") {
        stripped
    } else {
        body
    };

    Some((yaml_content.to_string(), body.to_string()))
}

/// Loads TOML file and normalizes to Value
fn load_toml(path: &Path) -> Result<Value, VVError> {
    let content = fs::read_to_string(path)?;
    let raw_toml: toml::Value = toml::from_str(&content)?;
    let sv = serde_value::to_value(&raw_toml)?;
    Ok(from_serde_value(sv))
}

/// Loads RON file and normalizes to Value
fn load_ron(path: &Path) -> Result<Value, VVError> {
    let content = fs::read_to_string(path)?;
    let raw_ron: ron::Value = ron::from_str(&content)?;
    let sv = serde_value::to_value(&raw_ron)?;
    Ok(from_serde_value(sv))
}

// =============================================================================
// Binary Format Loaders
// =============================================================================

/// Loads MessagePack file
fn load_msgpack(path: &Path) -> Result<Value, VVError> {
    let content = fs::read(path)?;
    let sv: serde_value::Value = rmp_serde::from_slice(&content)?;
    Ok(from_serde_value(sv))
}

/// Loads Python Pickle file
fn load_pickle(path: &Path) -> Result<Value, VVError> {
    let content = fs::read(path)?;
    let sv: serde_value::Value = serde_pickle::from_slice(&content, Default::default())?;
    Ok(from_serde_value(sv))
}

/// Loads CBOR file
fn load_cbor(path: &Path) -> Result<Value, VVError> {
    let content = fs::read(path)?;
    let sv: serde_value::Value = ciborium::from_reader(&content[..])?;
    Ok(from_serde_value(sv))
}

/// Loads BSON file
fn load_bson(path: &Path) -> Result<Value, VVError> {
    let content = fs::read(path)?;
    let doc: bson::Document = bson::from_slice(&content)?;
    let sv = serde_value::to_value(&doc)?;
    Ok(from_serde_value(sv))
}

// =============================================================================
// Tabular Format Loaders
// =============================================================================

/// Loads CSV file with dual row/column access patterns.
fn load_csv(path: &Path) -> Result<Value, VVError> {
    let bytes = fs::read(path)?;
    parse_csv(&bytes)
}

/// Sanitizes a CSV column header to a valid identifier.
fn sanitize_column_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{}", s)
    } else {
        s
    }
}

/// Infers the type of a CSV cell value.
fn infer_csv_value_type(s: &str) -> Value {
    // Try integer
    if let Ok(i) = s.parse::<i64>() {
        return Value::Integer(i);
    }
    // Try float
    if let Ok(f) = s.parse::<f64>() {
        return Value::Float(f);
    }
    // Try boolean
    match s.to_lowercase().as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(s.to_string()),
    }
}

// =============================================================================
// XLSX Format Loader
// =============================================================================

/// Loads XLSX file with dual row/column access patterns for each sheet.
fn load_xlsx(path: &Path) -> Result<Value, VVError> {
    let bytes = fs::read(path)?;
    parse_xlsx(&bytes)
}

/// Parses an Excel cell reference (e.g., "B3") to (row, col) 1-indexed
fn parse_cell_ref(cell_ref: &str) -> (usize, usize) {
    let mut col = 0usize;
    let mut row = 0usize;

    for c in cell_ref.chars() {
        if c.is_ascii_alphabetic() {
            col = col * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1);
        } else if c.is_ascii_digit() {
            row = row * 10 + (c as usize - '0' as usize);
        }
    }

    (row, col)
}

// =============================================================================
// XLSX Parsing Helpers
// =============================================================================

/// Loads the shared strings table from xl/sharedStrings.xml
fn load_shared_strings<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<String>, VVError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let file = archive.by_name("xl/sharedStrings.xml")?;
    let mut reader = Reader::from_reader(std::io::BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut strings = Vec::new();
    let mut current_string = String::new();
    let mut in_si = false;
    let mut in_t = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.name().as_ref() {
                b"si" => {
                    in_si = true;
                    current_string.clear();
                }
                b"t" if in_si => in_t = true,
                _ => {}
            },
            Event::Text(e) if in_t => {
                current_string.push_str(&e.unescape()?);
            }
            Event::End(e) => match e.name().as_ref() {
                b"si" => {
                    in_si = false;
                    strings.push(std::mem::take(&mut current_string));
                }
                b"t" => in_t = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(strings)
}

/// Gets sheet names and their corresponding XML paths from the workbook
fn get_sheet_paths<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<(String, String)>, VVError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    // Parse xl/workbook.xml for sheet names and rIds
    let mut sheet_info: Vec<(String, String)> = Vec::new(); // (name, rId)
    {
        let file = archive.by_name("xl/workbook.xml")?;
        let mut reader = Reader::from_reader(std::io::BufReader::new(file));
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Empty(ref e) | Event::Start(ref e) if e.name().as_ref() == b"sheet" => {
                    let mut name = String::new();
                    let mut r_id = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"name" => name = String::from_utf8_lossy(&attr.value).to_string(),
                            b"r:id" => r_id = String::from_utf8_lossy(&attr.value).to_string(),
                            _ => {}
                        }
                    }
                    if !name.is_empty() && !r_id.is_empty() {
                        sheet_info.push((name, r_id));
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
    }

    // Parse xl/_rels/workbook.xml.rels for rId -> path mapping
    let mut rid_to_path: HashMap<String, String> = HashMap::new();
    {
        let file = archive.by_name("xl/_rels/workbook.xml.rels")?;
        let mut reader = Reader::from_reader(std::io::BufReader::new(file));
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Empty(ref e) | Event::Start(ref e)
                    if e.name().as_ref() == b"Relationship" =>
                {
                    let mut id = String::new();
                    let mut target = String::new();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" => id = String::from_utf8_lossy(&attr.value).to_string(),
                            b"Target" => target = String::from_utf8_lossy(&attr.value).to_string(),
                            _ => {}
                        }
                    }
                    if !id.is_empty() && !target.is_empty() {
                        rid_to_path.insert(id, target);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }
    }

    // Map sheet names to their XML paths
    let result: Vec<(String, String)> = sheet_info
        .into_iter()
        .filter_map(|(name, r_id)| {
            rid_to_path.get(&r_id).map(|target| {
                let path = if target.starts_with('/') {
                    target.trim_start_matches('/').to_string()
                } else {
                    format!("xl/{}", target)
                };
                (name, path)
            })
        })
        .collect();

    Ok(result)
}

/// Parses a worksheet XML file into a row/col structure
fn parse_worksheet<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    xml_path: &str,
    shared_strings: &[String],
) -> Result<Value, VVError> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let file = archive.by_name(xml_path)?;
    let mut reader = Reader::from_reader(std::io::BufReader::new(file));
    reader.config_mut().trim_text(true);

    // Collect cells as (row, col) -> value
    let mut cells: HashMap<(usize, usize), String> = HashMap::new();
    let mut max_row = 0usize;
    let mut max_col = 0usize;

    let mut current_cell_ref = String::new();
    let mut current_cell_type = String::new();
    let mut in_cell = false;
    let mut in_value = false;
    let mut in_inline_string = false;
    let mut in_t = false;
    let mut current_value = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => match e.name().as_ref() {
                b"c" => {
                    in_cell = true;
                    current_cell_ref.clear();
                    current_cell_type.clear();
                    current_value.clear();
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"r" => {
                                current_cell_ref = String::from_utf8_lossy(&attr.value).to_string()
                            }
                            b"t" => {
                                current_cell_type = String::from_utf8_lossy(&attr.value).to_string()
                            }
                            _ => {}
                        }
                    }
                }
                b"v" if in_cell => in_value = true,
                b"is" if in_cell => in_inline_string = true,
                b"t" if in_inline_string => in_t = true,
                _ => {}
            },
            Event::Empty(ref e) if e.name().as_ref() == b"c" => {
                // Empty cell element - no value
            }
            Event::Text(ref e) if in_value || in_t => {
                current_value.push_str(&e.unescape()?);
            }
            Event::End(ref e) => match e.name().as_ref() {
                b"c" => {
                    if !current_cell_ref.is_empty() && !current_value.is_empty() {
                        let (row, col) = parse_cell_ref(&current_cell_ref);
                        let value = if current_cell_type == "s" {
                            match current_value.parse::<usize>() {
                                Ok(idx) => match shared_strings.get(idx) {
                                    Some(s) => s.clone(),
                                    None => {
                                        eprintln!(
                                            "Warning: XLSX shared string index {} out of range (max {}), skipping cell {}",
                                            idx,
                                            shared_strings.len().saturating_sub(1),
                                            current_cell_ref
                                        );
                                        current_cell_ref.clear();
                                        current_value.clear();
                                        current_cell_type.clear();
                                        in_cell = false;
                                        continue;
                                    }
                                },
                                Err(_) => {
                                    eprintln!(
                                        "Warning: XLSX malformed shared string index '{}' in cell {}, skipping",
                                        current_value, current_cell_ref
                                    );
                                    current_cell_ref.clear();
                                    current_value.clear();
                                    current_cell_type.clear();
                                    in_cell = false;
                                    continue;
                                }
                            }
                        } else {
                            current_value.clone()
                        };
                        cells.insert((row, col), value);
                        max_row = max_row.max(row);
                        max_col = max_col.max(col);
                    }
                    in_cell = false;
                    in_inline_string = false;
                    in_t = false;
                }
                b"v" => in_value = false,
                b"is" => in_inline_string = false,
                b"t" => in_t = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    // If no cells found, return empty table
    if cells.is_empty() {
        return Ok(Value::Table {
            headers: vec![],
            rows: vec![],
        });
    }

    // Row 1 is headers (index 1 in Excel), rows 2+ are data
    let headers: Vec<String> = (1..=max_col)
        .map(|col| {
            cells
                .get(&(1, col))
                .map(|s| sanitize_column_name(s))
                .unwrap_or_else(|| format!("col_{}", col))
        })
        .collect();

    // Build row data
    let mut rows: Vec<Vec<Value>> = Vec::new();

    for row_idx in 2..=max_row {
        let row: Vec<Value> = (1..=max_col)
            .map(|col| {
                cells
                    .get(&(row_idx, col))
                    .map(|s| infer_csv_value_type(s))
                    .unwrap_or(Value::String(String::new()))
            })
            .collect();
        rows.push(row);
    }

    Ok(Value::Table { headers, rows })
}

// =============================================================================
// Key Extraction
// =============================================================================

/// Extracts all leaf keys with their full path prefix from a Value
pub fn provides(value: &Value, prefix: &str) -> Option<Vec<String>> {
    extract_keys(value, prefix)
}

/// Recursively extracts all leaf keys from a Value
fn extract_keys(value: &Value, prefix: &str) -> Option<Vec<String>> {
    match value {
        Value::Map(map) => {
            let keys: Vec<String> = map
                .iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .flat_map(|(key, val)| {
                    let full_key = format!("{}.{}", prefix, key);
                    match extract_keys(val, &full_key) {
                        Some(nested) => nested,
                        None => vec![full_key], // Leaf node
                    }
                })
                .collect();

            Some(keys)
        }
        Value::Table { headers, .. } => {
            // Table provides row, col (with sub-keys for each header), and headers
            let mut keys = vec![format!("{}.row", prefix), format!("{}.headers", prefix)];
            for header in headers {
                keys.push(format!("{}.col.{}", prefix, header));
            }
            Some(keys)
        }
        Value::Markdown { front_matter, .. } => {
            let mut keys = vec![format!("{}.content", prefix)];
            if let Some(fm) = front_matter {
                let fm_prefix = format!("{}.front_matter", prefix);
                if let Some(fm_keys) = extract_keys(fm, &fm_prefix) {
                    keys.extend(fm_keys);
                } else {
                    keys.push(fm_prefix);
                }
            }
            Some(keys)
        }
        _ => None, // Non-map values are leaves
    }
}

/// Renders a single template by name to a string.
pub fn render_template<C: serde::Serialize>(
    template_name: &str,
    context: &C,
    env: &minijinja::Environment,
) -> Result<String, VVError> {
    let tmpl = env.get_template(template_name)?;
    Ok(tmpl.render(context)?)
}

/// Renders a single template by name, writing output via the sink.
/// Returns the output path on success.
pub fn fill_template<C: serde::Serialize>(
    template_name: &str,
    context: &C,
    output_dir: &Path,
    env: &minijinja::Environment,
    sink: &mut OutputSink,
) -> Result<PathBuf, VVError> {
    let rendered = render_template(template_name, context, env)?;
    let output_path = build_output_path(output_dir, template_name);
    source::write_file(sink, &output_path, rendered.as_bytes())?;
    Ok(output_path)
}

/// Builds output file path from template name
pub fn build_output_path(output_dir: &Path, template_name: &str) -> std::path::PathBuf {
    template_name
        .split("::")
        .fold(output_dir.to_path_buf(), |mut path, segment| {
            path.push(segment);
            path
        })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    // =========================================================================
    // Value Enum Tests
    // =========================================================================

    #[test]
    fn test_value_serialize_scalars() {
        // Float
        let json = serde_json::to_string(&Value::Float(3.14)).unwrap();
        assert_eq!(json, "3.14");

        // Integer
        let json = serde_json::to_string(&Value::Integer(42)).unwrap();
        assert_eq!(json, "42");

        // Bool
        let json = serde_json::to_string(&Value::Bool(true)).unwrap();
        assert_eq!(json, "true");

        // String
        let json = serde_json::to_string(&Value::String("hello".into())).unwrap();
        assert_eq!(json, "\"hello\"");
    }

    #[test]
    fn test_value_serialize_map_filters_underscore_keys() {
        let mut map = HashMap::new();
        map.insert("visible".to_string(), Value::Integer(1));
        map.insert("_hidden".to_string(), Value::Integer(2));
        map.insert("also_visible".to_string(), Value::Integer(3));

        let json = serde_json::to_string(&Value::Map(map)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("visible").is_some());
        assert!(parsed.get("also_visible").is_some());
        assert!(parsed.get("_hidden").is_none());
    }

    #[test]
    fn test_value_serialize_seq() {
        let seq = Value::Seq(vec![
            Value::Integer(1),
            Value::String("two".into()),
            Value::Float(3.0),
        ]);
        let json = serde_json::to_string(&seq).unwrap();
        assert_eq!(json, "[1,\"two\",3.0]");
    }

    #[test]
    fn test_from_serde_value_integers() {
        assert_eq!(
            from_serde_value(serde_value::Value::I64(42)),
            Value::Integer(42)
        );
        assert_eq!(
            from_serde_value(serde_value::Value::U8(255)),
            Value::Integer(255)
        );
        assert_eq!(
            from_serde_value(serde_value::Value::I32(-100)),
            Value::Integer(-100)
        );
        assert_eq!(
            from_serde_value(serde_value::Value::U64(1000)),
            Value::Integer(1000)
        );
    }

    #[test]
    fn test_from_serde_value_floats() {
        assert_eq!(
            from_serde_value(serde_value::Value::F64(3.14)),
            Value::Float(3.14)
        );
        assert_eq!(
            from_serde_value(serde_value::Value::F32(2.5)),
            Value::Float(2.5)
        );
    }

    #[test]
    fn test_from_serde_value_bool() {
        assert_eq!(
            from_serde_value(serde_value::Value::Bool(true)),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_from_serde_value_string() {
        // Non-quantity strings remain as Value::String
        assert_eq!(
            from_serde_value(serde_value::Value::String("hello".into())),
            Value::String("hello".into())
        );
        // Date strings stay as String (no time parsing yet)
        assert_eq!(
            from_serde_value(serde_value::Value::String("12-DEC-2030".into())),
            Value::String("12-DEC-2030".into())
        );
    }

    // =========================================================================
    // Quantity Eager Parsing Tests (Phase 2)
    // =========================================================================

    #[test]
    fn test_eager_parse_quantity_from_string() {
        // "100 N" should become Value::Quantity, not Value::String
        let result = from_serde_value(serde_value::Value::String("100 N".into()));
        match &result {
            Value::Quantity { value, unit } => {
                assert_eq!(*value, 100.0);
                assert_eq!(unit.symbol, "N");
            }
            other => panic!("Expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_eager_parse_quantity_with_decimal() {
        let result = from_serde_value(serde_value::Value::String("5.5 km/s".into()));
        match &result {
            Value::Quantity { value, unit } => {
                assert_eq!(*value, 5.5);
                // Compound derived unit — check via Quantity conversion to SI
                let qty = units::Quantity::new(*value, *unit);
                // 5.5 km/s = 5500 m/s
                assert!((qty.si_value() - 5500.0).abs() < 1e-6);
            }
            other => panic!("Expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_eager_parse_quantity_kilowatts() {
        let result = from_serde_value(serde_value::Value::String("200 kW".into()));
        match &result {
            Value::Quantity { value, unit } => {
                assert_eq!(*value, 200.0);
                assert_eq!(unit.symbol, "kW");
            }
            other => panic!("Expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_eager_parse_non_quantity_string() {
        // Plain text should remain a string
        let result = from_serde_value(serde_value::Value::String("hello world".into()));
        assert_eq!(result, Value::String("hello world".into()));

        // Date strings should remain strings
        let result = from_serde_value(serde_value::Value::String("04-JUL-2033".into()));
        assert_eq!(result, Value::String("04-JUL-2033".into()));
    }

    #[test]
    fn test_structured_quantity_map() {
        // {value: 100, unit: "N"} should become Value::Quantity
        let mut bmap = BTreeMap::new();
        bmap.insert(
            serde_value::Value::String("value".into()),
            serde_value::Value::I64(100),
        );
        bmap.insert(
            serde_value::Value::String("unit".into()),
            serde_value::Value::String("N".into()),
        );

        let result = from_serde_value(serde_value::Value::Map(bmap));
        match &result {
            Value::Quantity { value, unit } => {
                assert_eq!(*value, 100.0);
                assert_eq!(unit.symbol, "N");
            }
            other => panic!("Expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_structured_quantity_map_with_float() {
        // {value: 9.81, unit: "m/s^2"} should become Value::Quantity
        let mut bmap = BTreeMap::new();
        bmap.insert(
            serde_value::Value::String("value".into()),
            serde_value::Value::F64(9.81),
        );
        bmap.insert(
            serde_value::Value::String("unit".into()),
            serde_value::Value::String("m/s^2".into()),
        );

        let result = from_serde_value(serde_value::Value::Map(bmap));
        match &result {
            Value::Quantity { value, unit } => {
                assert_eq!(*value, 9.81);
                // Compound derived unit — verify via SI conversion (m/s^2 is already SI)
                let qty = units::Quantity::new(*value, *unit);
                assert!((qty.si_value() - 9.81).abs() < 1e-6);
            }
            other => panic!("Expected Quantity, got {:?}", other),
        }
    }

    #[test]
    fn test_structured_quantity_map_extra_keys_stays_map() {
        // {value: 100, unit: "N", extra: "data"} should NOT become Quantity
        let mut bmap = BTreeMap::new();
        bmap.insert(
            serde_value::Value::String("value".into()),
            serde_value::Value::I64(100),
        );
        bmap.insert(
            serde_value::Value::String("unit".into()),
            serde_value::Value::String("N".into()),
        );
        bmap.insert(
            serde_value::Value::String("extra".into()),
            serde_value::Value::String("data".into()),
        );

        let result = from_serde_value(serde_value::Value::Map(bmap));
        assert!(matches!(result, Value::Map(_)));
    }

    #[test]
    fn test_structured_quantity_map_invalid_unit_stays_map() {
        // {value: 100, unit: "not_a_unit"} should stay as a Map
        let mut bmap = BTreeMap::new();
        bmap.insert(
            serde_value::Value::String("value".into()),
            serde_value::Value::I64(100),
        );
        bmap.insert(
            serde_value::Value::String("unit".into()),
            serde_value::Value::String("not_a_unit".into()),
        );

        let result = from_serde_value(serde_value::Value::Map(bmap));
        assert!(matches!(result, Value::Map(_)));
    }

    #[test]
    fn test_quantity_serializes_as_string() {
        use crate::units::parse_unit;

        let unit = parse_unit("N").unwrap();
        let qty = Value::Quantity { value: 100.0, unit };
        let json = serde_json::to_string(&qty).unwrap();
        assert_eq!(json, "\"100 N\"");
    }

    #[test]
    fn test_quantity_serialize_kilowatts() {
        use crate::units::parse_unit;

        let unit = parse_unit("kW").unwrap();
        let qty = Value::Quantity { value: 200.0, unit };
        let json = serde_json::to_string(&qty).unwrap();
        assert_eq!(json, "\"200 kW\"");
    }

    #[test]
    fn test_quantity_in_map_serialization() {
        use crate::units::parse_unit;

        let mut map = HashMap::new();
        map.insert(
            "thrust".to_string(),
            Value::Quantity {
                value: 100.0,
                unit: parse_unit("N").unwrap(),
            },
        );
        map.insert("name".to_string(), Value::String("engine".into()));

        let json = serde_json::to_string(&Value::Map(map)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["thrust"], "100 N");
        assert_eq!(parsed["name"], "engine");
    }

    #[test]
    fn test_quantity_equality() {
        use crate::units::parse_unit;

        let a = Value::Quantity {
            value: 100.0,
            unit: parse_unit("N").unwrap(),
        };
        let b = Value::Quantity {
            value: 100.0,
            unit: parse_unit("N").unwrap(),
        };
        assert_eq!(a, b);

        let c = Value::Quantity {
            value: 200.0,
            unit: parse_unit("N").unwrap(),
        };
        assert_ne!(a, c);
    }



    // =========================================================================
    // Time Eager Parsing Tests (Phase 3)
    // =========================================================================

    #[test]
    fn test_eager_parse_utc_iso8601() {
        let result = from_serde_value(serde_value::Value::String("2030-12-12T00:00:00 UTC".into()));
        assert!(
            matches!(result, Value::Utc(_)),
            "Expected Utc, got {:?}",
            result
        );
    }

    #[test]
    fn test_eager_parse_tdb_iso8601() {
        let result = from_serde_value(serde_value::Value::String("2030-12-12T00:00:00 TDB".into()));
        assert!(
            matches!(result, Value::Tdb(_)),
            "Expected Tdb, got {:?}",
            result
        );
    }

    #[test]
    fn test_eager_parse_utc_spice_style() {
        let result = from_serde_value(serde_value::Value::String("12-DEC-2030 UTC".into()));
        assert!(
            matches!(result, Value::Utc(_)),
            "Expected Utc, got {:?}",
            result
        );
    }

    #[test]
    fn test_eager_parse_tdb_spice_with_time() {
        let result = from_serde_value(serde_value::Value::String(
            "2030-JUN-15 12:00:00.000 TDB".into(),
        ));
        assert!(
            matches!(result, Value::Tdb(_)),
            "Expected Tdb, got {:?}",
            result
        );
    }

    #[test]
    fn test_eager_parse_tt_becomes_tdb() {
        let result = from_serde_value(serde_value::Value::String("2030-12-12T00:00:00 TT".into()));
        assert!(
            matches!(result, Value::Tdb(_)),
            "TT should become Tdb, got {:?}",
            result
        );
    }

    #[test]
    fn test_eager_parse_tai_becomes_utc() {
        let result = from_serde_value(serde_value::Value::String("2030-12-12T00:00:00 TAI".into()));
        assert!(
            matches!(result, Value::Utc(_)),
            "TAI should become Utc, got {:?}",
            result
        );
    }

    #[test]
    fn test_no_time_suffix_stays_string() {
        // "12-DEC-2030" without suffix stays as String (could also be parsed as Quantity first)
        let result = from_serde_value(serde_value::Value::String("12-DEC-2030".into()));
        assert!(
            matches!(result, Value::String(_)),
            "No suffix should stay as String, got {:?}",
            result
        );
    }

    #[test]
    fn test_utc_serializes_as_string() {
        let days = crate::time::epoch::utc_calendar_to_j2000_days(2030, 12, 12, 0, 0, 0.0).unwrap();
        let val = Value::Utc(days);
        let json = serde_json::to_string(&val).unwrap();
        assert!(
            json.contains("2030-12-12"),
            "Expected date in JSON, got: {}",
            json
        );
        assert!(json.contains("UTC"), "Expected UTC suffix, got: {}", json);
    }

    #[test]
    fn test_tdb_serializes_as_string() {
        let days = crate::time::epoch::tdb_calendar_to_j2000_days(2030, 12, 12, 0, 0, 0.0);
        let val = Value::Tdb(days);
        let json = serde_json::to_string(&val).unwrap();
        assert!(
            json.contains("2030-12-12"),
            "Expected date in JSON, got: {}",
            json
        );
        assert!(json.contains("TDB"), "Expected TDB suffix, got: {}", json);
    }

    #[test]
    fn test_utc_tdb_equality() {
        let a = Value::Utc(100.0);
        let b = Value::Utc(100.0);
        let c = Value::Utc(200.0);
        let d = Value::Tdb(100.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d); // Utc != Tdb even with same f64
    }

    #[test]
    fn test_from_serde_value_map() {
        let mut bmap = BTreeMap::new();
        bmap.insert(
            serde_value::Value::String("key".into()),
            serde_value::Value::I64(42),
        );

        let result = from_serde_value(serde_value::Value::Map(bmap));
        match result {
            Value::Map(map) => {
                assert_eq!(map.get("key"), Some(&Value::Integer(42)));
            }
            _ => panic!("Expected Map"),
        }
    }

    #[test]
    fn test_from_serde_value_seq() {
        let seq = vec![serde_value::Value::I64(1), serde_value::Value::I64(2)];
        let result = from_serde_value(serde_value::Value::Seq(seq));
        assert_eq!(
            result,
            Value::Seq(vec![Value::Integer(1), Value::Integer(2)])
        );
    }

    // =========================================================================
    // File Type Detection Tests
    // =========================================================================

    #[test]
    fn test_is_supported_data_file_type() {
        // Text formats
        assert!(is_supported_data_file_type(Path::new("test.json")));
        assert!(is_supported_data_file_type(Path::new("test.yaml")));
        assert!(is_supported_data_file_type(Path::new("test.yml")));
        assert!(is_supported_data_file_type(Path::new("test.toml")));
        assert!(is_supported_data_file_type(Path::new("test.ron")));

        // Binary formats
        assert!(is_supported_data_file_type(Path::new("test.msgpack")));
        assert!(is_supported_data_file_type(Path::new("test.mp")));
        assert!(is_supported_data_file_type(Path::new("test.pickle")));
        assert!(is_supported_data_file_type(Path::new("test.pkl")));
        assert!(is_supported_data_file_type(Path::new("test.cbor")));
        assert!(is_supported_data_file_type(Path::new("test.bson")));

        // Tabular formats
        assert!(is_supported_data_file_type(Path::new("test.csv")));
        assert!(is_supported_data_file_type(Path::new("test.xlsx")));

        // Markdown
        assert!(is_supported_data_file_type(Path::new("test.md")));

        // Unsupported
        assert!(!is_supported_data_file_type(Path::new("test.txt")));
        assert!(!is_supported_data_file_type(Path::new("test.j2")));
        assert!(!is_supported_data_file_type(Path::new("noextension")));
    }

    #[test]
    fn test_unsupported_file_type() {
        let result = load_file(Path::new("test.txt"));
        assert!(result.is_err());
    }

    #[test]
    fn test_build_output_path() {
        let output_dir = Path::new("/output");

        assert_eq!(
            build_output_path(output_dir, "template.md"),
            PathBuf::from("/output/template.md")
        );

        assert_eq!(
            build_output_path(output_dir, "reports::summary.md"),
            PathBuf::from("/output/reports/summary.md")
        );
    }

    #[test]
    fn test_extract_front_matter() {
        let content = r#"---
name: "Test Component"
mass_kg: 10.5
---
# Documentation

This is markdown documentation that should be ignored.
"#;
        let result = extract_front_matter(content);
        assert!(result.is_some());
        let yaml_content = result.unwrap();
        assert!(yaml_content.contains("name:"));
        assert!(yaml_content.contains("mass_kg:"));
        assert!(!yaml_content.contains("Documentation"));
    }

    #[test]
    fn test_extract_front_matter_no_leading_delimiter() {
        let content = r#"name: "Test Component"
mass_kg: 10.5
"#;
        let result = extract_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_front_matter_single_delimiter() {
        let content = r#"---
name: "Test Component"
mass_kg: 10.5
"#;
        let result = extract_front_matter(content);
        assert!(result.is_none());
    }

    #[test]
    fn test_load_yaml_with_front_matter() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let content = r#"---
name: "PSR Radiators"
subsystem: "thermal"
mass_kg: 4.0
nested:
  value: 42
---

# PSR Radiators

## Description
Body-mounted thermal radiators for rejecting spacecraft waste heat.
"#;

        let mut temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        temp_file.write_all(content.as_bytes()).unwrap();

        let val = load_yaml(temp_file.path()).expect("Failed to load YAML with front matter");

        let prov = provides(&val, "test").unwrap();
        assert!(prov.iter().any(|k| k.contains("name")));
        assert!(prov.iter().any(|k| k.contains("mass_kg")));
    }

    #[test]
    fn test_sanitize_column_name() {
        assert_eq!(sanitize_column_name("name"), "name");
        assert_eq!(sanitize_column_name("First Name"), "first_name");
        assert_eq!(sanitize_column_name("value-1"), "value_1");
        assert_eq!(sanitize_column_name("UPPERCASE"), "uppercase");
        assert_eq!(sanitize_column_name("1st"), "_1st");
        assert_eq!(sanitize_column_name("123abc"), "_123abc");
    }

    #[test]
    fn test_infer_csv_value_type() {
        assert_eq!(infer_csv_value_type("42"), Value::Integer(42));
        assert_eq!(infer_csv_value_type("3.14"), Value::Float(3.14));
        assert_eq!(infer_csv_value_type("true"), Value::Bool(true));
        assert_eq!(infer_csv_value_type("FALSE"), Value::Bool(false));
        assert_eq!(infer_csv_value_type("hello"), Value::String("hello".into()));
    }

    // =========================================================================
    // Table Type Tests (Phase 4)
    // =========================================================================

    #[test]
    fn test_table_serializes_with_row_col_headers() {
        let table = Value::Table {
            headers: vec!["name".into(), "value".into()],
            rows: vec![
                vec![Value::String("a".into()), Value::Integer(1)],
                vec![Value::String("b".into()), Value::Integer(2)],
            ],
        };
        let json = serde_json::to_string(&table).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // Backward-compatible row access
        assert!(parsed["row"].is_array());
        assert_eq!(parsed["row"][0][0], "a");
        assert_eq!(parsed["row"][0][1], 1);
        assert_eq!(parsed["row"][1][0], "b");

        // Backward-compatible col access
        assert!(parsed["col"].is_object());
        assert_eq!(parsed["col"]["name"][0], "a");
        assert_eq!(parsed["col"]["name"][1], "b");
        assert_eq!(parsed["col"]["value"][0], 1);

        // New headers access
        assert_eq!(parsed["headers"][0], "name");
        assert_eq!(parsed["headers"][1], "value");
    }

    #[test]
    fn test_table_equality() {
        let a = Value::Table {
            headers: vec!["x".into()],
            rows: vec![vec![Value::Integer(1)]],
        };
        let b = Value::Table {
            headers: vec!["x".into()],
            rows: vec![vec![Value::Integer(1)]],
        };
        let c = Value::Table {
            headers: vec!["y".into()],
            rows: vec![vec![Value::Integer(1)]],
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_table_provides_keys() {
        let table = Value::Table {
            headers: vec!["name".into(), "value".into()],
            rows: vec![],
        };
        let prov = provides(&table, "data").unwrap();
        assert!(prov.contains(&"data.row".to_string()));
        assert!(prov.contains(&"data.headers".to_string()));
        assert!(prov.contains(&"data.col.name".to_string()));
        assert!(prov.contains(&"data.col.value".to_string()));
    }

    // =========================================================================
    // XLSX Format Tests
    // =========================================================================

    #[test]
    fn test_xlsx_extension_supported() {
        assert!(is_supported_data_file_type(Path::new("test.xlsx")));
    }

    #[test]
    fn test_parse_cell_ref() {
        assert_eq!(parse_cell_ref("A1"), (1, 1));
        assert_eq!(parse_cell_ref("B3"), (3, 2));
        assert_eq!(parse_cell_ref("Z1"), (1, 26));
        assert_eq!(parse_cell_ref("AA1"), (1, 27));
        assert_eq!(parse_cell_ref("AB10"), (10, 28));
    }

    // =========================================================================
    // Underscore Prefix Filtering Tests
    // =========================================================================

    #[test]
    fn test_provides_skips_underscore_keys() {
        let mut map = HashMap::new();
        map.insert("thrust".to_string(), Value::String("100 N".into()));
        map.insert("_source".to_string(), Value::String("file.yaml".into()));
        map.insert("isp".to_string(), Value::String("323 s".into()));

        let prov = provides(&Value::Map(map), "propulsion").unwrap();
        assert!(prov.contains(&"propulsion.thrust".to_string()));
        assert!(prov.contains(&"propulsion.isp".to_string()));
        assert!(!prov.iter().any(|k| k.contains("_source")));
    }

    // =========================================================================
    // Markdown Type Tests (Phase 5)
    // =========================================================================

    #[test]
    fn test_parse_markdown_plain() {
        let content = b"# Hello World\n\nSome markdown content.";
        let val = parse_markdown(content).unwrap();
        match &val {
            Value::Markdown {
                content,
                front_matter,
            } => {
                assert_eq!(content, "# Hello World\n\nSome markdown content.");
                assert!(front_matter.is_none());
            }
            other => panic!("Expected Markdown, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_markdown_with_front_matter() {
        let content = b"---\ntitle: \"Test Doc\"\nauthor: \"alice\"\n---\n# Hello\n\nBody text.";
        let val = parse_markdown(content).unwrap();
        match &val {
            Value::Markdown {
                content,
                front_matter,
            } => {
                assert_eq!(content, "# Hello\n\nBody text.");
                let fm = front_matter.as_ref().expect("Expected front matter");
                if let Value::Map(map) = fm.as_ref() {
                    assert_eq!(map.get("title"), Some(&Value::String("Test Doc".into())));
                    assert_eq!(map.get("author"), Some(&Value::String("alice".into())));
                } else {
                    panic!("Expected front matter to be a Map, got {:?}", fm);
                }
            }
            other => panic!("Expected Markdown, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_markdown_front_matter_quantity_parsing() {
        // Front matter values should be eagerly parsed
        let content = b"---\nthrust: \"100 N\"\nname: \"engine\"\n---\n# Engine Docs";
        let val = parse_markdown(content).unwrap();
        match &val {
            Value::Markdown { front_matter, .. } => {
                let fm = front_matter.as_ref().expect("Expected front matter");
                if let Value::Map(map) = fm.as_ref() {
                    assert!(matches!(map.get("thrust"), Some(Value::Quantity { .. })));
                    assert_eq!(map.get("name"), Some(&Value::String("engine".into())));
                } else {
                    panic!("Expected Map");
                }
            }
            other => panic!("Expected Markdown, got {:?}", other),
        }
    }

    #[test]
    fn test_markdown_serializes_with_content_key() {
        let val = Value::Markdown {
            content: "# Title\n\nBody.".into(),
            front_matter: None,
        };
        let json = serde_json::to_string(&val).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["content"], "# Title\n\nBody.");
        assert!(parsed.get("front_matter").is_none());
    }

    #[test]
    fn test_markdown_serializes_with_front_matter() {
        let mut fm = HashMap::new();
        fm.insert("title".to_string(), Value::String("Test".into()));
        let val = Value::Markdown {
            content: "# Body".into(),
            front_matter: Some(Box::new(Value::Map(fm))),
        };
        let json = serde_json::to_string(&val).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["content"], "# Body");
        assert_eq!(parsed["front_matter"]["title"], "Test");
    }

    #[test]
    fn test_markdown_equality() {
        let a = Value::Markdown {
            content: "hello".into(),
            front_matter: None,
        };
        let b = Value::Markdown {
            content: "hello".into(),
            front_matter: None,
        };
        let c = Value::Markdown {
            content: "world".into(),
            front_matter: None,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_markdown_provides_keys() {
        let val = Value::Markdown {
            content: "# Hello".into(),
            front_matter: None,
        };
        let prov = provides(&val, "readme").unwrap();
        assert!(prov.contains(&"readme.content".to_string()));
    }

    #[test]
    fn test_markdown_provides_keys_with_front_matter() {
        let mut fm = HashMap::new();
        fm.insert("title".to_string(), Value::String("Test".into()));
        fm.insert("version".to_string(), Value::Integer(2));
        let val = Value::Markdown {
            content: "# Hello".into(),
            front_matter: Some(Box::new(Value::Map(fm))),
        };
        let prov = provides(&val, "readme").unwrap();
        assert!(prov.contains(&"readme.content".to_string()));
        assert!(prov.contains(&"readme.front_matter.title".to_string()));
        assert!(prov.contains(&"readme.front_matter.version".to_string()));
    }

    #[test]
    fn test_extract_front_matter_and_body() {
        let content = "---\ntitle: \"Test\"\n---\n# Hello\n\nWorld.";
        let result = extract_front_matter_and_body(content);
        assert!(result.is_some());
        let (yaml, body) = result.unwrap();
        assert!(yaml.contains("title:"));
        assert_eq!(body, "# Hello\n\nWorld.");
    }

    #[test]
    fn test_extract_front_matter_and_body_no_front_matter() {
        let content = "# Just Markdown\n\nNo front matter here.";
        assert!(extract_front_matter_and_body(content).is_none());
    }

    #[test]
    fn test_load_markdown_file() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let content =
            "---\nsubsystem: thermal\nmass_kg: 4.0\n---\n# Radiators\n\nBody-mounted radiators.";
        let mut temp = NamedTempFile::with_suffix(".md").unwrap();
        temp.write_all(content.as_bytes()).unwrap();

        let val = load_file(temp.path()).expect("Failed to load markdown file");
        match &val {
            Value::Markdown {
                content,
                front_matter,
            } => {
                assert!(content.contains("Radiators"));
                let fm = front_matter.as_ref().expect("Expected front matter");
                if let Value::Map(map) = fm.as_ref() {
                    assert_eq!(map.get("subsystem"), Some(&Value::String("thermal".into())));
                } else {
                    panic!("Expected Map");
                }
            }
            other => panic!("Expected Markdown, got {:?}", other),
        }
    }

    #[test]
    fn test_md_extension_supported() {
        assert!(is_supported_data_file_type(Path::new("readme.md")));
        assert!(is_supported_data_file_type(Path::new("docs/notes.md")));
    }

    // =========================================================================
    // Source Provenance Tests (Phase 6)
    // =========================================================================

    #[test]
    fn test_source_filtered_from_serialization() {
        let mut map = HashMap::new();
        map.insert("visible".to_string(), Value::Integer(1));
        map.insert(
            "_source".to_string(),
            Value::Source {
                path: PathBuf::from("test.yaml"),
            },
        );

        let json = serde_json::to_string(&Value::Map(map)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("visible").is_some());
        assert!(parsed.get("_source").is_none());
    }

    #[test]
    fn test_source_filtered_from_provides() {
        let mut map = HashMap::new();
        map.insert("thrust".to_string(), Value::String("100 N".into()));
        map.insert(
            "_source".to_string(),
            Value::Source {
                path: PathBuf::from("propulsion.yaml"),
            },
        );

        let prov = provides(&Value::Map(map), "propulsion").unwrap();
        assert!(prov.contains(&"propulsion.thrust".to_string()));
        assert!(!prov.iter().any(|k| k.contains("_source")));
    }

    #[test]
    fn test_source_equality() {
        let a = Value::Source {
            path: PathBuf::from("a.yaml"),
        };
        let b = Value::Source {
            path: PathBuf::from("a.yaml"),
        };
        let c = Value::Source {
            path: PathBuf::from("b.yaml"),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // =========================================================================
    // Annotation Tests (Phase 7)
    // =========================================================================

    #[test]
    fn test_annotation_sidecar_path_for_data_file() {
        let path = PathBuf::from("/project/propulsion.yaml");
        let sidecar = annotation_sidecar_path(&path);
        assert_eq!(
            sidecar,
            PathBuf::from("/project/propulsion.annotations.ron")
        );
    }

    #[test]
    fn test_annotation_sidecar_path_for_markdown() {
        let path = PathBuf::from("/project/readme.md");
        let sidecar = annotation_sidecar_path(&path);
        assert_eq!(sidecar, PathBuf::from("/project/readme.md.annotations.ron"));
    }

    #[test]
    fn test_is_annotation_sidecar() {
        assert!(is_annotation_sidecar(Path::new(
            "propulsion.annotations.ron"
        )));
        assert!(is_annotation_sidecar(Path::new(
            "/project/readme.md.annotations.ron"
        )));
        assert!(!is_annotation_sidecar(Path::new("propulsion.yaml")));
        assert!(!is_annotation_sidecar(Path::new("propulsion.ron")));
        assert!(!is_annotation_sidecar(Path::new("annotations.ron")));
    }

    #[test]
    fn test_annotation_data_deserialize_from_ron() {
        let ron_str = r#"{
            "thrust": [
                Annotation(
                    ann_type: Comment,
                    author: "alice",
                    text: "Verified against datasheet rev B",
                    status: Open,
                    tags: ["verified"],
                    replies: [],
                ),
            ],
            "isp": [
                Annotation(
                    ann_type: Question,
                    author: "bob",
                    text: "Is this at sea level or vacuum?",
                    status: InProgress,
                    tags: [],
                    replies: [
                        Reply(
                            author: "alice",
                            text: "Vacuum conditions",
                            created: "2030-06-15T10:30:00Z",
                        ),
                    ],
                ),
            ],
        }"#;
        let annotations: HashMap<String, Vec<AnnotationData>> =
            ron::from_str(ron_str).expect("Failed to parse RON annotations");

        assert_eq!(annotations.len(), 2);
        assert_eq!(annotations["thrust"].len(), 1);
        assert_eq!(annotations["thrust"][0].ann_type, AnnotationType::Comment);
        assert_eq!(annotations["thrust"][0].author, "alice");
        assert_eq!(annotations["isp"].len(), 1);
        assert_eq!(annotations["isp"][0].ann_type, AnnotationType::Question);
        assert_eq!(annotations["isp"][0].replies.len(), 1);
        assert_eq!(annotations["isp"][0].replies[0].author, "alice");
    }

    #[test]
    fn test_markdown_annotation_data_deserialize_from_ron() {
        let ron_str = r#"[
            MarkdownAnnotation(
                ann_type: Question,
                author: "bob",
                text: "This section needs a citation",
                status: Open,
                line: 42,
                line_end: Some(45),
                tags: ["review"],
                replies: [],
            ),
            MarkdownAnnotation(
                ann_type: Issue,
                author: "alice",
                text: "Formatting inconsistent",
                status: Resolved,
                line: 10,
                tags: [],
                replies: [],
            ),
        ]"#;
        let annotations: Vec<MarkdownAnnotationData> =
            ron::from_str(ron_str).expect("Failed to parse RON markdown annotations");

        assert_eq!(annotations.len(), 2);
        assert_eq!(annotations[0].ann_type, AnnotationType::Question);
        assert_eq!(annotations[0].line, 42);
        assert_eq!(annotations[0].line_end, Some(45));
        assert_eq!(annotations[1].ann_type, AnnotationType::Issue);
        assert_eq!(annotations[1].status, Status::Resolved);
        assert_eq!(annotations[1].line_end, None);
    }

    #[test]
    fn test_annotation_type_suggestion() {
        let ron_str = r#"[
            Annotation(
                ann_type: Suggestion(suggested: "Use 120 N instead"),
                author: "charlie",
                text: "Based on updated test data",
                status: Open,
                tags: [],
                replies: [],
            ),
        ]"#;
        let annotations: Vec<AnnotationData> =
            ron::from_str(ron_str).expect("Failed to parse suggestion annotation");
        assert_eq!(annotations.len(), 1);
        if let AnnotationType::Suggestion { suggested } = &annotations[0].ann_type {
            assert_eq!(suggested, "Use 120 N instead");
        } else {
            panic!("Expected Suggestion variant");
        }
    }

    #[test]
    fn test_merge_data_annotations() {
        let ron_bytes = br#"{
            "thrust": [
                Annotation(
                    ann_type: Comment,
                    author: "alice",
                    text: "Verified",
                    status: Open,
                    tags: [],
                    replies: [],
                ),
            ],
        }"#;

        let mut value = Value::Map(HashMap::from([(
            "thrust".to_string(),
            Value::String("100 N".to_string()),
        )]));

        merge_data_annotations(&mut value, ron_bytes);

        if let Value::Map(map) = &value {
            assert!(map.contains_key("_annotations"));
            if let Some(Value::Map(ann_map)) = map.get("_annotations") {
                assert!(ann_map.contains_key("thrust"));
                if let Some(Value::Seq(anns)) = ann_map.get("thrust") {
                    assert_eq!(anns.len(), 1);
                    assert!(matches!(&anns[0], Value::Annotation(_)));
                } else {
                    panic!("Expected Seq for thrust annotations");
                }
            } else {
                panic!("Expected Map for _annotations");
            }
        } else {
            panic!("Expected Map");
        }
    }

    #[test]
    fn test_merge_data_annotations_no_op_for_non_map() {
        let ron_bytes = br#"{ "key": [ Annotation(ann_type: Comment, author: "a", text: "t", status: Open, tags: [], replies: []) ] }"#;
        let mut value = Value::String("not a map".to_string());
        merge_data_annotations(&mut value, ron_bytes);
        // Value should be unchanged
        assert_eq!(value, Value::String("not a map".to_string()));
    }

    #[test]
    fn test_parse_markdown_annotations() {
        let ron_bytes = br#"[
            MarkdownAnnotation(
                ann_type: Question,
                author: "bob",
                text: "Needs citation",
                status: Open,
                line: 42,
                tags: [],
                replies: [],
            ),
        ]"#;

        let result = parse_markdown_annotations(ron_bytes);
        assert!(result.is_some());
        if let Some(Value::Seq(anns)) = result {
            assert_eq!(anns.len(), 1);
            assert!(matches!(&anns[0], Value::MarkdownAnnotation(_)));
        } else {
            panic!("Expected Seq");
        }
    }

    #[test]
    fn test_parse_markdown_annotations_empty() {
        let ron_bytes = b"[]";
        let result = parse_markdown_annotations(ron_bytes);
        assert!(result.is_none());
    }

    #[test]
    fn test_annotation_not_serialized() {
        let ann = Value::Annotation(AnnotationData {
            ann_type: AnnotationType::Comment,
            author: "alice".to_string(),
            text: "test".to_string(),
            status: Status::Open,
            tags: vec![],
            replies: vec![],
        });
        // Serialize as unit (null)
        let json = serde_json::to_string(&ann).expect("Failed to serialize");
        assert_eq!(json, "null");
    }

    #[test]
    fn test_markdown_annotation_not_serialized() {
        let ann = Value::MarkdownAnnotation(MarkdownAnnotationData {
            ann_type: AnnotationType::Issue,
            author: "bob".to_string(),
            text: "test".to_string(),
            status: Status::Open,
            line: 1,
            line_end: None,
            char_start: None,
            char_end: None,
            tags: vec![],
            replies: vec![],
        });
        let json = serde_json::to_string(&ann).expect("Failed to serialize");
        assert_eq!(json, "null");
    }

    #[test]
    fn test_annotations_filtered_from_provides() {
        let value = Value::Map(HashMap::from([
            ("thrust".to_string(), Value::String("100 N".to_string())),
            (
                "_annotations".to_string(),
                Value::Map(HashMap::from([(
                    "thrust".to_string(),
                    Value::Seq(vec![Value::Annotation(AnnotationData {
                        ann_type: AnnotationType::Comment,
                        author: "alice".to_string(),
                        text: "test".to_string(),
                        status: Status::Open,
                        tags: vec![],
                        replies: vec![],
                    })]),
                )])),
            ),
        ]));
        let keys = provides(&value, "propulsion").expect("Should provide keys");
        // _annotations key should be filtered (starts with _)
        assert!(keys.contains(&"propulsion.thrust".to_string()));
        assert!(!keys.iter().any(|k| k.contains("_annotations")));
    }

    #[test]
    fn test_annotation_equality() {
        let a1 = Value::Annotation(AnnotationData {
            ann_type: AnnotationType::Comment,
            author: "alice".to_string(),
            text: "test".to_string(),
            status: Status::Open,
            tags: vec!["tag1".to_string()],
            replies: vec![],
        });
        let a2 = Value::Annotation(AnnotationData {
            ann_type: AnnotationType::Comment,
            author: "alice".to_string(),
            text: "test".to_string(),
            status: Status::Open,
            tags: vec!["tag1".to_string()],
            replies: vec![],
        });
        let a3 = Value::Annotation(AnnotationData {
            ann_type: AnnotationType::Question,
            author: "bob".to_string(),
            text: "different".to_string(),
            status: Status::Resolved,
            tags: vec![],
            replies: vec![],
        });
        assert_eq!(a1, a2);
        assert_ne!(a1, a3);
    }

    #[test]
    fn test_status_variants() {
        let statuses = r#"[Open, InProgress, Resolved, Accepted, Rejected]"#;
        let parsed: Vec<Status> = ron::from_str(statuses).expect("Failed to parse statuses");
        assert_eq!(parsed.len(), 5);
        assert_eq!(parsed[0], Status::Open);
        assert_eq!(parsed[1], Status::InProgress);
        assert_eq!(parsed[2], Status::Resolved);
        assert_eq!(parsed[3], Status::Accepted);
        assert_eq!(parsed[4], Status::Rejected);
    }
}
