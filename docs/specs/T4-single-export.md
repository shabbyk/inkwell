# T4 — Single-note TXT and HTML export

**Difficulty: easy to moderate. Safe for a smaller local model.**

## Goal

Create `server/src/single_export.rs` — render one note as plain text or as a standalone
HTML document, matching Notally's output closely enough that a user cannot tell the
difference.

## Create

File: `server/src/single_export.rs`. Add to `src/main.rs`:

```rust
mod single_export;
```

## Signatures

```rust
pub fn to_txt(note: &models::Note, show_date_created: bool) -> String
pub fn to_html(note: &models::Note, show_date_created: bool) -> String
```

## TXT output

Reproduce Notally's `BaseNoteModel.getTXTFile`:

```
{title}\n\n            <- only if title is not empty
{date}\n\n             <- only if show_date_created
{body}
```

For a list, the body is the items rendered as `[✓] item` or `[ ] item`, one per line —
this is `Operations.getBody()` in the Kotlin source. Use U+2713 CHECK MARK for the ticked
box and an ASCII space for the unticked one.

The date is formatted in full, e.g. `Monday, 27 September 2026`. Notally uses
`DateFormat.getDateInstance(DateFormat.FULL)`. Getting the exact locale-dependent string
is not required, but it must be a human-readable absolute date, never a relative one.

## HTML output

Reproduce Notally's `BaseNoteModel.getHTML`:

```html
<!DOCTYPE html>
<html><head>
<meta charset="UTF-8"><title>{escaped title}</title>
<style>h1{letter-spacing:0.5px}body{letter-spacing:0.25px;line-height:1.3}</style>
</head><body>
<h2>{escaped title}</h2>
<p>{date}</p>                          <- only if show_date_created
{body or list markup}
</body></html>
```

Keep that CSS and structure. It is a small amount of code and reproducing it means output
matches the Android app.

For a **note**, convert the body plus `spans` into HTML. Bold → `<b>`, italic → `<i>`,
monospace → `<code>`, strikethrough → `<s>`, link → `<a href="...">`. For a **list**,
Notally emits:

```html
<ol style="list-style: none; padding: 0;">
<li><input style="margin-right:8px;" type="checkbox" checked>escaped body</li>
</ol>
```

with `checked` present only when the item is ticked.

## Escaping — get this right

**Every piece of user text must be HTML-escaped before insertion.** Note titles, note
bodies, and list item bodies are all attacker-controlled once a backup from an untrusted
source can be imported. Escape at minimum `&`, `<`, `>`, `"`, and `'`. A missing escape
here is a stored-XSS vulnerability, not a cosmetic bug.

If you build tags by slicing a body string at `SpanRepresentation` offsets, note that the
slice boundaries can fall inside a character. See the span ordering note below.

## Span handling — the part to be careful about

`SpanRepresentation` gives `start`/`end` offsets into `body`, inclusive of `start` and
exclusive of `end`. Several attributes can be set on one span, and separate spans can
overlap.

To render correctly:

1. Collect the ranges that need each style.
2. Find the boundaries — every `start` and `end` offset.
3. Sort the boundaries, walk them, and track which styles are active in each segment.
4. Escape the text of each segment, then wrap it.

A simple approach that is *wrong* if applied naively: sorting spans by `start` and emitting
them one after another. That breaks as soon as two spans overlap or nest. The
boundary-walk approach handles both.

**Safety requirement:** if a span's `end` exceeds `body.len()`, clamp it to `body.len()`
and skip the span if `start >= end`. Malformed spans from a hand-edited or corrupt backup
must never panic. Write a test for this.

A full round-trip implementation is being written separately in `spans.rs`. **Do not create
that module.** These two functions only need one-way rendering, and the boundary-walk is
self-contained. If it turns out to be more than about 60 lines, stop and report back —
that is a sign it belongs in `spans.rs` and needs the primary agent.

## Tests

- `txt_note_includes_title_and_body`
- `txt_omits_empty_title` — no leading blank lines
- `txt_list_uses_checkboxes` — assert the output contains `[✓] a` and `[ ] b`
- `txt_omits_date_when_disabled`
- `html_escapes_title` — a title of `<script>alert(1)</script>` must come out as
  `&lt;script&gt;…`
- `html_escapes_list_item_body` — same check for a list item
- `html_list_renders_checkbox_inputs`
- `html_applies_all_five_styles` — a note with one span per attribute; assert `<b>`, `<i>`,
  `<code>`, `<s>`, and `<a ` all appear
- `html_handles_overlapping_spans` — spans at `0..5` bold and `3..8` italic; assert both
  appear and the function does not panic
- `out_of_range_span_is_clamped` — a span with `end` beyond `body.len()`; assert no panic
  and no panic on a zero-width span either
- `empty_note_produces_valid_document` — still emits the doctype and closing tags

## Commit

```
feat(server): single-note TXT and HTML export
```
