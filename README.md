# tofi-rs

A Rust fork of [tofi](https://github.com/philj56/tofi).

## Building

**System dependencies** (development headers required at compile time):

| Library        | Debian/Ubuntu package | Arch package   |
| -------------- | --------------------- | -------------- |
| Wayland client | `libwayland-dev`      | `wayland`      |
| Cairo          | `libcairo2-dev`       | `cairo`        |
| Pango          | `libpango1.0-dev`     | `pango`        |
| HarfBuzz       | `libharfbuzz-dev`     | `harfbuzz`     |
| xkbcommon      | `libxkbcommon-dev`    | `libxkbcommon` |
| pkg-config     | `pkg-config`          | `pkgconf`      |

```sh
cargo build --release
# binary is at target/release/tofi
```

The `clipboard` feature is opt-in; add `--features clipboard` to enable paste support.

**Note on binary name:** the installed binary is named `tofi`, the same as the upstream C program. Installing both will cause a PATH conflict — ensure only one is on your `PATH` at a time, or install one under a distinct prefix.

## Migrating from C tofi

See [CHANGELOG.md](CHANGELOG.md) for known differences and migration notes per release.

If something behaves differently from upstream, please [open an issue](https://github.com/Gigas002/tofi/issues) with the compositor name, scale factor, and a minimal config to reproduce.

## Configuration

tofi reads its settings from two separate TOML files:

| File                                | Purpose                                          |
| ----------------------------------- | ------------------------------------------------ |
| `~/.config/tofi/config.toml`        | Behavioral settings (matching, history, output)  |
| `~/.config/tofi/themes/<name>.toml` | Visual settings (colors, fonts, window geometry) |

The theme file is referenced by the `[base].theme` key in the config, or overridden on the command line with `--theme <path>`.

Example files are provided in `examples/`:

| File                  | Description                                               |
| --------------------- | --------------------------------------------------------- |
| `config/minimal.toml` | Empty config — documents all built-in behavioral defaults |
| `config/config.toml`  | Real-world config with every option explained             |
| `config/maximal.toml` | All config options set explicitly                         |
| `theme/minimal.toml`  | Empty theme — documents all built-in visual defaults      |
| `theme/theme.toml`    | Real-world themed example with every option explained     |
| `theme/maximal.toml`  | All theme options set explicitly                          |

Quick-start:

```sh
mkdir -p ~/.config/tofi/themes
cp examples/config/config.toml ~/.config/tofi/config.toml
cp examples/theme/theme.toml ~/.config/tofi/themes/theme.toml
```

## Versioning

| Series   | Branch | Goal                                                                                                                                                                                                     |
| -------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0.10.x` | `v0`   | Drop-in parity with upstream tofi `0.9.1`. Config format, CLI flags, and XDG paths are compatible. Patches welcome; the branch is in maintenance mode.                                                   |
| `1.0.0+` | `rust` | Opinionated fork — `drun` and `run` modes kept, stdin/dmenu removed. Single binary: `tofi --mode drun` / `tofi --mode run` (no `tofi-drun`/`tofi-run` symlinks). TOML config/theme, trimmed CLI surface. |
