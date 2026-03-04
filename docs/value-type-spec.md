# Value Type Specification

This document specifies the custom `Value` enum used by VVERDAD to represent all loaded data. It replaces the previous `pub type Value = serde_value::Value` type alias with a rich, domain-aware type system.

## Value Enum

```rust
pub enum Value {
    // Scalars
    Float(f64),
    Integer(i64),
    Bool(bool),
    String(String),

    // Physical quantity (float-only; integers auto-promote to f64)
    Quantity { value: f64, unit: Unit },

    // Time (f64 days after J2000.0 epoch = 2000-01-01 12:00:00 TT)
    Utc(f64),
    Tdb(f64),

    // Table (first-class, heterogeneous cells)
    Table { headers: Vec<String>, rows: Vec<Vec<Value>> },

    // Markdown (raw content + optional YAML front matter)
    Markdown { content: String, front_matter: Option<Box<Value>> },

    // Non-serializing metadata (stored under `_`-prefixed keys)
    Source { path: PathBuf },
    Annotation(AnnotationData),
    MarkdownAnnotation(MarkdownAnnotationData),

    // Compound
    Map(HashMap<String, Value>),
    Seq(Vec<Value>),
}
```

## Variant Descriptions

### Scalars

| Variant | Description |
|---------|-------------|
| `Float(f64)` | 64-bit floating-point number. All serde float types (F32, F64) normalize to this. |
| `Integer(i64)` | 64-bit signed integer. All serde integer types (U8, U16, U32, U64, I8, I16, I32, I64) normalize to this. |
| `Bool(bool)` | Boolean true/false. |
| `String(String)` | UTF-8 string. Only values that fail all eager parsing rules remain as String. |

### Physical Quantities

| Variant | Description |
|---------|-------------|
| `Quantity { value: f64, unit: Unit }` | A numeric value with a physical unit. The value is always f64 (integers are promoted). Created by eager parsing of strings like `"100 N"` or structured maps like `{value: 100, unit: "N"}`. |

### Time Epochs

| Variant | Description |
|---------|-------------|
| `Utc(f64)` | UTC epoch stored as days after J2000.0. Created from strings with UTC or TAI suffixes. TAI inputs are converted to UTC at parse time. |
| `Tdb(f64)` | TDB (Barycentric Dynamical Time) epoch stored as days after J2000.0. Created from strings with TDB or TT suffixes. TT inputs are converted to TDB at parse time. |

### Structured Data

| Variant | Description |
|---------|-------------|
| `Table { headers, rows }` | First-class tabular data from CSV and XLSX files. Headers are sanitized column names. Rows contain heterogeneous cell values (Integer, Float, Bool, or String). Serializes to `{row: [...], col: {...}, headers: [...]}`. |
| `Markdown { content, front_matter }` | Markdown file content with optional parsed YAML front matter. Front matter is extracted from `---` delimiters and parsed as a Value. Serializes to `{content: "...", front_matter: {...}}`. |

### Non-Serializing Metadata

These variants are stored under `_`-prefixed keys in Maps and serialize to `null` (unit). They are filtered out during serialization to Minijinja templates.

| Variant | Description |
|---------|-------------|
| `Source { path: PathBuf }` | File provenance tracking. Automatically attached as `_source` to all Map values when loaded from a file. |
| `Annotation(AnnotationData)` | Data value annotation from a `.annotations.ron` sidecar file. Stored in the `_annotations` map. |
| `MarkdownAnnotation(MarkdownAnnotationData)` | Markdown line annotation from a `.md.annotations.ron` sidecar file. Stored in the `_markdown_annotations` sequence. |

### Compound Types

| Variant | Description |
|---------|-------------|
| `Map(HashMap<String, Value>)` | Key-value map. Keys starting with `_` are filtered during serialization and `provides()`. Non-string map keys from serde are converted to strings. |
| `Seq(Vec<Value>)` | Ordered sequence of values. |

## Eager Parsing Rules

When a string or map value is loaded from any data format, VVERDAD applies eager parsing to detect domain-specific types. The parsing order is important because some patterns overlap.

### String Parsing Order

The function `try_parse_string(s)` applies the following tests in order:

1. **Time epoch suffix**: If the string ends with a space-separated time system suffix (` UTC`, ` TDB`, ` TT`, ` TAI`, case-insensitive), attempt to parse the preceding date as an epoch.
   - Success: returns `Value::Utc(days)` or `Value::Tdb(days)`
   - If suffix is present but date is unparseable: returns `Some(Err(...))` (the error propagates)
   - If no suffix is found: continues to step 2

2. **Quantity string**: Attempt to parse as `"<number> <unit>"` (e.g., `"100 N"`, `"5.5 km/s"`, `"-3.14 rad"`).
   - Success: returns `Value::Quantity { value, unit }`
   - Failure: continues to step 3

3. **Fall through**: The string remains as `Value::String(s)`.

### Map Parsing (Structured Quantity)

After converting a serde map to `HashMap<String, Value>`, the function `try_parse_structured_quantity()` checks for:

- The map has **exactly 2 keys**: `"value"` and `"unit"`
- `"value"` is `Value::Float(f64)` or `Value::Integer(i64)` (integers promote to f64)
- `"unit"` is `Value::String(s)` that parses as a valid unit

If all conditions are met, the map becomes `Value::Quantity { value, unit }`. Otherwise it remains `Value::Map(...)`.

### Parsing Precedence Summary

```
String input
  |
  +--[ends with time suffix?]---> parse date ---> Value::Utc or Value::Tdb
  |
  +--[parses as quantity?]------> Value::Quantity
  |
  +--[otherwise]---------------> Value::String

Map input
  |
  +--[exactly {value, unit}?]---> Value::Quantity
  |
  +--[otherwise]---------------> Value::Map
```

## Conversion from serde_value::Value

The `from_serde_value()` function converts any `serde_value::Value` into the custom `Value` enum. All data formats (JSON, TOML, YAML, RON, MessagePack, Pickle, CBOR, BSON) go through serde and then this conversion.

### Type Normalization

| serde_value type | Value type |
|-----------------|------------|
| U8, U16, U32, U64, I8, I16, I32, I64 | `Integer(i64)` |
| F32, F64 | `Float(f64)` |
| Bool | `Bool(bool)` |
| String | `try_parse_string()` (may become Quantity, Utc, Tdb, or String) |
| Char | `String(c.to_string())` |
| Unit, Option(None) | `String("")` (empty string) |
| Option(Some(v)) | Recurse on inner value |
| Bytes | `String(from_utf8_lossy)` |
| Newtype(v) | Recurse (unwrap) |
| Seq | `Seq(vec)` with recursive conversion |
| Map | Check for structured quantity, then `Map(hashmap)` with recursive conversion |

### Map Key Conversion

Non-string map keys from serde are converted to strings:
- `I64` and `U64` use `to_string()`
- `Bool`, `F64`, `Char` use `to_string()`
- Other types use `Debug` formatting

## CSV and XLSX: Direct Table Construction

CSV and XLSX files bypass `serde_value` entirely. They are parsed directly into `Value::Table { headers, rows }`:

- **CSV**: Each cell is individually type-inferred (`infer_csv_value_type`): integer, float, boolean, or string
- **XLSX**: Each cell is parsed from the XML structure with type attributes. Shared strings and inline strings are both supported
- **Column name sanitization**: Spaces/hyphens become underscores, uppercase becomes lowercase, leading digits get a `_` prefix

XLSX files produce a `Value::Map` where each key is a sanitized sheet name and each value is a `Value::Table`.

## Markdown Loading

Markdown files (`.md`) are loaded as `Value::Markdown`:

1. The file content is checked for YAML front matter (delimited by `---` at the start)
2. If front matter is found:
   - The YAML between `---` delimiters is parsed through `serde_value` and `from_serde_value()` (so eager parsing applies to front matter values)
   - The remaining body becomes `content`
   - Result: `Markdown { content: body, front_matter: Some(Box::new(parsed_yaml)) }`
3. If no front matter:
   - The entire file becomes `content`
   - Result: `Markdown { content: full_text, front_matter: None }`

Front matter detection requires:
- The file starts with `---` (after optional whitespace)
- A second `---` appears at the start of a subsequent line
- The content between delimiters contains at least one `:` (to distinguish from non-YAML)

## Serialization to Minijinja

Each variant serializes differently for template rendering:

| Variant | Serialization |
|---------|--------------|
| `Float(f)` | `f64` number |
| `Integer(i)` | `i64` number |
| `Bool(b)` | boolean |
| `String(s)` | string |
| `Quantity { value, unit }` | String `"<value> <unit>"` (e.g., `"100 N"`) |
| `Utc(days)` | String `"YYYY-MM-DDTHH:MM:SS.fff UTC"` |
| `Tdb(days)` | String `"YYYY-MM-DDTHH:MM:SS.fff TDB"` |
| `Table { headers, rows }` | Map with `row` (array of arrays), `col` (map of column arrays), `headers` (array of names) |
| `Markdown { content, front_matter }` | Map with `content` string and optional `front_matter` |
| `Source { .. }` | `null` (unit) |
| `Annotation(..)` | `null` (unit) |
| `MarkdownAnnotation(..)` | `null` (unit) |
| `Map(map)` | Map with `_`-prefixed keys **filtered out** |
| `Seq(seq)` | Array |

### _-Prefix Filtering in Map Serialization

When a `Value::Map` is serialized, all keys starting with `_` are excluded from the output. This means metadata keys like `_source`, `_annotations`, and `_markdown_annotations` are invisible to templates. This filtering happens in the custom `Serialize` implementation for `Value`.

## Source Provenance

When a data file is loaded, `_source` metadata is automatically attached:

```rust
fn attach_source(value: &mut Value, path: &Path) {
    if let Value::Map(map) = value {
        map.insert("_source".to_string(), Value::Source { path: path.to_path_buf() });
    }
}
```

This only applies to `Value::Map` values. Scalar values, sequences, tables, and markdown values do not receive `_source` metadata.
