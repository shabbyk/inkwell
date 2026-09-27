//! Core data types.
//!
//! # Two representations
//!
//! Notally stores its list/map fields as JSON *strings* in SQLite columns (Room
//! `@TypeConverters`), but serialises them as real nested JSON when exporting. Inkwell
//! preserves both shapes, because Android backup import depends on the row shape and
//! the portable zip export depends on the nested shape:
//!
//! - [`Note`] — nested. Used by the HTTP API, the JSON export, and the editor.
//! - [`NoteRow`] — flattened. Used for reading and writing SQLite rows.
//!
//! [`Note`] and [`NoteRow`] convert losslessly in both directions via
//! [`Note::to_row`] and [`NoteRow::to_note`].
//!
//! # Field naming
//!
//! Field names here are **not** free choices. They must match Notally exactly, or
//! Android backup import breaks. Two details are easy to get wrong:
//!
//! - [`Image::mime_type`] serialises as `mimeType` (camelCase), not `mime_type`.
//! - In the portable export, a note's timestamp is `date-created` (kebab-case), while
//!   the SQLite column is `timestamp`.
//!
//! See `NoteExport` for the export shape and `docs/plan.md` §5 for format details.

use serde::{Deserialize, Serialize};

/// Whether a note is free text or a checklist.
///
/// Serialised uppercase to match Notally's `Type.valueOf(...)` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NoteType {
    Note,
    List,
}

/// Which collection a note belongs to. Notally uses a soft delete.
///
/// Serialised uppercase to match Notally's `Folder.valueOf(...)` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Folder {
    /// Active notes.
    Notes,
    /// Soft-deleted. Not shown in the normal list; removable for good.
    Deleted,
    /// Archived out of the way but kept.
    Archived,
}

/// The twelve note colours, with the exact hex values Notally uses.
///
/// The hex values are part of the interchange format: a note written by the Android
/// app must render in the same colour here. Do not "tidy" them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Color {
    #[default]
    Default,
    Coral,
    Orange,
    Sand,
    Storm,
    Fog,
    Sage,
    Mint,
    Dusk,
    Flower,
    Blossom,
    Clay,
}

impl Color {
    /// The twelve colours in Notally's display order, for the colour picker UI.
    pub const ALL: [Color; 12] = [
        Color::Default,
        Color::Coral,
        Color::Orange,
        Color::Sand,
        Color::Storm,
        Color::Fog,
        Color::Sage,
        Color::Mint,
        Color::Dusk,
        Color::Flower,
        Color::Blossom,
        Color::Clay,
    ];

    /// Hex value, copied verbatim from Notally's `res/values/colors.xml`.
    pub fn hex(self) -> &'static str {
        match self {
            Color::Default => "#FFFFFF",
            Color::Coral => "#FAAFA9",
            Color::Orange => "#FFCC80",
            Color::Sand => "#FFF8B9",
            Color::Storm => "#AFCCDC",
            Color::Fog => "#D3E4EC",
            Color::Sage => "#B4DED4",
            Color::Mint => "#E2F6D3",
            Color::Dusk => "#D3BFDB",
            Color::Flower => "#F8BBD0",
            Color::Blossom => "#F5E2DC",
            Color::Clay => "#E9E3D3",
        }
    }
}

/// How often a reminder repeats.
///
/// Notally's reminders are inert in Inkwell: this is stored and round-tripped so that
/// imported backups keep their data, but nothing schedules or fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Frequency {
    Once,
    Daily,
    Monthly,
}

/// An attached image. `name` is a filename under `media/images/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub name: String,
    /// Serialises as `mimeType` — camelCase is required for Notally compatibility.
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// An attached audio recording. `name` is a filename under `media/audios/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Audio {
    pub name: String,
    /// Duration in milliseconds.
    pub duration: i64,
    /// Epoch milliseconds.
    pub timestamp: i64,
}

/// One line of a checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub body: String,
    pub checked: bool,
}

/// A reminder. Stored and round-tripped, but never scheduled — see [`Frequency`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    /// Epoch milliseconds.
    pub timestamp: i64,
    pub frequency: Frequency,
}

/// A run of formatted text within a note body.
///
/// `start` is inclusive, `end` is exclusive, both in Unicode scalar values. Several
/// attributes may be true at once — Notally merges spans that share the same range
/// (see `NotallyModel.getFilteredRepresentations` in the Android source), and
/// [`crate::spans`] reproduces that merge.
///
/// Note these offsets index the *stored* `body`, which may be shorter than the text the
/// user typed: Notally trims trailing whitespace on save
/// (`NotallyModel.getBaseNote`). Spans past the end of `body` must be dropped on load
/// rather than panicking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanRepresentation {
    pub bold: bool,
    pub link: bool,
    pub italic: bool,
    pub monospace: bool,
    pub strikethrough: bool,
    pub start: usize,
    pub end: usize,
}

impl SpanRepresentation {
    /// True if at least one attribute is set. Mirrors Notally's `isNotUseless()`.
    /// Spans where every attribute is false are dropped on save.
    pub fn is_not_useless(&self) -> bool {
        self.bold || self.link || self.italic || self.monospace || self.strikethrough
    }

    /// True if this span covers exactly the same range as `other`. Mirrors Notally's
    /// `isEqualInSize()`.
    pub fn is_equal_in_size(&self, other: &SpanRepresentation) -> bool {
        self.start == other.start && self.end == other.end
    }
}

/// A global label. Inkwell is single-user, so labels are just a distinct sorted list of
/// strings; this struct exists to match Notally's `Label` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Label {
    pub value: String,
}

/// A note, in nested form. This is the shape the API, the editor, and the portable
/// export all speak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    #[serde(rename = "type")]
    pub note_type: NoteType,
    pub folder: Folder,
    pub color: Color,
    pub title: String,
    pub pinned: bool,
    /// Epoch milliseconds. Named `date-created` in the portable export — see
    /// [`NoteExport`].
    pub timestamp: i64,
    pub labels: Vec<String>,
    pub body: String,
    pub spans: Vec<SpanRepresentation>,
    pub items: Vec<ListItem>,
    pub images: Vec<Image>,
    pub audios: Vec<Audio>,
    pub reminder: Option<Reminder>,
}

impl Note {
    /// Flatten to the SQLite row shape, JSON-encoding the list fields.
    pub fn to_row(&self) -> NoteRow {
        NoteRow {
            id: self.id,
            note_type: self.note_type,
            folder: self.folder,
            color: self.color,
            title: self.title.clone(),
            pinned: self.pinned,
            timestamp: self.timestamp,
            labels: serde_json::to_string(&self.labels).unwrap_or_else(|_| "[]".into()),
            body: self.body.clone(),
            spans: serde_json::to_string(&self.spans).unwrap_or_else(|_| "[]".into()),
            items: serde_json::to_string(&self.items).unwrap_or_else(|_| "[]".into()),
            images: serde_json::to_string(&self.images).unwrap_or_else(|_| "[]".into()),
            audios: serde_json::to_string(&self.audios).unwrap_or_else(|_| "[]".into()),
            reminder: self
                .reminder
                .as_ref()
                .and_then(|r| serde_json::to_string(r).ok()),
        }
    }
}

/// A note, in SQLite row shape — the list fields as JSON strings, matching Notally's
/// column layout exactly so that an Android backup can be copied in row by row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteRow {
    pub id: i64,
    #[serde(rename = "type")]
    pub note_type: NoteType,
    pub folder: Folder,
    pub color: Color,
    pub title: String,
    pub pinned: bool,
    pub timestamp: i64,
    pub labels: String,
    pub body: String,
    pub spans: String,
    pub items: String,
    pub images: String,
    pub audios: String,
    pub reminder: Option<String>,
}

impl NoteRow {
    /// Expand to nested form, parsing the JSON columns.
    ///
    /// Each column is parsed independently: a malformed one falls back to an empty
    /// list rather than failing the whole row. Backups from older Notally versions
    /// (before migrations 2–5 added `color`, `images`, `audios`, `reminder`) are read
    /// by the import layer, which supplies defaults before this is called.
    pub fn to_note(&self) -> Note {
        Note {
            id: self.id,
            note_type: self.note_type,
            folder: self.folder,
            color: self.color,
            title: self.title.clone(),
            pinned: self.pinned,
            timestamp: self.timestamp,
            labels: parse_or_empty(&self.labels),
            body: self.body.clone(),
            spans: parse_or_empty(&self.spans),
            items: parse_or_empty(&self.items),
            images: parse_or_empty(&self.images),
            audios: parse_or_empty(&self.audios),
            reminder: self
                .reminder
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok()),
        }
    }
}

/// Parse a JSON column, falling back to an empty vector.
///
/// A single corrupt column should not make a whole note unreadable, and during import a
/// missing or empty column is normal.
fn parse_or_empty<T: serde::de::DeserializeOwned>(s: &str) -> Vec<T> {
    serde_json::from_str(s).unwrap_or_default()
}

/// The portable-export shape of a single note, matching Notally's `getJSON()`.
///
/// Two deliberate differences from [`Note`]:
///
/// - The timestamp field is `date-created`, not `timestamp`.
/// - Only one of `body` + `spans`, or `items`, is present — whichever suits the note
///   type. Notally writes `body`/`spans` for notes and `items` for lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteExport {
    #[serde(rename = "type")]
    pub note_type: NoteType,
    pub color: Color,
    pub title: String,
    pub pinned: bool,
    #[serde(rename = "date-created")]
    pub timestamp: i64,
    pub labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<SpanRepresentation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<ListItem>>,
}

impl From<&Note> for NoteExport {
    fn from(n: &Note) -> Self {
        // Notally omits the body fields for lists and the items field for notes,
        // rather than writing them as null.
        let (body, spans, items) = match n.note_type {
            NoteType::Note => {
                (Some(n.body.clone()), Some(n.spans.clone()), None)
            }
            NoteType::List => (None, None, Some(n.items.clone())),
        };
        NoteExport {
            note_type: n.note_type,
            color: n.color,
            title: n.title.clone(),
            pinned: n.pinned,
            timestamp: n.timestamp,
            labels: n.labels.clone(),
            body,
            spans,
            items,
        }
    }
}

impl Note {
    /// Recover a full note from its export form, applying Notally's defaults for fields
    /// the export omits.
    pub fn from_export(e: &NoteExport, id: i64, folder: Folder) -> Note {
        Note {
            id,
            note_type: e.note_type,
            folder,
            color: e.color,
            title: e.title.clone(),
            pinned: e.pinned,
            timestamp: e.timestamp,
            labels: e.labels.clone(),
            body: e.body.clone().unwrap_or_default(),
            spans: e.spans.clone().unwrap_or_default(),
            items: e.items.clone().unwrap_or_default(),
            images: Vec::new(),
            audios: Vec::new(),
            reminder: None,
        }
    }
}

/// The `manifest.json` entry at the root of a portable export zip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Export format version. Inkwell writes `1`.
    pub version: u32,
    /// RFC 3339 timestamp of when the export was made.
    pub exported_at: String,
    pub note_count: usize,
    pub app_version: String,
}

/// The complete contents of a portable export, serialised as `notes.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesFile {
    pub notes: Vec<NoteExport>,
    pub labels: Vec<String>,
    pub settings: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_round_trips_through_row_form() {
        let note = Note {
            id: 7,
            note_type: NoteType::List,
            folder: Folder::Archived,
            color: Color::Sage,
            title: "Groceries".into(),
            pinned: true,
            timestamp: 1_758_000_000_000,
            labels: vec!["home".into()],
            body: String::new(),
            spans: vec![],
            items: vec![
                ListItem { body: "milk".into(), checked: true },
                ListItem { body: "eggs".into(), checked: false },
            ],
            images: vec![Image { name: "a.png".into(), mime_type: "image/png".into() }],
            audios: vec![],
            reminder: Some(Reminder { timestamp: 42, frequency: Frequency::Daily }),
        };
        assert_eq!(note.to_row().to_note(), note);
    }

    /// The camelCase is load-bearing: Notally writes `mimeType`, and Android import
    /// reads that exact key.
    #[test]
    fn image_serialises_mime_type_as_camel_case() {
        let image = Image { name: "a.png".into(), mime_type: "image/png".into() };
        assert_eq!(
            serde_json::to_string(&image).unwrap(),
            r#"{"name":"a.png","mimeType":"image/png"}"#
        );
    }

    /// Likewise kebab-case: Notally's export key is `date-created`, not `timestamp`.
    #[test]
    fn export_serialises_timestamp_as_date_created() {
        let note = Note {
            id: 1,
            note_type: NoteType::Note,
            folder: Folder::Notes,
            color: Color::Default,
            title: "t".into(),
            pinned: false,
            timestamp: 1_758_000_000_000,
            labels: vec![],
            body: "b".into(),
            spans: vec![],
            items: vec![],
            images: vec![],
            audios: vec![],
            reminder: None,
        };
        let json = serde_json::to_value(NoteExport::from(&note)).unwrap();
        assert_eq!(json["date-created"], 1_758_000_000_000i64);
        assert!(json.get("timestamp").is_none());
    }

    #[test]
    fn enum_names_are_uppercase_to_match_notally() {
        assert_eq!(serde_json::to_string(&NoteType::Note).unwrap(), r#""NOTE""#);
        assert_eq!(serde_json::to_string(&Folder::Deleted).unwrap(), r#""DELETED""#);
        assert_eq!(serde_json::to_string(&Frequency::Monthly).unwrap(), r#""MONTHLY""#);
        assert_eq!(serde_json::to_string(&Color::Blossom).unwrap(), r#""BLOSSOM""#);
    }

    #[test]
    fn export_omits_the_inapplicable_field() {
        let mut note = Note {
            id: 1,
            note_type: NoteType::Note,
            folder: Folder::Notes,
            color: Color::Default,
            title: String::new(),
            pinned: false,
            timestamp: 0,
            labels: vec![],
            body: "hello".into(),
            spans: vec![],
            items: vec![ListItem { body: "x".into(), checked: false }],
            images: vec![],
            audios: vec![],
            reminder: None,
        };
        let json = serde_json::to_value(NoteExport::from(&note)).unwrap();
        assert!(json.get("body").is_some());
        assert!(json.get("items").is_none(), "notes must not carry items");

        note.note_type = NoteType::List;
        let json = serde_json::to_value(NoteExport::from(&note)).unwrap();
        assert!(json.get("body").is_none(), "lists must not carry a body");
        assert!(json.get("items").is_some());
    }

    #[test]
    fn corrupt_json_column_degrades_to_empty() {
        let mut row = NoteRow {
            id: 1,
            note_type: NoteType::Note,
            folder: Folder::Notes,
            color: Color::Default,
            title: String::new(),
            pinned: false,
            timestamp: 0,
            labels: "[]".into(),
            body: "body".into(),
            spans: "not json".into(),
            items: "[]".into(),
            images: "[]".into(),
            audios: "[]".into(),
            reminder: None,
        };
        row.spans = "{ broken".into();
        let note = row.to_note();
        assert!(note.spans.is_empty());
        assert_eq!(note.body, "body", "other fields must survive");
    }
}
