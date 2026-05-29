# Changelog

## Unreleased

### Added

- **Dmenu mode restored.** Use `tofi --mode dmenu` or `[base].mode = "dmenu"` in config. Reads newline-separated items from stdin and prints the selection to stdout (same behavior as upstream tofi / dmenu).

### Changed

- **Rendering stack is now pure Rust.** Cairo + Pango were replaced with `tiny-skia` (vector drawing) and `cosmic-text` (layout, shaping via harfrust, font discovery via fontdb, rasterization via swash).
- **System build dependencies updated.** `libcairo2-dev`, `libpango1.0-dev`, and `libharfbuzz-dev` are no longer required; `libfontconfig1-dev` is used on Linux for font resolution.

### Notes

- OpenType **`font_variations`** from theme files are accepted in config but not yet applied by the cosmic-text backend; **`font_features`** (`liga=1`, etc.) work as before.

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
