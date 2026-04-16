# tofi-rs

A rust-rewritten fork of [tofi](https://github.com/philj56/tofi).

The theoretically-compatible to original `tofi` tag is `0.10.0`. If you are interested in supporting original use-case scenarios and overhaul the code - propose changes to `v0` branch, but better just fork it since it'll be on life support without much interest from me.

Breaking changes are coming starting from `1.0.0` tag (`rust` branch), with a lot of features removed and stuff changed since I don't have interest supporting patterns I don't use. It's basically a `tofi-drun` only, with config/themes migrated to `toml` and some minor stuff (like CLI params/commands and some config opts) I don't use removed. `libtofi` deprecations will probably be minimal.

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
