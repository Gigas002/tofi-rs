# Changelog

## Unreleased

### Added

- **`libtofi_rs::drun::exec_args` and `libtofi_rs::drun::split_args`.** `exec_args` turns a desktop entry's `Exec` value into an argument vector following the Desktop Entry Specification (string unescaping, double-quote quoting, per-argument field-code expansion); `split_args` is the underlying quote-aware splitter.
- **`libtofi_rs::wayland::OutputInfo::logical_size` and `physical_size`.** The output's size in logical pixels (from `xdg-output` when available) and its rotated mode size.

### Fixed

- **Config and theme mistakes are reported instead of silently ignored.** A file that isn't valid TOML (including a wrong type, like `size = "16"`) used to make tofi quietly use the defaults for *every* setting in it; it is still unusable, but now an error says so and where. Unknown keys (such as a misspelled name), a `--config` or theme file that doesn't exist, and invalid values (an unknown `anchor`, `mode`, `algorithm`, `cursor_style`, `[results].mode`, size or colour) are now logged; invalid values still fall back to their defaults. `"center"`, `"bar"`, `"normal"` and `"vertical"` are now accepted as values too.
- **drun finds apps by their `Keywords` again**, as upstream tofi does: typing "browser" finds Firefox. The keywords were read and cached but never matched. A keyword match ranks below a name match.
- **Fuzzy matching no longer freezes tofi.** It tried every way to match the query against each line, which is exponential: eight repeated letters against one 60-character line took 48 s, and ordinary queries took up to half a millisecond per line, so seconds per keystroke for a few thousand lines. It now finds the same best score in linear time (microseconds per line). Lines over 100 characters keep upstream's quicker first-match scoring.
- **Moving the selection no longer re-filters the list.** Every redraw, including arrow keys and Tab, matched and sorted every entry again; now that only happens when the query changes.
- **`tofi --completions <shell>` works while tofi is open.** It took the single-instance lock first, so it failed with "Another tofi instance is already running".
- **A key held while tofi loses keyboard focus stops repeating.** Its release goes elsewhere, so it used to repeat until tofi exited.
- **Logs go to stderr instead of stdout.** In `run` and `dmenu` mode stdout carries the selection, and a warning (shown at the default `warn` level) used to land in front of it, so `tofi --mode dmenu | …` passed log text to the script. Colour codes are now also only used when stderr is a terminal.
- **Percentage sizes are right on scaled outputs.** `width` and `height` given in `%` were taken of the output's physical pixels but applied as logical ones, so `width = "50%"` filled 75 % of a 1.5×-scaled screen. They are now taken of the logical size, read from `xdg-output` when the compositor offers it (needed for fractional scales), else the physical size over the integer scale.
- **`[window].scale = false` works.** It was read but ignored. As in upstream tofi, pixel values (window size, margins, font size, borders, padding) are then physical pixels and tofi draws 1:1 instead of at the output's scale.
- **`[font].hint = false` works.** It was read but ignored; it now turns glyph hinting off. With `true` (the default) hinting is left to fontconfig as before.
- **The text is drawn at the scale the window was sized for.** Without the fractional-scale protocol, the text used the first output's scale even when tofi opened on another output with a different scale.
- **No more false "compositor will STRETCH" warning** with an edge anchor (`top`, `bottom`, `left`, `right`). A set width or height is kept and the window is centred along that edge, so the warning was wrong, and it appeared on every run.
- **Quoted `Exec` arguments containing spaces are no longer split apart** ([#61](https://github.com/Gigas002/tofi-rs/issues/61)). E.g. `"--profile-directory=Profile 1"` in Chromium PWA entries is now passed as one argument, without the literal quotes. Field-code values (`%c`, `%k`, `%i`) containing spaces also stay a single argument, and the terminal command is split with the same quoting rules.

### Changed

- **`libtofi_rs::entry::EntryConfig` has a new field, `hint_font`.** Struct literals must set it or end in `..EntryConfig::default()` (default `true`).

### Deprecated

- **`libtofi_rs::drun::exec_command`.** It returns a flat string that can't be split correctly; use `exec_args` instead.

## [1.1.0] - 2026-09-04

### Added

- **Dmenu mode restored.** Use `tofi --mode dmenu` or `[base].mode = "dmenu"` in config. Reads newline-separated items from stdin and prints the selection to stdout (same behavior as upstream tofi / dmenu).

### Fixed

- **`--output` / `[base].output` now actually selects the target monitor.** Previously the requested output name was parsed but never passed through to surface creation, so tofi always opened on the compositor's default output regardless of the setting.

## [1.0.0] - 2026-04-19

### Breaking changes

- **Stdin / dmenu-style mode removed.** Only `drun` and `run` modes are supported.
- **Config format changed** from a flat keyfile to two separate TOML files.
- **`tofi-drun` / `tofi-run` symlinks removed.** Use `tofi --mode drun` / `tofi --mode run`.
- **`--drun` / `--run` flags replaced by `--mode`.** `tofi --drun` → `tofi --mode drun`; `tofi --run` → `tofi --mode run`.
- **CLI surface drastically reduced.** All visual and window geometry flags (anchor, width, height, colors, padding, font, …) are removed from the CLI; they are now theme-file-only. The surviving flags are: `--config`, `--theme`, `--mode`, `--output`, `--terminal`, `--algorithm`, `--history`.
- **`--include` removed.** Config file inclusion is no longer supported; use a single `config.toml`.
- **`clipboard` paste support is now opt-in.** Build with `--features clipboard` to enable; it is excluded from the default build.

### Migrating from C tofi (or tofi-rs 0.10.x)

#### File locations

|                | Old                                 | New                                                                      |
| -------------- | ----------------------------------- | ------------------------------------------------------------------------ |
| Config file    | `~/.config/tofi/config`             | `~/.config/tofi/config.toml`                                             |
| Theme          | Inline in config file               | `~/.config/tofi/themes/<name>.toml` (set `[base].theme = "<name>.toml"`) |
| History (run)  | `$XDG_STATE_HOME/tofi-history`      | unchanged                                                                |
| History (drun) | `$XDG_STATE_HOME/tofi-drun-history` | unchanged                                                                |

The config file now contains only behavioral options; all visual options move to the theme file. See [`examples/`](examples/) for annotated reference files.

#### Color format

Colors must now use `#rrggbb` or `#rrggbbaa` hex notation. CSS named colors and bare hex (`FFFFFF`) are no longer accepted.

#### Behavioral options (config.toml)

These options live in `~/.config/tofi/config.toml`. Options that were CLI-only remain CLI-only.

| Old key              | New section  | New key     | Old default   | New default    | Notes                               |
| -------------------- | ------------ | ----------- | ------------- | -------------- | ----------------------------------- |
| `output`             | `[base]`     | `output`    | `""`          | `""`           |                                     |
| `terminal`           | `[base]`     | `terminal`  | `$TERMINAL`   | `""` (env var) |                                     |
| `matching-algorithm` | `[matching]` | `algorithm` | `normal`      | `normal`       | values: `normal`, `prefix`, `fuzzy` |
| `require-match`      | `[matching]` | `require`   | `true`        | `true`         |                                     |
| `ascii-input`        | `[matching]` | `ascii`     | `false`       | `false`        |                                     |
| `history`            | `[history]`  | `history`   | `true`        | `true`         |                                     |
| `history-file`       | `[history]`  | `path`      | _(XDG state)_ | _(XDG state)_  |                                     |

#### Visual options (theme.toml)

These options live in the theme file referenced by `[base].theme`.

**Font**

| Old key           | New section | New key      | Old default | New default |
| ----------------- | ----------- | ------------ | ----------- | ----------- |
| `font`            | `[font]`    | `name`       | `"Sans"`    | `"Sans"`    |
| `font-size`       | `[font]`    | `size`       | `24`        | `24`        |
| `font-features`   | `[font]`    | `features`   | `""`        | `""`        |
| `font-variations` | `[font]`    | `variations` | `""`        | `""`        |
| `hint-font`       | `[font]`    | `hint`       | `true`      | `true`      |

**Window geometry and decoration**

| Old key            | New section | New key            | Old default | New default |
| ------------------ | ----------- | ------------------ | ----------- | ----------- |
| `width`            | `[window]`  | `width`            | `1280`      | `1280`      |
| `height`           | `[window]`  | `height`           | `720`       | `720`       |
| `anchor`           | `[window]`  | `anchor`           | `center`    | `center`    |
| `margin-top`       | `[window]`  | `margin_top`       | `0`         | `0`         |
| `margin-bottom`    | `[window]`  | `margin_bottom`    | `0`         | `0`         |
| `margin-left`      | `[window]`  | `margin_left`      | `0`         | `0`         |
| `margin-right`     | `[window]`  | `margin_right`     | `0`         | `0`         |
| `background-color` | `[window]`  | `background_color` | `#1b1d1eff` | `#1b1d1eff` |
| `border-width`     | `[window]`  | `border_width`     | `12`        | `12`        |
| `border-color`     | `[window]`  | `border_color`     | `#f92672ff` | `#f92672ff` |
| `outline-width`    | `[window]`  | `outline_width`    | `4`         | `4`         |
| `outline-color`    | `[window]`  | `outline_color`    | `#080800ff` | `#080800ff` |
| `corner-radius`    | `[window]`  | `corner_radius`    | `0`         | `0`         |
| `padding-top`      | `[window]`  | `padding_top`      | `8`         | `8`         |
| `padding-bottom`   | `[window]`  | `padding_bottom`   | `8`         | `8`         |
| `padding-left`     | `[window]`  | `padding_left`     | `8`         | `8`         |
| `padding-right`    | `[window]`  | `padding_right`    | `8`         | `8`         |
| `clip-to-padding`  | `[window]`  | `clip_to_padding`  | `true`      | `true`      |
| `scale`            | `[window]`  | `scale`            | `true`      | `true`      |

**Text colors**

| Old key                 | New section | New key                 | Old default             | New default     |
| ----------------------- | ----------- | ----------------------- | ----------------------- | --------------- |
| `text-color`            | `[text]`    | `color`                 | `#ffffffff`             | `#ffffffff`     |
| `prompt-color`          | `[text]`    | `prompt_color`          | _(inherits text-color)_ | none (inherits) |
| `input-color`           | `[text]`    | `input-color`           | _(inherits text-color)_ | none (inherits) |
| `default-result-color`  | `[text]`    | `match_color`           | _(inherits text-color)_ | none (inherits) |
| `selection-color`       | `[text]`    | `selection_color`       | `#f92672ff`             | `#f92672ff`     |
| `selection-match-color` | `[text]`    | `selection_match_color` | `#00000000`             | `#00000000`     |

**Prompt**

| Old key          | New section | New key   | Old default | New default |
| ---------------- | ----------- | --------- | ----------- | ----------- |
| `prompt-text`    | `[prompt]`  | `text`    | `"run: "`   | `"run: "`   |
| `prompt-padding` | `[prompt]`  | `padding` | `0`         | `0`         |

**Results**

| Old key          | New section | New key   | Old default | New default  | Notes                                                      |
| ---------------- | ----------- | --------- | ----------- | ------------ | ---------------------------------------------------------- |
| `num-results`    | `[results]` | `count`   | `0`         | `0`          |                                                            |
| `result-spacing` | `[results]` | `spacing` | `0`         | `0`          |                                                            |
| `horizontal`     | `[results]` | `mode`    | `false`     | `"vertical"` | set `mode = "horizontal"` to replicate `horizontal = true` |

**Cursor (system pointer)**

| Old key       | New section | New key | Old default | New default |
| ------------- | ----------- | ------- | ----------- | ----------- |
| `hide-cursor` | `[cursor]`  | `hide`  | `false`     | `false`     |

**Input and text cursor**

| Old key                     | New section | New key                | Old default | New default |
| --------------------------- | ----------- | ---------------------- | ----------- | ----------- |
| `text-cursor`               | `[input]`   | `cursor`               | `false`     | `false`     |
| `text-cursor-style`         | `[input]`   | `cursor_style`         | `bar`       | `bar`       |
| `text-cursor-color`         | `[input]`   | `cursor_color`         | `#ffffffff` | `#ffffffff` |
| `text-cursor-background`    | `[input]`   | `cursor_background`    | `#000000ff` | `#000000ff` |
| `text-cursor-corner-radius` | `[input]`   | `cursor_corner_radius` | `0`         | `0`         |
| `text-cursor-thickness`     | `[input]`   | `cursor_thickness`     | `2`         | `2`         |
| `hide-input`                | `[input]`   | `hide`                 | `false`     | `false`     |
| `hidden-character`          | `[input]`   | `hidden_character`     | `"*"`       | `"*"`       |

#### Removed CLI flags

These command-line flags no longer exist. Scripts and shell aliases using them must be updated.

| Old flag(s)                               | Replacement / notes                     |
| ----------------------------------------- | --------------------------------------- |
| `--drun`                                  | `--mode drun`                           |
| `--run`                                   | `--mode run`                            |
| `--include <file>`                        | removed; use a single `config.toml`     |
| `--anchor`, `--width`, `--height`, …      | moved to theme file; no CLI override    |
| `--background-color`, `--border-color`, … | moved to theme file; no CLI override    |
| `--font`, `--font-size`, …                | moved to theme file; no CLI override    |
| `--padding-*`, `--margin-*`, …            | moved to theme file; no CLI override    |
| `--num-results`, `--result-spacing`, …    | moved to theme file; no CLI override    |
| `--require-match`, `--ascii-input`        | config file (`[matching]` section) only |
| `--history-file`                          | config file (`[history].path`) only     |
| `--print-index`                           | stdin mode removed                      |

#### Removed options

These options have no equivalent in 1.0.0 and are silently ignored if present.

| Old key                                     | Reason                                                                |
| ------------------------------------------- | --------------------------------------------------------------------- |
| `exclusive-zone`                            | Always `-1` (ignore exclusive zones).                                 |
| `auto-accept-single`                        | Removed; always requires explicit selection.                          |
| `print-index`                               | Stdin mode removed.                                                   |
| `drun-launch`                               | Always launches in drun mode; `drun-launch = false` behavior is gone. |
| `physical-keybindings`                      | Always enabled.                                                       |
| `late-keyboard-init`                        | Removed (performance knob not needed).                                |
| `multi-instance`                            | Always single-instance (lock file always active).                     |
| `min-input-width`                           | Removed.                                                              |
| `include`                                   | Config inclusion removed; use a single file.                          |
| `placeholder-text`                          | Placeholder text not supported.                                       |
| `placeholder-color`                         | Placeholder theming not supported.                                    |
| `placeholder-background`                    | Placeholder theming not supported.                                    |
| `placeholder-background-padding`            | Placeholder theming not supported.                                    |
| `placeholder-background-corner-radius`      | Placeholder theming not supported.                                    |
| `prompt-background`                         | Per-element backgrounds not supported.                                |
| `prompt-background-padding`                 | Per-element backgrounds not supported.                                |
| `prompt-background-corner-radius`           | Per-element backgrounds not supported.                                |
| `input-background`                          | Per-element backgrounds not supported.                                |
| `input-background-padding`                  | Per-element backgrounds not supported.                                |
| `input-background-corner-radius`            | Per-element backgrounds not supported.                                |
| `default-result-background`                 | Per-element backgrounds not supported.                                |
| `default-result-background-padding`         | Per-element backgrounds not supported.                                |
| `default-result-background-corner-radius`   | Per-element backgrounds not supported.                                |
| `alternate-result-color`                    | Alternate rows use `match_color`; no separate color.                  |
| `alternate-result-background`               | Per-element backgrounds not supported.                                |
| `alternate-result-background-padding`       | Per-element backgrounds not supported.                                |
| `alternate-result-background-corner-radius` | Per-element backgrounds not supported.                                |
| `selection-background`                      | Per-element backgrounds not supported.                                |
| `selection-background-padding`              | Per-element backgrounds not supported.                                |
| `selection-background-corner-radius`        | Per-element backgrounds not supported.                                |

## [0.10.0] - 2026-04-17

Targets behavioral parity with upstream [tofi `0.9.1`](https://github.com/philj56/tofi).

### Known differences from upstream

- Single binary only: use `tofi --drun` / `tofi --run` instead of `tofi-drun` / `tofi-run` symlinks.
- `tofi-run` uses direct PATH scanning rather than `compgen`. Results are equivalent.
- Pixel-identical rendering is not guaranteed across hardware/drivers (documented non-goal).
