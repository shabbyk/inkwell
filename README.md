# Inkwell

A self-hosted notes app for the web. Rich text notes and checklists, labels, colours,
pinning, archive and trash, image attachments, search, and a backup you can zip up and carry.

## Why it exists

Host your own notes. No account, no telemetry, no third-party service. Your data is a SQLite
file plus a `media/` folder — readable, inspectable, and yours. Import a backup exported from
the [Notally](https://github.com/OmGodse/Notally) Android app and everything comes across.

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

**In development.** Not yet usable — see [`docs/plan.md`](docs/plan.md) for the plan and
[`docs/TASKS.md`](docs/TASKS.md) for the task breakdown.

Planned layout:

```
server/   Rust crate (axum API + static file serving)
client/   React + Vite + TipTap frontend
data/     runtime state — SQLite database and media. Gitignored.
```

## Portability

Export produces a single zip:

```
inkwell-20260927-143012.zip
├── manifest.json     version, timestamp, note count
├── notes.json        all notes, labels, settings
└── media/
    ├── images/
    └── audios/
```

Import auto-detects three formats: the JSON above, an Android Notally backup (`.zip`
containing a SQLite file plus `Images/` and `Audios/`), and the legacy Notally XML dump. Notes
from the phone app import without conversion.

## Security

- Single user with a password (Argon2id)
- Session cookie is `HttpOnly` + `SameSite=Strict`
- Serve behind TLS in production
- `.env` holds the session secret and is gitignored

## Credits and licence

Inkwell is an **independent reimplementation** of the features of
[Notally](https://github.com/OmGodse/Notally) by Om Godse, written from scratch in a different
language and architecture. The Notally data model, export formats, and colour palette are
reproduced so that backups are interchangeable between the two apps.

No source code was copied from Notally. Notally is licensed GPL-3.0; because no Notally code
is included here, this project is licensed independently under the MIT licence below.

If any substantive Notally code is incorporated in future, this project's licence must be
reconsidered.

MIT — see [`LICENSE`](LICENSE).

The original Notally app is © Om Godse and contributors, and is available under GPL-3.0.
