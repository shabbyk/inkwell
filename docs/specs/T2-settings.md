# T2 — Settings store

**Difficulty: easy. Safe for a smaller local model.**

## Goal

Create `server/src/settings.rs` — a typed get/set layer over the `Setting` table, matching
Notally's preference keys and defaults exactly.

## Create

File: `server/src/settings.rs`. Add to `src/main.rs`:

```rust
mod settings;
```

## The table

```sql
CREATE TABLE IF NOT EXISTS Setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

Already created by `db::init`.

## Keys, defaults, and valid values

These mirror Notally's `preferences/Preferences.kt` and `preferences/ListInfo.kt`. Use
these exact lowercase keys and defaults:

| Key | Type | Default | Valid values |
|---|---|---|---|
| `view` | string | `list` | `list`, `grid` |
| `theme` | string | `followSystem` | `dark`, `light`, `followSystem` |
| `dateFormat` | string | `relative` | `none`, `relative`, `absolute` |
| `textSize` | string | `medium` | `small`, `medium`, `large` |
| `maxItems` | int | `10` | 1–100 |
| `maxLines` | int | `8` | 1–50 |
| `maxTitle` | int | `100` | 10–1000 |
| `autoBackupPath` | string | `""` | any path |

`maxItems`, `maxLines`, and `maxTitle` control how much of a note the list view shows
before truncating. Defaults are chosen for a web layout; Notally's exact numeric defaults
were tuned for a phone screen and do not transfer.

## Signatures to implement

```rust
/// All settings as strings, with defaults filled in for anything missing.
pub fn all(conn: &Connection) -> Result<BTreeMap<String, String>>

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>>
pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()>

pub fn get_view(conn: &Connection) -> Result<View>
pub fn set_view(conn: &Connection, v: View) -> Result<()>

pub fn get_theme(conn: &Connection) -> Result<Theme>
pub fn set_theme(conn: &Connection, t: Theme) -> Result<()>

pub fn get_date_format(conn: &Connection) -> Result<DateFormat>
pub fn set_date_format(conn: &Connection, d: DateFormat) -> Result<()>

pub fn get_text_size(conn: &Connection) -> Result<TextSize>
pub fn set_text_size(conn: &Connection, t: TextSize) -> Result<()>

pub fn get_max_items(conn: &Connection) -> Result<i64>
pub fn set_max_items(conn: &Connection, n: i64) -> Result<())
// ...and likewise for max_lines and max_title

pub fn all_json(conn: &Connection) -> Result<serde_json::Value>
```

## Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum View { List, Grid }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme { Dark, Light, FollowSystem }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DateFormat { None, Relative, Absolute }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextSize { Small, Medium, Large }
```

`rename_all = "camelCase"` gives `followSystem` for `FollowSystem`, which is the string
Notally stores. Check your output: `FollowSystem` must serialise to `"followSystem"`, not
`"follow_system"`. If serde produces the wrong thing, add explicit
`#[serde(rename = "followSystem")]` to that variant.

`all_json` should return a JSON object with camelCase keys suitable for sending to the
browser — the same shape as the settings block in a `notes.json` export.

## Validation

`set_max_items` and friends must **reject** out-of-range values rather than storing them.
Return `Err(anyhow::anyhow!(...))` with a message naming the key and the valid range. The
integer getters clamp defensively on read as well, so a bad value written by an older
version or a hand-edited database cannot produce a broken UI.

Unknown string values on read (e.g. `theme = "neon"`) fall back to the default rather than
erroring. A settings typo should never make the app unusable.

`set` must use `INSERT OR REPLACE INTO Setting (key, value) VALUES (?1, ?2)`.

## Tests

- `defaults_are_returned_when_table_is_empty` — check all eight
- `set_then_get_round_trips`
- `unknown_value_falls_back_to_default` for each enum
- `max_items_rejects_out_of_range` for 0, 101, and -1
- `max_items_clamps_on_read` — write `999` directly with SQL, assert the getter clamps
- `all_json_has_camel_case_keys` — assert the key is `"followSystem"` for theme, and that
  `"dateFormat"` and `"textSize"` are camelCase
- `serialised_theme_is_follow_system` — the exact-string test

## Commit

```
feat(server): typed settings store with Notally-compatible keys
```
