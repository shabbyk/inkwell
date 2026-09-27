# T1 — Label CRUD

**Difficulty: easy. Safe for a smaller local model.**

## Goal

Create `server/src/labels.rs` with functions to create, rename, delete, and list labels,
and to find notes carrying a given label.

## Create

File: `server/src/labels.rs`

Add this line to `server/src/main.rs` alongside the existing `mod` declarations:

```rust
mod labels;
```

## The `Label` table

```sql
CREATE TABLE IF NOT EXISTS Label (
    value TEXT PRIMARY KEY
);
```

Already created by `db::init`. Do not add a migration.

## Signatures to implement

All take `&rusqlite::Connection` and return `anyhow::Result`:

```rust
/// Every label, sorted alphabetically, case-insensitively.
pub fn list(conn: &Connection) -> Result<Vec<String>>

/// Insert a label. Ignored if it already exists — does not error on conflict.
pub fn create(conn: &Connection, value: &str) -> Result<()>

/// Rename a label everywhere it is used.
///
/// `value` is stored as a JSON array inside each note's `labels` TEXT column, so it
/// cannot be updated with SQL alone. Load each note, rewrite the array, write it back.
/// A rename must not collide: if `new_value` already exists, merge into it and delete
/// the old label rather than failing.
pub fn rename(conn: &Connection, old_value: &str, new_value: &str) -> Result<()>

/// Delete a label from the `Label` table and strip it from every note that carries it.
pub fn delete(conn: &Connection, value: &str) -> Result<()>

/// Notes in `folder` that carry `label`. Sorted pinned first, then newest first,
/// matching Notally's ordering.
pub fn notes_with_label(conn: &Connection, label: &str, folder: &str) -> Result<Vec<models::Note>>
```

## Important: exact label matching

Notally's original search used SQL `LIKE '%label%'`, which returns false positives — asking
for `"Important"` also matches `"Unimportant"`. Notally then filtered the results in Kotlin
to compensate. **Do not do the `LIKE` trick.** Filter properly in Rust:

```rust
notes.into_iter().filter(|n| n.labels.iter().any(|l| l == label)).collect()
```

The `SELECT` may use `LIKE` as a cheap pre-filter, but the Rust filter above is what makes
the result correct. A test must cover the `"Important"` / `"Unimportant"` case.

## Reading and writing notes

Use `models::NoteRow` to read and write. Example:

```rust
let mut stmt = conn.prepare("SELECT * FROM BaseNote")?;
let rows = stmt.query_map([], |row| {
    Ok(models::NoteRow {
        id: row.get(0)?,
        note_type: ...,
        // ...
    })
})?;
```

Getting the `type` and `folder` enums back needs explicit mapping, because SQLite stores
them as TEXT and `NoteType` derives `Deserialize` with `rename_all = "UPPERCASE"` — that
means the string form is `"NOTE"`, not `"Note"`. Write a small helper:

```rust
fn parse_type(s: &str) -> models::NoteType {
    match s { "LIST" => models::NoteType::List, _ => models::NoteType::Note }
}
```

Do the same for `Folder` and `Color`. A `DEFAULT` colour column is `"DEFAULT"`.

## Writing a note back

Build a `models::NoteRow` with `.to_row()` on a `models::Note`, then write all fourteen
columns. Do not use `INSERT OR REPLACE` on `BaseNote` for updates — it deletes and
reinserts, which changes the rowid. Use `UPDATE BaseNote SET ... WHERE id = ?`.

## Tests to write

Put them in the same file under `#[cfg(test)]`. Use
`Connection::open_in_memory()` then `db::init(&conn)`.

- `create_then_list_returns_it`
- `create_is_idempotent` — creating the same label twice leaves one row
- `rename_updates_the_label_table`
- `rename_rewrites_notes_using_it` — the note's JSON array now holds the new value
- `rename_into_existing_merges` — renaming `a` to `b` when `b` exists results in one
  label `b`, and no note has a duplicate `b` in its array
- `delete_strips_from_notes`
- `label_match_is_exact_not_substring` — the important one. Create labels `Important` and
  `Unimportant`, attach both to one note, and assert `notes_with_label("Important")`
  returns exactly one note.

## Commit

```
feat(server): label CRUD with exact-match filtering
```
