# T3 — Portable zip export

**Difficulty: moderate. Safe for a smaller local model, with care around file paths.**

## Goal

Create `server/src/export.rs` — write a complete backup as a zip file.

## Create

File: `server/src/export.rs`. Add to `src/main.rs`:

```rust
mod export;
```

## Output layout

```
inkwell-20260927-143012.zip
├── manifest.json
├── notes.json
└── media/
    ├── images/<name>
    └── audios/<name>
```

## Signatures

```rust
pub fn write_backup(
    conn: &Connection,
    media_root: &Path,
    out_path: &Path,
    app_version: &str,
) -> Result<()>
```

## Steps

1. **Read every note.** `SELECT * FROM BaseNote`. Convert each `NoteRow` to a `Note` with
   `.to_note()`.

2. **Build `notes.json`.** A `models::NotesFile` holds `notes`, `labels`, and `settings`.
   - `notes` is `Vec<NoteExport>`, built with `NoteExport::from(&note)`.
   - `labels` is every value from the `Label` table, sorted.
   - `settings` is `BTreeMap<String, String>` from `settings::all(conn)`.

   `NoteExport` already handles the awkward parts: the `date-created` key, and omitting
   `body`/`spans` for lists and `items` for notes. Do not hand-roll this — use the type.

3. **Build `manifest.json`.** A `models::Manifest` with `version: 1`, `note_count`, the
   app version, and `exported_at` as an RFC 3339 timestamp. `time` 0.3 is a dependency;
   the formatting feature is not currently enabled. Either add it with
   `cargo add time --features formatting` or format the timestamp manually as
   `YYYY-MM-DDTHH:MM:SSZ` from a `SystemTime` duration since the Unix epoch. Manual
   formatting is fine and avoids touching the dependency set.

4. **Write the zip** with the `zip` crate, version 5. Note the current API:
   ```rust
   let file = std::fs::File::create(out_path)?;
   let mut zip = zip::ZipWriter::new(file);
   // options: zip::write::SimpleFileOptions::default()
   zip.start_file("notes.json", options)?;
   zip.write_all(&json_bytes)?;
   zip.finish()?;
   ```

5. **Copy media.** For each distinct filename in every note's `images`, copy
   `media_root/images/<name>` to the zip entry `media/images/<name>`. Same for
   `audios` from `media_root/audios/<name>` to `media/audios/<name>`.

## Two rules that matter

**A missing media file must not fail the export.** Notally logs the error and continues
(`BaseNoteModel.exportBackup`). A backup that aborts because one image was deleted from
disk is worse than a backup with one image missing — and the note's reference is still
recorded in `notes.json`, so nothing is silently lost. Skip and continue.

**Deduplicate by filename.** The same image may be referenced by several notes, or the
same note may be listed twice. Use a `HashSet` so each file is written once. Duplicate
zip entries are a real corruption risk, not just bloat.

## Path handling — read this

The filename in `Image.name` is untrusted input that came out of an imported backup. It
could be `../../../etc/passwd`.

Before opening any path, validate:

```rust
fn safe_join(root: &Path, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    let p = root.join(name);
    // confirm the result is still inside root
    p.starts_with(root).then_some(p)
}
```

Return `None` for a rejected name and skip the file. The archive entry name is built as
`format!("media/images/{name}")` — a name containing a slash would also produce a nested
entry path, which is why the slash check matters twice over.

## Zip entry names

Always forward slashes, never OS separators. The export is consumed on other platforms.

## Tests

- `backup_contains_manifest_and_notes` — open the result with `zip::ZipArchive` and assert
  both entries exist
- `manifest_version_is_one` and parses as `models::Manifest`
- `notes_json_round_trips` — deserialize into `models::NotesFile` and compare
  `notes.len()` to the number of rows
- `media_files_are_included`
- `duplicate_image_written_once` — two notes referencing one image, one zip entry
- `missing_media_file_does_not_fail` — reference a filename that does not exist on disk;
  the export still succeeds and the zip is readable
- `path_traversal_is_rejected` — an image named `../../etc/passwd`; assert the export
  succeeds and no such entry exists in the zip

## Commit

```
feat(server): portable zip export with manifest, notes and media
```
