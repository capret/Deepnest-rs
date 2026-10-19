# Interface languages

Open the **Deepnest** menu in the upper left, then choose **Language** (shown as
**语言 / Language** in Chinese). English and 简体中文 are bundled. The workspace,
settings and illustrated guides, tooltips, select options, status messages, and
app-provided file-dialog captions switch immediately. Existing status messages
also switch; a nesting job does not need to restart.

Project names, part names, filenames, geometry, units, numeric input conventions,
and serialized project values are preserved. For example, an imported part named
`Mounting plate` keeps that name in either language. OS-owned dialog controls use
the OS language; low-level parser and operating-system error details stay verbatim
inside a translated error message.

## Device preference

Language defaults to English and is stored outside `.deepnest` projects, so it
does not mark a project dirty or consume an undo step. It is restored on launch:

| Platform | Preference file |
| --- | --- |
| Windows | `%APPDATA%/Deepnest/preferences.json` |
| macOS | `~/Library/Application Support/Deepnest/preferences.json` |
| Linux | `$XDG_CONFIG_HOME/Deepnest/preferences.json`, or `~/.config/Deepnest/preferences.json` |

Missing, malformed, and unknown-language preferences fall back to English. A
failed save leaves the selected language active for this session and shows an
error. Writes use a temporary file in the destination directory and replacement.

## Adding a language

1. Copy `crates/app/locales/en.json` to a locale file such as `fr.json`. Keep the
   English source keys and translate each value. Every catalog is embedded in
   the executable, so no translation files are needed beside the installed app.
2. Add a variant to `Language` in `crates/app/src/i18n.rs`, with a stable serde
   locale code, its autonym in `Display`, and its catalog in `catalog()`. Add it
   to `Language::ALL`; the menu will list it automatically.
3. Preserve each named placeholder, such as `{count}`, `{path}`, or `{total}`.
   Translators can reorder them. Singular/plural English sentences have separate
   keys. Add the plural forms needed by a new language before shipping it.
4. Run `cargo test --workspace`. Tests require identical catalog keys, nonempty
   translations, and matching placeholders, and check preference replacement,
   fallback, live notices, validation, and preservation of project edits.
5. Check the running app: project menu, Parts/Stock, Setup/Run insights, all five
   settings sections, help illustrations, validation errors, and compact windows.
   Switch back to English and relaunch to check persistence and recovery.

## Writing translatable UI

Use `language.tr("Source message")` for static copy and `trf!(language,
"Sheet {number}", number = index + 1)` for values. Add both catalog entries when
adding or changing copy. Unknown keys fall back to their English source.

Keep deferred notices and validation errors as `i18n::Text`, with source and
arguments separate, until rendering. Never translate user-provided names or
interpolate values before translation. Substitution is a single pass so a file
named `{count}.svg` remains unchanged. Non-UI consumers such as the benchmark
convert these messages to English via `Display`.

Use `language.label(value)` / `labels()` for dropdowns whose values are project
enums. The translated display wrapper leaves underlying values and file formats
unchanged. Numeric inputs continue to accept a decimal point and physical-unit
symbols remain `mm` and `in`.

Iced's automatic text shaping selects advanced shaping and system font fallback
for non-ASCII text. A system font covering the selected language is required.
Tooltip width estimates allow a full em for non-ASCII glyphs. Avoid fixed widths
based on English letter counts; verify wrapping and clipping with each language.
