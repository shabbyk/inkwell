# Notally Web

A self-hosted, server-hosted notes app — a web recreation of the
[Notally](https://github.com/OmGodse/Notally) Android app, with portable zip storage.

Rich text notes and checklists, labels, colours, pinning, archive/trash, image attachments,
search, and one-click backup you can zip up and carry.

## Why it exists

Self-host your own notes. No account, no telemetry, no third-party service. Your data is a
SQLite file plus a `media/` folder — readable, inspectable, and yours. Import a backup
exported from the Android app and it all comes across.

## Stack

| Layer | Choice |
|---|---|
| Server | Rust — [axum](https://github.com/tokio-rs/axum) + [rusqlite](https://github.com/rusqlite/rs-sqlite) |
| Frontend | React + TypeScript + [Vite](https://vitejs.dev) |
| Editor | [TipTap](https://tiptap.dev) (ProseMirror) |
| Storage | SQLite file + `media/` directory |

Chosen for a small memory footprint: the server process runs in roughly 10–20 MB, so this is
comfortable on a small VPS or a home box.

## Status

In development. See [`docs/plan.md`](docs/plan.md) for the full plan and
[`docs/TASKS.md`](docs/TASKS.md) for the task breakdown.

## Building

Not yet wired up. Planned layout:

```
server/   Rust crate (axum API + static file serving)
client/   React + Vite + TipTap frontend
data/     runtime state — SQLite database and media. Gitignored.
```

## Portability

Export produces a single zip:

```
notally-20260927-143012.zip
├── manifest.json     version, timestamp, note count
├── notes.json        all notes, labels, settings
└── media/
    ├── images/
    └── audios/
```

Import auto-detects three formats: the JSON above, an Android app backup
(`.zip` containing a SQLite file plus `Images/` and `Audios/`), and the legacy Android XML
dump. Notes from the phone app import without conversion.

## Security notes

- Single user with a password (Argon2id)
- Session cookie is `HttpOnly` + `SameSite=Strict`
- Serve behind TLS in production; the app refuses to issue session cookies over plain
  HTTP unless explicitly allowed for local development
- `.env` holds the session secret and is gitignored

## Licence

Not yet decided. The original Notally app is by
[Om Godse](https://github.com/OmGodse/Notally) — see that project for its licence.
