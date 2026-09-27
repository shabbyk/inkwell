# Task specs for a smaller local model

Inkwell's server is small and the build is mostly mechanical. These tasks are sized for a
local model (Qwen 9B or similar) to complete reliably.

**Ground rules for whoever picks up a task**

1. Read the spec file for your task completely before writing code.
2. Only create or modify the files that spec names. If you need a file that does not exist
   yet, stop and report that instead of inventing it.
3. The types in `server/src/models.rs` are already written. Import from
   `crate::models::…`. Do **not** redefine them.
4. Follow the exact signatures given. Other tasks are being written against them.
5. Run `cargo build` and `cargo test` in `server/` when done. Both must pass with no new
   warnings.
6. When done, commit with the message given in the spec.

**Where the types come from**

`server/src/models.rs` is the contract. It already contains:

```rust
pub enum NoteType { Note, List }          // serialises as "NOTE" / "LIST"
pub enum Folder   { Notes, Deleted, Archived }
pub enum Color    { Default, Coral, Orange, Sand, Storm, Fog,
                    Sage, Mint, Dusk, Flower, Blossom, Clay }
pub enum Frequency { Once, Daily, Monthly }

pub struct Image { pub name: String, pub mime_type: String }  // JSON key: mimeType
pub struct Audio { pub name: String, pub duration: i64, pub timestamp: i64 }
pub struct ListItem { pub body: String, pub checked: bool }
pub struct Reminder { pub timestamp: i64, pub frequency: Frequency }
pub struct SpanRepresentation { pub bold: bool, pub link: bool, pub italic: bool,
                                pub monospace: bool, pub strikethrough: bool,
                                pub start: usize, pub end: usize }
pub struct Label { pub value: String }
pub struct Note { pub id: i64, pub note_type: NoteType, pub folder: Folder,
                   pub color: Color, pub title: String, pub pinned: bool,
                   pub timestamp: i64, pub labels: Vec<String>, pub body: String,
                   pub spans: Vec<SpanRepresentation>, pub items: Vec<ListItem>,
                   pub images: Vec<Image>, pub audios: Vec<Audio>,
                   pub reminder: Option<Reminder> }
pub struct NoteRow { /* same fields, list fields are String */ }
pub struct NoteExport { pub note_type: NoteType, pub color: Color, pub title: String,
                         pub pinned: bool, pub timestamp: i64, pub labels: Vec<String>,
                         pub body: Option<String>,
                         pub spans: Option<Vec<SpanRepresentation>>,
                         pub items: Option<Vec<ListItem>> }
pub struct Manifest { pub version: u32, pub exported_at: String,
                      pub note_count: usize, pub app_version: String }
pub struct NotesFile { pub notes: Vec<NoteExport>, pub labels: Vec<String>,
                       pub settings: BTreeMap<String, String> }

impl Note { pub fn to_row(&self) -> NoteRow }
impl NoteRow { pub fn to_note(&self) -> Note }
impl Color { pub const ALL: [Color; 12]; pub fn hex(self) -> &'static str }
```

All of these are `#[derive(Debug, Clone, Serialize, Deserialize)]` as appropriate, and are
`pub` from a module declared as `mod models;` in `src/main.rs`.

**Note the three details a small model reliably gets wrong.** If a task mentions them,
they matter:

- `Image.mime_type` serialises as **`mimeType`** (camelCase), set with
  `#[serde(rename = "mimeType")]`. Android import reads that exact key.
- In the portable export a note's timestamp is **`date-created`** (kebab-case), not
  `timestamp`.
- All enums serialise **UPPERCASE** (`"NOTE"`, `"DELETED"`, `"MONTHLY"`).

---

## Division of work

Tasks marked **[local]** are safe to hand to a smaller model. They are mechanical, fully
specified, and easy to verify by running the tests.

Tasks marked **[core]** are not delegated. They need judgement or carry a failure mode
that tests will not catch. Do not hand these to a small model, and do not let one touch
them even if it appears to work.

| Task | Who | Why |
|---|---|---|
| T1 label CRUD | [local] | Repetitive, one clear pattern |
| T2 settings store | [local] | Mechanical key-value reads |
| T3 zip export | [local] | Crate usage, well-defined output |
| T4 single-note TXT/HTML | [local] | String building, testable against fixtures |
| T5 colour picker UI | [local] | Presentational component |
| T6 note list component | [local] | Presentational component |
| T7 settings form UI | [local] | Form wiring |
| T8 note CRUD routes | [core] | Needs a query-builder contract first |
| T9 `spans.rs` round-trip | **[core]** | Highest risk in the project |
| T10 `auth.rs` | **[core]** | Security-critical |
| T11 import format detection | **[core]** | Subtle, silently wrong on edge cases |
| T12 Android SQLite import | **[core]** | `PRAGMA` migration handling |
| T13 `search.rs` FTS5 | **[core]** | The original's known false-positive bug |
| T14 media upload | [core] | Path traversal risk |

**Sequencing.** T1–T4 are independent of each other and can run in parallel. T5–T7 need
the API client, which lands with T8. T9–T14 are core and stay with the primary agent.
