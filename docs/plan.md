# Notally Web — Server-Hosted Notes App

Recreate the Notally Android notes app as a self-hosted web application with portable,
zip-carried storage.

**Status:** Plan approved, not yet started. Requires switching to a build agent.

---

## 1. Locked Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Backend | Rust — `axum` + `rusqlite` (bundled SQLite) | Smallest RAM footprint of all options considered |
| Frontend | React + TypeScript + Vite | Static bundle costs the server 0 RAM |
| Editor | TipTap (ProseMirror) | Best fit for the `SpanRepresentation` model |
| Auth | Single user, Argon2id + HttpOnly session cookie | Simplest model for personal self-hosting |
| Storage | SQLite file + `media/` directory | Portable, inspectable |
| Export format | Readable `notes.json` + `media/` in a zip | Human-readable, diffable, version-independent |
| Import formats | All three (new JSON, Android `.zip`, legacy XML) | So existing phone notes carry over |
| Search | FTS5, with LIKE fallback | Replaces the original's LIKE + post-filter hack |
| Reminders | **Deferred — data only** | Column and enum preserved; no UI, nothing fires |
| Repository | **Separate new repo** (`~/Dev/notally-web`), not the Android fork | See section 10 |

### Why Rust

Typical idle RSS for a small notes app of this shape:

| Option | Idle RAM | Processes | Build |
|---|---|---|---|
| Next.js full-stack | ~120–200 MB | 1 | Next build (heavy) |
| Node + Fastify | ~45–80 MB | 1 | esbuild/Vite (fast) |
| Python + FastAPI | ~50–90 MB | 1 (+worker) | pip install |
| **Rust + axum** | **~5–15 MB** | 1 | Cargo (~1–3 min) |

The difference is structural, not incremental: no garbage collector, no interpreter, no
framework runtime. `axum` + `rusqlite` (statically bundling SQLite) compiles to one binary
mapping a few MB resident. Incidental benefit: no GC pauses, so save latency stays flat as
note count grows.

The frontend is a static build in **all** options — a static bundle costs the server zero RAM.
So the backend choice is purely footprint plus dev ergonomics, and the editor choice is
identical regardless. Deployment is: copy one binary + `dist/` onto a box and run it.

---

## 2. Source Analysis (NotallyOnPrem Android app)

Total Kotlin LOC: **7,894**.

### Data model

`BaseNote` (`room/BaseNote.kt:8-23`), a single Room entity, SQLite version 5:

| Field | Type | Notes |
|---|---|---|
| `id` | Long | autoincrement primary key |
| `type` | enum | `NOTE` \| `LIST` (`Type.kt:3`) |
| `folder` | enum | `NOTES` \| `DELETED` \| `ARCHIVED` (`Folder.kt:3`) — soft delete |
| `color` | enum | 12 pastels (see below) |
| `title` | String | |
| `body` | String | |
| `spans` | List\<SpanRepresentation\> | JSON; bold, italic, monospace, strikethrough, link + start/end offsets |
| `items` | List\<ListItem\> | JSON; `{body, checked}` for checklists |
| `images` | List\<Image\> | JSON; `{name, mimeType}` |
| `audios` | List\<Audio\> | JSON; `{name, duration, timestamp}` |
| `labels` | List\<String\> | JSON array; global label list in separate `Label` table |
| `pinned` | Boolean | pinned sort first, with Pinned/Others headers |
| `timestamp` | Long | creation date |
| `reminder` | Reminder? | nullable JSON; `timestamp` + `frequency` |

Enums and value types: `Frequency { ONCE, DAILY, MONTHLY }`, `SpanRepresentation`
(carries `isNotUseless()` and `isEqualInSize()`), `ListItem`, `Reminder`, `Image`, `Audio`.

All list/map fields are stored as JSON strings via Room `@TypeConverters` in
`room/Converters.kt`, not as normalized tables.

### Colour palette (verbatim from `res/values/colors.xml`)

```
DEFAULT #FFFFFF   CORAL  #FAAFA9   ORANGE #FFCC80   SAND  #FFF8B9
STORM   #AFCCDC   FOG    #D3E4EC   SAGE   #B4DED4   MINT  #E2F6D3
DUSK    #D3BFDB   FLOWER #F8BBD0   BLOSSOM #F5E2DC  CLAY   #E9E3D3
```

### Sorting

`ORDER BY pinned DESC, timestamp DESC` (`BaseNoteDao.kt:52-53`).

### Current portable storage

- Export backup = `.zip` containing the raw SQLite file plus `Images/` and `Audios/`
  directories (`Export.kt:12-36`).
- Import also reads a legacy XML dump (`legacy/XMLUtils.kt`, 129 lines).
- Single notes export to TXT / JSON / HTML / PDF.

### Notable details worth preserving

- **Search is `LIKE` + post-filter.** Labels are stored as a JSON string so cannot be queried
  relationally; the app uses `LIKE` and re-filters in Kotlin to kill false positives from
  substring matches (`BaseNoteDao.kt:150-195`). A web version should index properly instead.
- **Migrations 2–5** added `color`, `images`, `audios`, `reminder` progressively
  (`NotallyDatabase.kt:49-75`). Older backups therefore lack columns.
- **Import deliberately ignores unreferenced files** in an uploaded zip — a 100 MB stray image
  is skipped (`BaseNoteModel.kt:262-264`).
- **Span de-duplication.** Overlapping spans with identical extents are merged on save
  (`NotallyModel.kt:417-444`).
- **Pinned/Others headers** are a list transform, not a DB feature
  (`BaseNoteModel.kt:662-681`).
- **List body rendering** for sharing and notifications uses `[✓]` / `[ ]` prefixes
  (`Operations.kt:99-104`).
- **Preview truncation** via `maxItems` / `maxLines` / `maxTitle`.

### Code reuse analysis — the honest number

| Layer | LOC | Reusable? |
|---|---|---|
| Activities, Fragments, Views, 15 adapters, viewholders | ~4,000 | No — ViewBinding, Glide, LiveData, navigation |
| ViewModels (`BaseNoteModel`, `NotallyModel`) | ~1,050 | No — Room, AndroidViewModel, AlarmManager |
| Room DAOs, `NotallyDatabase`, `Content` | ~380 | No — but pure logic inside is salvageable |
| Services (audio), widgets, `ReminderReceiver` | ~900 | No |
| **Domain model** (13 Android-free files in `room/`) | **~62** | **Yes, verbatim** |
| `legacy/XMLUtils.kt` | 129 | Yes, verbatim |
| `Converters.kt` | 157 | Translate — `org.json` → `kotlinx-serialization` |
| `getBody()`, `matchesKeyword()`, `transform()`, `getJSON()`, `getHTML()` | ~150 | Translate — buried in Android-coupled files |

**~200–250 lines are literally copy-pasteable; another ~300 are logic to rewrite against a
new type system. About 7% of 7,894 total.** Most of the "clean" files are 2-line enums
(`Type.kt`, `Folder.kt`, `Frequency.kt`), so the real saving is roughly an hour.

**Code reuse is not format reuse — and format reuse is what delivers portability.** The
things worth preserving exactly are *contracts*: the JSON shape from `getJSON()`
(`BaseNoteModel.kt:598-619`), the zip layout, the colour hex values, and the
`SpanRepresentation` offsets. Keeping those identical lets the web app import a backup
exported from the Android app. That interop is achievable in any language at near-zero cost,
which is why the Rust choice does not forfeit the valuable part of "reuse".

Ktor/Kotlin-JS was considered and rejected: it would reuse ~100% of the domain model but
inherit the JVM's ~50–100 MB floor, forfeiting the RAM goal. There is no hybrid that works —
a Rust backend with a Kotlin/JS frontend would need the model written twice, and cross-language
sharing across an HTTP boundary is copy-paste, not reuse.

---

## 3. Architecture

Rooted at `~/Dev/notally-web` — a **standalone repository**, sibling to the Android fork
(see section 10 for why).

```
notally-web/
├── server/                 Rust crate
│   ├── src/
│   │   ├── main.rs         binary entry, static file serving
│   │   ├── db.rs           schema, migrations, pooled connection
│   │   ├── models.rs       BaseNote, ListItem, SpanRepresentation, ...
│   │   ├── spans.rs        PM JSON <-> flat span offsets  [HIGH RISK]
│   │   ├── auth.rs         Argon2id, session cookies
│   │   ├── routes/         notes, labels, media, settings, backup
│   │   ├── search.rs       FTS5 + LIKE fallback
│   │   └── import/         detect.rs, json.rs, android.rs, xml.rs
│   └── Cargo.toml
├── client/                 React + Vite + TipTap
├── docs/
│   └── plan.md             copy of this plan
└── data/                   runtime state — gitignored, never committed
    ├── notally.sqlite
    └── media/{images,audios}/
```

Runtime: one Rust process (~10–20 MB) serving the JSON API and the static `dist/` bundle.

---

## 4. Data Model

Schema stays **byte-identical to Room's**, so Android `.zip` import is a straight row copy:

```sql
CREATE TABLE BaseNote (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  type TEXT NOT NULL, folder TEXT NOT NULL, color TEXT NOT NULL,
  title TEXT NOT NULL, pinned INTEGER NOT NULL, timestamp INTEGER NOT NULL,
  labels TEXT NOT NULL, body TEXT NOT NULL, spans TEXT NOT NULL,
  items TEXT NOT NULL, images TEXT NOT NULL, audios TEXT NOT NULL,
  reminder TEXT
);
CREATE TABLE Label (value TEXT PRIMARY KEY);
CREATE TABLE Setting (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE User (id INTEGER PRIMARY KEY, username TEXT, password_hash TEXT);
```

Exact column names, types, and order are deliberate — that is what makes Android interop
nearly free.

**Backward compatibility:** migrations 2–5 added columns progressively, so older backups may
lack `color`, `images`, `audios`, `reminder`. Import detects via `PRAGMA table_info` and
defaults missing ones, porting the `getColumnIndex(...) != -1` logic from
`BaseNoteModel.kt:366-381`.

**Search:** FTS5 virtual table over title/body/items/labels, rebuilt after every import.
Never appears in the export. Falls back to the original LIKE + post-filter strategy if FTS5
is unavailable in the build.

**Settings** mirror `Preferences.kt` / `ListInfo.kt`: `view` (list\|grid), `theme`
(dark\|light\|followSystem), `dateFormat` (none\|relative\|absolute), `textSize`
(small\|medium\|large), `maxItems`, `maxLines`, `maxTitle`.

**Media:** `data/media/images/<uuid>.<ext>`, `data/media/audios/<uuid>.m4a`, served through
an authenticated route. Accepts JPG/PNG/WEBP only, matching the original.

---

## 5. Portable Formats

### Export (new readable format)

```
notally-20260927-143012.zip
├── manifest.json     # version, exportedAt, noteCount, appVersion
├── notes.json        # all notes, labels, settings
└── media/
    ├── images/<uuid>.<ext>
    └── audios/<uuid>.m4a
```

`notes.json` note shape matches `getJSON()` (`BaseNoteModel.kt:598-619`) so round-tripping
with the Android app is lossless:
`type`, `color`, `title`, `pinned`, `date-created`, `labels`, `body`, `spans`, `items`.

### Import — auto-detect all three by sniffing

1. **`notes.json` present** → new format.
2. **`NotallyDatabase` entry present** → Android format. `ATTACH` the SQLite file,
   `INSERT INTO main.BaseNote SELECT * FROM imported.BaseNote`, then rebuild FTS5.
3. **Root element is XML** → legacy. `legacy/XMLUtils.kt` ports directly (dependency-free).

Images referenced by notes are copied; unreferenced files are ignored, porting
`BaseNoteModel.kt:262-264`. Reminders imported are preserved in storage but inert.

---

## 6. Highest-Risk Component

TipTap works in ProseMirror JSON; storage uses flat `start`/`end` integer offsets. A
`spans.rs` module converts both directions:

- **save:** PM JSON → flat `SpanRepresentation[]`, including the same de-duplication as
  `getFilteredRepresentations` (`NotallyModel.kt:417-444`).
- **load:** flat spans → PM JSON marks.

This round-trip determines whether an Android backup renders correctly. Write it early and
unit-test it hardest. Edge cases: overlapping spans, zero-width spans, and stale spans
pointing past `body.len()` after a bad import.

---

## 7. Feature Scope

### In scope (core set)

- Notes and checklists
- Rich text: bold, italic, monospace, strikethrough, link
- 12 colours, verbatim hex values
- Labels: create, rename, delete, assign to notes
- Pin, with Pinned/Others headers
- Folders: NOTES / DELETED / ARCHIVED — archive, restore, delete-forever
- Image attachments (JPG/PNG/WEBP)
- Search across title, body, items, labels
- Settings: list/grid view, theme, date format, text size, preview truncation
- Zip export and import (all three formats)
- Single-note export: TXT, JSON, HTML
- Multi-select action mode: pin, copy, label, recolor, archive, delete, restore, delete-forever

### Deferred — data preserved where noted

- **Reminders** — `reminder` column and `Frequency` enum preserved in schema and export.
  Imported reminders round-trip losslessly; no UI, nothing fires. No migration needed later.
- **Audio recording/playback** — `audios` column and media path preserved.
- **PDF export** — a headless browser would cost more RAM than the entire rest of the server.
- **Web Share API** — browser share sheet.
- **Widgets** — no web analogue.

### How reminders would work later, if wanted

The Android app arms a one-shot exact `AlarmManager` alarm keyed to the note ID, then on each
fire recomputes and re-arms the next occurrence from the stored wall-clock time. Monthly
recurrence clamps the day: `min(getActualMaximum(DAY_OF_MONTH), dayOfMonth)`, so a 31st
reminder fires on the 30th in April. Because Android wipes the alarm table on reboot, all
reminders are re-armed on `ACTION_BOOT_COMPLETED` and after backup import.

The web equivalent is a server-side tokio interval scanning due reminders, pushed via Web
Push (VAPID keys, service worker, permission prompt). This is *more* reliable than the
Android version, since nothing is lost when a device restarts and the re-arm-on-boot dance
disappears. The blocker is push subscription infrastructure, not scheduling.

---

## 8. Build Order

1. Scaffold, schema, Argon2id auth, session middleware
2. Note CRUD API + label / colour / pin / folder operations
3. Frontend shell: navigation drawer, list, settings
4. Editor + `spans.rs` round-trip — **the risky part, done early**
5. Labels, colours, pinning, folder actions, multi-select
6. Image upload and display
7. Search
8. Export and the three import paths
9. Polish: truncation, theming, text size, grid view

---

## 9. Verification

- `cargo test`
  - span round-trip: convert → load → convert is identity
  - one fixture test per import format, built from the real Android schema
  - PRAGMA-based column-missing path for pre-migration-2 backups
- `vitest` for frontend components
- Manual: import an actual `.zip` exported from this Android build; confirm every note,
  label, colour, and image survives, including reminder field round-trip

---

## 10. Repository & Git Workflow

### Separate repo, not the Android fork

`/home/shahbaaz-deb/Dev/NotallyOnPrem` is a **fork of `OmGodse/Notally`**, with `origin` at
`shabbyk/NotallyOnPrem` and `master` identical to `origin/main` (0 divergence). Recent commits
are authored by the upstream maintainer, so this repo is used to track and ship the Android
app.

The web app gets its own repository. Reasons:

1. **Upstream tracking.** Thousands of lines of unrelated Rust in a `web/` tree make every
   future fetch/merge from upstream noisier, benefiting neither project.
2. **Distinct purpose.** The fork exists to ship the Android app; the web app has a different
   audience, CI, and release cadence.
3. **Inverted ownership.** The Android code is not ours; the web app fully is. Coupling them
   makes our work inherit someone else's maintenance decisions.

The Android fork stays a clean mirror and is never modified by this work.

### Setup

```bash
mkdir -p ~/Dev/notally-web && cd ~/Dev/notally-web && git init
mkdir -p docs && cp ~/.opencode/plan/notally-web.md docs/plan.md
git switch -c feat/initial-scaffold
```

### ⚠️ The notes database must never reach git

If `data/notally.sqlite` or `data/media/` is committed and pushed, real private notes become
public in git history — and deleting them afterward does **not** remove the blobs. Commit
`.gitignore` first, in its own commit, before any code:

```gitignore
/target/          # Rust build artifacts
/node_modules/
/dist/            # Vite build output
/data/            # notes DB and media — NEVER commit
*.sqlite
*.sqlite-wal
*.sqlite-shm
.env              # session secret, Argon2 salt
```

### Branching and commit discipline

Never build on `master`. A Rust + React build of this size touches 30–50 files across a
language switch; a single "add web app" commit is unreviewable and cannot be bisected or
partially reverted. **One commit per phase** from the build order in section 8, using
conventional commits:

```bash
git commit -m "chore: add gitignore for Rust/Node artifacts and notes data"
git commit -m "feat(server): scaffold axum app with Notally schema"
git commit -m "feat(spans): PM JSON <-> flat SpanRepresentation round-trip"
```

Isolating `spans.rs` in its own commit means it can be reverted, bisected, or reviewed alone —
it is the component most likely to need rework.

Conventions:

- **Conventional commits** (`feat:`, `fix:`, `chore:`) for free changelogs and version bumps
- **Open a PR against `main` even working solo** — forces CI to run and provides a place to
  catch mistakes before merge
- **Tag releases** (`v0.1.0`) so a bad deploy is one `git revert` away
- **Private remote** by default; the repo hosts real personal notes' schema and test fixtures

### Note on `.opencode/`

`.opencode/` in the Android repo is untracked and not gitignored, so it shows up as untracked
noise there. The canonical plan lives at `~/.opencode/plan/notally-web.md`; a copy is kept at
`docs/plan.md` inside the new repo so it travels with the code.

---

## 11. Open Items

None blocking. Recorded for later:

- Whether to implement reminders via server scheduler + Web Push.
- Whether to add audio recording.
- Whether to add PDF export, accepting the RAM cost of a headless browser.
- Consider a scheduled writeback to the same readable JSON format as a belt-and-braces
  on-server backup.
