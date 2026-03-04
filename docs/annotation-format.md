# Annotation Sidecar Format

VVERDAD supports annotations on data values and markdown content through sidecar files in RON (Rusty Object Notation) format. Annotations provide a way to attach review comments, questions, issues, and suggestions to specific data points or document locations without modifying the original data files.

## Overview

There are two types of annotation sidecars:

| Type | Sidecar filename | RON type | Stored as |
|------|-----------------|----------|-----------|
| Data annotations | `{stem}.annotations.ron` | `HashMap<String, Vec<AnnotationData>>` | `_annotations` Map in the data value |
| Markdown annotations | `{filename}.md.annotations.ron` | `Vec<MarkdownAnnotationData>` | `_markdown_annotations` Seq |

## Data Annotations

Data annotations attach to specific keys within a data file. The sidecar file is named by replacing the data file's extension with `.annotations.ron`.

### Naming Convention

| Data file | Sidecar file |
|-----------|-------------|
| `propulsion.yaml` | `propulsion.annotations.ron` |
| `mission.json` | `mission.annotations.ron` |
| `power.toml` | `power.annotations.ron` |
| `config.ron` | `config.annotations.ron` |

### RON Format

The sidecar contains a RON `HashMap<String, Vec<AnnotationData>>` where:
- Keys map to data keys in the file (use dotted notation for nested values)
- Values are lists of annotations on that key

```ron
{
    "thrust": [
        Annotation(
            ann_type: Comment,
            author: "Jane Smith",
            text: "Verified against RD-180 spec sheet, rev 3.2",
            status: Resolved,
            tags: ["verification", "propulsion"],
            replies: [],
        ),
    ],
    "isp": [
        Annotation(
            ann_type: Question,
            author: "Bob Chen",
            text: "Is this sea-level or vacuum Isp?",
            status: Open,
            tags: ["clarification"],
            replies: [
                Reply(
                    author: "Jane Smith",
                    text: "Vacuum Isp. Will add a note to the data file.",
                    created: "2024-11-15",
                ),
            ],
        ),
    ],
    "tanks.pressure": [
        Annotation(
            ann_type: Issue,
            author: "Alice Wong",
            text: "Operating pressure exceeds safety factor of 2.0 for Ti-6Al-4V at this wall thickness",
            status: InProgress,
            tags: ["safety", "structures"],
            replies: [],
        ),
    ],
}
```

### Storage in the Data Tree

When annotations are loaded, they are merged into the parent `Value::Map` under the `_annotations` key:

```
propulsion (Map)
  +-- thrust: Quantity(500.0, "N")
  +-- isp: Quantity(320.0, "s")
  +-- _source: Source("propulsion.yaml")
  +-- _annotations: Map
        +-- "thrust": Seq [Annotation(...)]
        +-- "isp": Seq [Annotation(...)]
        +-- "tanks.pressure": Seq [Annotation(...)]
```

Because `_annotations` starts with `_`, it is filtered from template serialization and `provides()`.

## Markdown Annotations

Markdown annotations attach to specific lines or character ranges in a markdown file. The sidecar appends `.annotations.ron` to the full markdown filename.

### Naming Convention

| Markdown file | Sidecar file |
|--------------|-------------|
| `readme.md` | `readme.md.annotations.ron` |
| `notes.md` | `notes.md.annotations.ron` |

### RON Format

The sidecar contains a RON `Vec<MarkdownAnnotationData>`:

```ron
[
    MarkdownAnnotation(
        ann_type: Comment,
        author: "Jane Smith",
        text: "This section needs updated references",
        status: Open,
        line: 15,
        line_end: None,
        char_start: None,
        char_end: None,
        tags: ["documentation"],
        replies: [],
    ),
    MarkdownAnnotation(
        ann_type: Suggestion(
            suggested: "The delta-v budget is 5.2 km/s for the transfer orbit.",
        ),
        author: "Bob Chen",
        text: "Current text has an error in the delta-v value",
        status: Accepted,
        line: 42,
        line_end: Some(42),
        char_start: Some(4),
        char_end: Some(48),
        tags: ["correction"],
        replies: [],
    ),
]
```

### Position Fields

| Field | Type | Description |
|-------|------|-------------|
| `line` | `usize` | The line number (1-indexed) where the annotation starts |
| `line_end` | `Option<usize>` | Optional end line for multi-line annotations |
| `char_start` | `Option<usize>` | Optional character offset within the start line |
| `char_end` | `Option<usize>` | Optional character offset within the end line |

### Storage in the Data Tree

Markdown annotations are stored as a `_markdown_annotations` Seq of `MarkdownAnnotation` values.

## Common Types

### AnnotationType

```rust
pub enum AnnotationType {
    Comment,                           // General observation
    Question,                          // Request for clarification
    Issue,                             // Problem that needs resolution
    Suggestion { suggested: String },  // Proposed change with replacement text
}
```

### Status

```rust
pub enum Status {
    Open,        // Newly created, not yet addressed
    InProgress,  // Being worked on
    Resolved,    // Fixed or answered
    Accepted,    // Suggestion accepted
    Rejected,    // Suggestion rejected or issue dismissed
}
```

### Reply

```rust
pub struct Reply {
    pub author: String,   // Who wrote the reply
    pub text: String,     // Reply content
    pub created: String,  // When the reply was created (free-form string)
}
```

### AnnotationData (Data Annotations)

```rust
pub struct AnnotationData {
    pub ann_type: AnnotationType,
    pub author: String,
    pub text: String,
    pub status: Status,
    pub tags: Vec<String>,       // default: empty
    pub replies: Vec<Reply>,     // default: empty
}
```

### MarkdownAnnotationData (Markdown Annotations)

```rust
pub struct MarkdownAnnotationData {
    pub ann_type: AnnotationType,
    pub author: String,
    pub text: String,
    pub status: Status,
    pub line: usize,
    pub line_end: Option<usize>,     // default: None
    pub char_start: Option<usize>,   // default: None
    pub char_end: Option<usize>,     // default: None
    pub tags: Vec<String>,           // default: empty
    pub replies: Vec<Reply>,         // default: empty
}
```

## Serialization Behavior

Both `Value::Annotation` and `Value::MarkdownAnnotation` serialize to `null` (unit) in Minijinja. They are intended as metadata, not template data. Additionally, the `_annotations` and `_markdown_annotations` keys are `_`-prefixed, so they are filtered out of Map serialization entirely.

## Detection and Filtering

The function `is_annotation_sidecar(path)` returns `true` for any file whose name ends with `.annotations.ron`. These files are not loaded as regular data files -- they are loaded alongside their parent data file and merged in automatically.
