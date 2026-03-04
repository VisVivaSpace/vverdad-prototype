# Underscore-Prefix Convention

VVERDAD uses a naming convention where keys starting with `_` (underscore) in `Value::Map` are treated as internal metadata and excluded from user-facing output.

## Purpose

The `_`-prefix convention separates metadata from user data within the same data structure. This allows provenance tracking, annotations, and other internal bookkeeping to coexist with user-defined data keys without interfering with template rendering or dependency validation.

## Behavior

### Serialization Filtering

When a `Value::Map` is serialized (for Minijinja template rendering), all keys starting with `_` are **filtered out**:

```rust
// From the Serialize implementation for Value::Map:
let visible: Vec<(&String, &Value)> = map
    .iter()
    .filter(|(k, _)| !k.starts_with('_'))
    .collect();
```

This means templates never see `_`-prefixed keys. For example, given a data file `propulsion.yaml`:

```yaml
thrust: "500 N"
isp: "320 s"
```

The loaded `Value::Map` contains:
```
propulsion:
  thrust: Quantity(500.0, N)
  isp: Quantity(320.0, s)
  _source: Source("propulsion.yaml")     <-- invisible to templates
  _annotations: Map(...)                  <-- invisible to templates
```

In a template, `{{ propulsion.thrust }}` works but there is no way to access `{{ propulsion._source }}`.

### Provides() Filtering

The `provides()` function, which extracts the list of available data keys for dependency validation, also filters out `_`-prefixed keys:

```rust
fn extract_keys(value: &Value, prefix: &str) -> Option<Vec<String>> {
    match value {
        Value::Map(map) => {
            let keys: Vec<String> = map
                .iter()
                .filter(|(k, _)| !k.starts_with('_'))
                // ...
        }
    }
}
```

This ensures that internal metadata keys are not reported as "provided" data and cannot be listed as dependencies.

## Reserved Keys

The following `_`-prefixed keys are used by VVERDAD internally:

| Key | Value Type | Description |
|-----|-----------|-------------|
| `_source` | `Value::Source { path }` | File path provenance. Automatically inserted into Map values when loaded from a data file. |
| `_annotations` | `Value::Map(HashMap<String, Value>)` | Data annotations from a `.annotations.ron` sidecar. Each key maps to a `Value::Seq` of `Value::Annotation` values. |
| `_markdown_annotations` | `Value::Seq(Vec<Value>)` | Markdown annotations from a `.md.annotations.ron` sidecar. Each element is a `Value::MarkdownAnnotation`. |

## Non-Serializing Value Types

Three Value variants always serialize to `null` (unit), regardless of where they appear:

| Variant | Purpose |
|---------|---------|
| `Value::Source { path }` | File provenance tracking |
| `Value::Annotation(AnnotationData)` | Data annotation |
| `Value::MarkdownAnnotation(MarkdownAnnotationData)` | Markdown annotation |

These types are designed to be stored under `_`-prefixed keys, where they would be filtered out by Map serialization anyway. The `null` serialization provides a safety net in case they appear outside of `_`-prefixed keys.

## Scope

The `_`-prefix convention applies **only to Map keys**, not to:

- Value content (a string containing `_source` is just a string)
- File or directory names (a directory named `_output` has separate handling logic)
- Sequence elements (there is no filtering within `Value::Seq`)
- Table headers or cell values

## User-Defined _-Prefixed Keys

Users can create their own `_`-prefixed keys in data files. These will be stored in the `Value::Map` but will be invisible to templates and `provides()`, just like the reserved keys. This can be useful for storing metadata that should travel with the data but not appear in rendered output.

```yaml
# propulsion.yaml
thrust: "500 N"
isp: "320 s"
_notes: "Values from RD-180 spec sheet, rev 3.2"  # invisible to templates
_last_verified: "2024-11-15"                        # invisible to templates
```
