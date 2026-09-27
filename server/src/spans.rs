//! Conversion between Notally's flat span format and ProseMirror/TipTap JSON.
//!
//! # The two formats
//!
//! Notally stores rich text as a plain string plus a list of character ranges:
//!
//! ```text
//! body  = "call bob tomorrow"
//! spans = [ { start: 5, end: 8, bold: true, ... } ]   // "bob" is bold
//! ```
//!
//! ProseMirror stores a nested document tree, where formatting is a *mark* attached to a
//! text node. Inkwell's editor speaks ProseMirror, the database speaks flat spans, so
//! every load and save crosses this boundary:
//!
//! ```text
//! to_flat  : ProseMirror JSON -> (body, spans)   -- on save
//! to_pm    : (body, spans)    -> ProseMirror JSON -- on load
//! ```
//!
//! # Why round-trip fidelity is the thing to get right
//!
//! This module decides whether a backup written by the Android app displays correctly
//! here. A bug here does not throw — it renders text with the wrong formatting, or drops
//! formatting silently, and nothing in the UI will report it. Hence the heavy testing and
//! the defensive handling of malformed input.
//!
//! # Offsets are character indices, not byte indices
//!
//! Notally's spans index *characters*. Rust's `&str` slices index *bytes*. Slicing a body
//! at a span boundary will panic on any non-ASCII text, and silently mis-render emoji or
//! accented characters. Every offset crossing this module is therefore a character index,
//! and all slicing goes through the `char` vector. **Do not use `&body[a..b]`.**
//!
//! # Merging
//!
//! Notally collapses several spans that cover the same range into one span with several
//! attributes set, rather than storing overlapping spans. `normalize` reproduces that,
//! and is applied on both sides so a save/load cycle is stable.
//!
//! # Scope
//!
//! Only inline formatting is handled: bold, italic, monospace, strikethrough, link. The
//! editor must be configured to produce exactly these marks and plain text nodes — no
//! headings, no block types, no lists inside a note body. Checklists live in the
//! separate `items` field and never pass through here. If the editor is allowed to
//! produce other node types, the round-trip guarantee does not hold.

use anyhow::{anyhow, Result};
use regex::Regex;
use serde_json::{json, Value};
use std::sync::LazyLock;

use crate::models::SpanRepresentation;

/// The five inline styles Notally supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MarkSet {
    pub bold: bool,
    pub italic: bool,
    pub monospace: bool,
    pub strikethrough: bool,
    pub link: bool,
}

impl MarkSet {
    /// True if no style is set. Such a run needs no marks in the output.
    pub fn is_empty(&self) -> bool {
        !(self.bold || self.italic || self.monospace || self.strikethrough || self.link)
    }

    fn from_span(s: &SpanRepresentation) -> Self {
        MarkSet {
            bold: s.bold,
            italic: s.italic,
            monospace: s.monospace,
            strikethrough: s.strikethrough,
            link: s.link,
        }
    }

    /// ProseMirror mark objects for this set, in a stable order.
    fn pm_marks(&self, text: &str) -> Vec<Value> {
        let mut marks = Vec::new();
        if self.bold {
            marks.push(json!({ "type": "bold" }));
        }
        if self.italic {
            marks.push(json!({ "type": "italic" }));
        }
        if self.strikethrough {
            marks.push(json!({ "type": "strike" }));
        }
        if self.monospace {
            marks.push(json!({ "type": "code" }));
        }
        if self.link {
            // Notally stores no URL — a link is just text that looks like one, and the
            // target is derived from the text. See `url_from`.
            marks.push(json!({ "type": "link", "attrs": { "href": url_from(text) } }));
        }
        marks
    }
}

/// Collapse spans that cover the same range into one span with combined attributes.
///
/// Mirrors Notally's `NotallyModel.getFilteredRepresentations`. Spans with no active
/// style are dropped, matching `isNotUseless()`.
pub fn normalize(spans: &[SpanRepresentation]) -> Vec<SpanRepresentation> {
    let mut out: Vec<SpanRepresentation> = Vec::new();

    for s in spans {
        if !s.is_not_useless() || s.start >= s.end {
            continue;
        }
        match out.iter_mut().find(|e| e.is_equal_in_size(s)) {
            Some(existing) => {
                existing.bold |= s.bold;
                existing.link |= s.link;
                existing.italic |= s.italic;
                existing.monospace |= s.monospace;
                existing.strikethrough |= s.strikethrough;
            }
            None => out.push(*s),
        }
    }

    out.sort_by_key(|s| (s.start, s.end));
    out
}

/// Build a ProseMirror document from a body and its spans.
///
/// Spans reaching past the end of `body` are clamped, and empty ones are dropped, so
/// malformed data from a hand-edited or corrupt backup cannot panic.
pub fn to_pm(body: &str, spans: &[SpanRepresentation]) -> Value {
    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();

    // Clamp into range and discard anything that ends up empty.
    let spans: Vec<(usize, usize, MarkSet)> = spans
        .iter()
        .filter(|s| s.is_not_useless())
        .map(|s| {
            let start = s.start.min(len);
            let end = s.end.min(len);
            (start, end, MarkSet::from_span(s))
        })
        .filter(|(start, end, _)| start < end)
        .collect();

    // Boundaries partition the body into runs of identical formatting. Overlapping spans
    // are handled by including every start and end as a cut point.
    let mut bounds: Vec<usize> = vec![0, len];
    for (start, end, _) in &spans {
        bounds.push(*start);
        bounds.push(*end);
    }
    bounds.sort_unstable();
    bounds.dedup();

    let mut runs: Vec<(usize, usize, MarkSet)> = Vec::new();
    for window in bounds.windows(2) {
        let (a, b) = (window[0], window[1]);
        if a >= b {
            continue;
        }
        let marks = spans
            .iter()
            .filter(|(s, e, _)| *s <= a && *e >= b)
            .fold(MarkSet::default(), |mut acc, (_, _, m)| {
                acc.bold |= m.bold;
                acc.italic |= m.italic;
                acc.monospace |= m.monospace;
                acc.strikethrough |= m.strikethrough;
                acc.link |= m.link;
                acc
            });

        // Merge with the previous run when the formatting matches, so output is stable
        // across a save/load cycle.
        match runs.last_mut() {
            Some(last) if last.1 == a && last.2 == marks => last.1 = b,
            _ => runs.push((a, b, marks)),
        }
    }

    // Split runs on newlines: a newline ends a paragraph. An empty paragraph is kept, so
    // "a\n\nb" becomes three paragraphs rather than two.
    let mut paragraphs: Vec<Vec<Value>> = vec![Vec::new()];
    for (a, b, marks) in runs {
        let text: String = chars[a..b].iter().collect();
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                paragraphs.push(Vec::new());
            }
            if !part.is_empty() {
                let mut node = json!({ "type": "text", "text": part });
                if !marks.is_empty() {
                    node["marks"] = Value::Array(marks.pm_marks(part));
                }
                paragraphs.last_mut().expect("never empty").push(node);
            }
        }
    }

    json!({
        "type": "doc",
        "content": paragraphs
            .into_iter()
            .map(|content| json!({ "type": "paragraph", "content": content }))
            .collect::<Vec<_>>(),
    })
}

/// Flatten a ProseMirror document back to a body and its spans.
///
/// Returns `(body, spans)`. The returned spans are normalised.
pub fn to_flat(doc: &Value) -> Result<(String, Vec<SpanRepresentation>)> {
    let paragraphs = doc
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("document has no content array"))?;

    let mut body = String::new();
    let mut spans: Vec<SpanRepresentation> = Vec::new();
    let mut offset = 0usize;

    for (i, para) in paragraphs.iter().enumerate() {
        // Reject any block that is not a paragraph. Without this, a stray heading would
        // be treated as a paragraph with no content and flatten to an empty body —
        // silent data loss, which is worse than an error.
        match para.get("type").and_then(Value::as_str) {
            Some("paragraph") => {}
            other => {
                return Err(anyhow!(
                    "unsupported block node type: {other:?}. \
                     The editor must be limited to paragraphs and inline marks."
                ))
            }
        }

        if i > 0 {
            body.push('\n');
            offset += 1;
        }
        let inline = para
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("paragraph {i} has no content array"))?;

        for node in inline {
            let kind = node.get("type").and_then(Value::as_str).unwrap_or("");
            match kind {
                "text" => {
                    let text = node
                        .get("text")
                        .and_then(Value::as_str)
                        .ok_or_else(|| anyhow!("text node has no text"))?;
                    let start = offset;
                    let marks = node.get("marks").and_then(Value::as_array);
                    let mut span = SpanRepresentation {
                        bold: false, link: false, italic: false,
                        monospace: false, strikethrough: false,
                        start, end: start,
                    };

                    for mark in marks.into_iter().flatten() {
                        match mark.get("type").and_then(Value::as_str) {
                            Some("bold") => span.bold = true,
                            Some("italic") => span.italic = true,
                            Some("strike") => span.strikethrough = true,
                            Some("code") => span.monospace = true,
                            Some("link") => span.link = true,
                            // An unknown mark is ignored rather than fatal, so adding a
                            // mark to the editor later cannot make old notes unreadable.
                            _ => {}
                        }
                    }

                    body.push_str(text);
                    offset += text.chars().count();
                    span.end = offset;
                    if span.is_not_useless() {
                        spans.push(span);
                    }
                }
                // ProseMirror's explicit line break, if the editor inserts one.
                "hardBreak" => {
                    body.push('\n');
                    offset += 1;
                }
                other => return Err(anyhow!("unsupported inline node type: {other}")),
            }
        }
    }

    Ok((body, normalize(&spans)))
}

/// Derive a link target from link text, mirroring Notally's `TakeNote.getURLFrom`.
///
/// Notally stores no URL alongside a link — the displayed text *is* the target. A bare
/// domain gains an `http://` prefix, an address becomes `mailto:`, and a phone number
/// becomes `tel:`.
pub fn url_from(text: &str) -> String {
    let t = text.trim();

    if PHONE.is_match(t) {
        return format!("tel:{t}");
    }
    if EMAIL.is_match(t) {
        return format!("mailto:{t}");
    }
    if DOMAIN.is_match(t) {
        return format!("http://{t}");
    }
    t.to_string()
}

// Approximations of android.util.Patterns.PHONE, .EMAIL_ADDRESS and .DOMAIN_NAME.
// Compiled once on first use rather than per call.
static PHONE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\+?[0-9][0-9\s\-()]{6,}$").expect("static pattern must compile")
});

static EMAIL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").expect("static pattern must compile")
});

static DOMAIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([a-zA-Z0-9]([a-zA-Z0-9-]*[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$")
        .expect("static pattern must compile")
});

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize) -> SpanRepresentation {
        SpanRepresentation {
            bold: true, link: false, italic: false,
            monospace: false, strikethrough: false,
            start, end,
        }
    }

    fn styled(
        start: usize, end: usize,
        bold: bool, italic: bool, mono: bool, strike: bool, link: bool,
    ) -> SpanRepresentation {
        SpanRepresentation {
            bold, link, italic, monospace: mono, strikethrough: strike, start, end,
        }
    }

    // ---- normalize ----

    #[test]
    fn normalize_merges_identical_ranges() {
        let merged = normalize(&[span(0, 5), styled(0, 5, false, true, false, false, false)]);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].bold && merged[0].italic);
    }

    #[test]
    fn normalize_drops_useless_and_empty_spans() {
        let out = normalize(&[
            styled(0, 5, false, false, false, false, false), // no attributes
            span(3, 3),                                     // zero width
        ]);
        assert!(out.is_empty());
    }

    #[test]
    fn normalize_keeps_different_ranges_apart() {
        assert_eq!(normalize(&[span(0, 5), span(6, 9)]).len(), 2);
    }

    // ---- to_pm ----

    #[test]
    fn plain_body_becomes_one_paragraph() {
        let doc = to_pm("hello", &[]);
        let content = &doc["content"];
        assert_eq!(content[0]["content"][0]["text"], "hello");
        assert!(content[0]["content"][0].get("marks").is_none());
    }

    #[test]
    fn all_five_styles_produce_marks() {
        let spans = [
            styled(0, 4, true, false, false, false, false),
            styled(4, 8, false, true, false, false, false),
            styled(8, 12, false, false, true, false, false),
            styled(12, 16, false, false, false, true, false),
            styled(16, 20, false, false, false, false, true),
        ];
        let doc = to_pm("aaaabbbbccccddddeeee", &spans);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        let kinds: Vec<&str> = nodes
            .iter()
            .map(|n| n["marks"][0]["type"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, vec!["bold", "italic", "code", "strike", "link"]);
    }

    #[test]
    fn link_mark_carries_a_derived_href() {
        let doc = to_pm("example.com", &[styled(0, 11, false, false, false, false, true)]);
        assert_eq!(
            doc["content"][0]["content"][0]["marks"][0]["attrs"]["href"],
            "http://example.com"
        );
    }

    #[test]
    fn overlapping_spans_split_into_boundary_runs() {
        // bold 0..5 and italic 3..8 over "abcdefgh"
        let doc = to_pm("abcdefgh", &[
            styled(0, 5, true, false, false, false, false),
            styled(3, 8, false, true, false, false, false),
        ]);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert_eq!(nodes.len(), 3, "expected plain/bold+italic/italic runs");

        let text_of = |i: usize| nodes[i]["text"].as_str().unwrap();
        assert_eq!(text_of(0), "abc");
        assert_eq!(text_of(1), "de");

        let marks_of = |i: usize| -> Vec<String> {
            nodes[i]["marks"]
                .as_array()
                .map(|m| {
                    m.iter()
                        .map(|x| x["type"].as_str().unwrap().to_string())
                        .collect()
                })
                .unwrap_or_default()
        };
        assert_eq!(marks_of(0), vec!["bold"], "abc is inside bold only");
        assert_eq!(marks_of(1), vec!["bold", "italic"], "de is inside both");
        assert_eq!(marks_of(2), vec!["italic"], "fgh is inside italic only");
    }

    #[test]
    fn unformatted_gaps_are_preserved() {
        let doc = to_pm("aaa bbb", &[span(4, 7)]);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert_eq!(nodes[0]["text"], "aaa ");
        assert!(nodes[0].get("marks").is_none(), "gap must not be bold");
    }

    #[test]
    fn adjacent_runs_with_same_marks_are_merged() {
        let doc = to_pm("abcdef", &[span(0, 3), span(3, 6)]);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "identical formatting should not fragment");
    }

    #[test]
    fn out_of_range_span_is_clamped_not_panicking() {
        let doc = to_pm("short", &[styled(0, 999, true, false, false, false, false)]);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert_eq!(nodes[0]["text"], "short");
    }

    #[test]
    fn span_entirely_past_the_end_is_dropped() {
        let doc = to_pm("short", &[styled(50, 60, true, false, false, false, false)]);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].get("marks").is_none());
    }

    #[test]
    fn newlines_become_paragraphs() {
        let doc = to_pm("a\nb", &[]);
        assert_eq!(doc["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn blank_line_produces_an_empty_paragraph() {
        let doc = to_pm("a\n\nb", &[]);
        let content = doc["content"].as_array().unwrap();
        assert_eq!(content.len(), 3);
        assert_eq!(content[1]["content"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn trailing_newline_produces_trailing_empty_paragraph() {
        let doc = to_pm("a\n", &[]);
        assert_eq!(doc["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn empty_body_gives_one_empty_paragraph() {
        let doc = to_pm("", &[]);
        let content = doc["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["content"].as_array().unwrap().len(), 0);
    }

    /// The one that would panic with a naive `&body[a..b]`.
    #[test]
    fn offsets_are_characters_not_bytes() {
        let body = "héllo wörld";           // 11 chars, 13 bytes
        assert_eq!(body.chars().count(), 11);
        assert!(body.len() > 11, "must actually differ from the char count");
        let doc = to_pm(body, &[span(6, 11)]); // "wörld"
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        let joined: String = nodes.iter().map(|n| n["text"].as_str().unwrap()).collect();
        assert_eq!(joined, "héllo wörld", "text must survive intact");
        let bolded: String = nodes
            .iter()
            .filter(|n| n.get("marks").is_some())
            .map(|n| n["text"].as_str().unwrap())
            .collect();
        assert_eq!(bolded, "wörld", "the span must land on char offsets 6..11");
    }

    #[test]
    fn emoji_offsets_do_not_split_a_character() {
        let doc = to_pm("hi 👋 there", &[span(3, 5)]);
        let nodes = doc["content"][0]["content"].as_array().unwrap();
        let joined: String = nodes
            .iter()
            .map(|n| n["text"].as_str().unwrap())
            .collect();
        assert_eq!(joined, "hi 👋 there");
    }

    // ---- to_flat ----

    #[test]
    fn to_flat_recovers_body_and_spans() {
        let (body, spans) = to_flat(&to_pm("hello world", &[span(6, 11)])).unwrap();
        assert_eq!(body, "hello world");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold);
        assert_eq!((spans[0].start, spans[0].end), (6, 11));
    }

    #[test]
    fn to_flat_handles_paragraphs() {
        let (body, _) = to_flat(&to_pm("a\nb\nc", &[])).unwrap();
        assert_eq!(body, "a\nb\nc");
    }

    #[test]
    fn to_flat_preserves_empty_paragraphs() {
        let (body, _) = to_flat(&to_pm("a\n\nb", &[])).unwrap();
        assert_eq!(body, "a\n\nb");
    }

    #[test]
    fn to_flat_handles_hard_breaks() {
        let doc = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": "a" },
                    { "type": "hardBreak" },
                    { "type": "text", "text": "b" }
                ]
            }]
        });
        let (body, _) = to_flat(&doc).unwrap();
        assert_eq!(body, "a\nb");
    }

    #[test]
    fn to_flat_ignores_unknown_marks() {
        let doc = json!({
            "type": "doc",
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text", "text": "x",
                    "marks": [{ "type": "underline" }]
                }]
            }]
        });
        let (_, spans) = to_flat(&doc).unwrap();
        assert!(spans.is_empty(), "unknown mark must not become a span");
    }

    #[test]
    fn to_flat_rejects_unsupported_block_nodes() {
        let doc = json!({
            "type": "doc",
            "content": [{ "type": "heading", "attrs": { "level": 1 }, "content": [] }]
        });
        assert!(to_flat(&doc).is_err(), "headings are out of scope and must error");
    }

    #[test]
    fn to_flat_rejects_malformed_documents() {
        assert!(to_flat(&json!({ "type": "doc" })).is_err());
        assert!(to_flat(&json!({})).is_err());
        assert!(to_flat(&json!({
            "type": "doc",
            "content": [{ "type": "paragraph", "content": [{ "type": "text" }] }]
        })).is_err());
    }

    // ---- round trip ----

    fn round_trip(body: &str, spans: &[SpanRepresentation]) {
        let normalized = normalize(spans);
        let doc = to_pm(body, &normalized);
        let (out_body, out_spans) = to_flat(&doc).expect("round trip must not fail");
        assert_eq!(out_body, body, "body changed for {body:?}");
        assert_eq!(out_spans, normalized, "spans changed for {body:?}");
    }

    #[test]
    fn round_trips_a_representative_sample() {
        round_trip("", &[]);
        round_trip("plain", &[]);
        round_trip("hello", &[span(0, 5)]);
        round_trip("hello world", &[span(6, 11)]);
        round_trip("a\nb\nc", &[span(2, 3)]);
        round_trip("a\n\nb", &[]);
        round_trip("trailing\n", &[]);
        round_trip("unformatted gap", &[span(4, 8)]);
        round_trip("all styles here", &[
            styled(0, 3, true, false, false, false, false),
            styled(4, 9, false, false, true, false, false),
        ]);
    }

    #[test]
    fn round_trips_unicode_bodies() {
        round_trip("héllo wörld", &[span(6, 11)]);
        round_trip("日本語のテキスト", &[span(0, 3), span(4, 7)]);
        round_trip("emoji 👋 and text", &[span(6, 15)]);
        round_trip("é́ combining", &[span(0, 2)]);
    }

    #[test]
    fn round_trips_whole_body_styled() {
        let body = "everything here"; // 15 characters
        assert_eq!(body.chars().count(), 15);
        round_trip(body, &[span(0, body.chars().count())]);
    }

    #[test]
    fn round_trip_is_idempotent() {
        let spans = [styled(0, 5, true, false, false, false, false),
                     styled(3, 8, false, true, false, false, false)];
        let first = to_flat(&to_pm("abcdefgh", &normalize(&spans))).unwrap();
        let second = to_flat(&to_pm(&first.0, &first.1)).unwrap();
        assert_eq!(first, second);
    }

    // ---- url_from ----

    #[test]
    fn derives_targets_like_notally() {
        assert_eq!(url_from("5551234"), "tel:5551234");
        assert_eq!(url_from("+1 555 123 4567"), "tel:+1 555 123 4567");
        assert_eq!(url_from("a@b.com"), "mailto:a@b.com");
        assert_eq!(url_from("example.com"), "http://example.com");
        assert_eq!(url_from("sub.example.co.uk"), "http://sub.example.co.uk");
        assert_eq!(url_from("https://example.com/x"), "https://example.com/x");
        assert_eq!(url_from("just words"), "just words");
    }
}
