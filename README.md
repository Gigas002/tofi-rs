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

The `clipboard` and `single-instance-lock` features are on by default. Build without them via `--no-default-features` if not needed.

**Note on binary name:** the installed binary is named `tofi`, the same as the upstream C program. Installing both will cause a PATH conflict — ensure only one is on your `PATH` at a time, or install one under a distinct prefix.

## Migrating from C tofi

See [CHANGELOG.md](CHANGELOG.md) for known differences and migration notes per release.

If something behaves differently from upstream, please [open an issue](https://github.com/Gigas002/tofi/issues) with the compositor name, scale factor, and a minimal config to reproduce.

## Versioning

| Series   | Branch | Goal                                                                                                                                                                                           |
| -------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0.10.x` | `v0`   | Drop-in parity with upstream tofi `0.9.1`. Config format, CLI flags, and XDG paths are compatible. Patches welcome; the branch is in maintenance mode.                                         |
| `1.0.0+` | `rust` | Opinionated fork — `drun` and `run` modes kept, stdin/dmenu removed. Single binary: `tofi --drun` / `tofi --run` (no `tofi-drun`/`tofi-run` symlinks). TOML config/theme, trimmed CLI surface. |
