# Cutover inventory — C → Rust

This document fulfils **Phase 9 Step 9.1** of
[`docs/RUST_MIGRATION_PLAN.md`](RUST_MIGRATION_PLAN.md).
It records what will be removed in Phase 9, what will be kept, and the
criteria that must hold before the cutover happens.

---

## 1. Cutover criteria (§5.4)

All of the following must pass before any legacy tree item is deleted:

- [ ] `cargo build --release --workspace` succeeds with default features.
- [ ] `cargo test --workspace` passes (with growing coverage per §5.3).
- [ ] `target/release/tofi` runs on Sway (or another wlroots compositor) in
      **stdin**, **run**, and **drun** modes.
- [ ] `tofi-run` and `tofi-drun` symlinks are documented for packagers
      (see `docs/RUST_MIGRATION_PLAN.md` §8 Step 8.1).
- [ ] Config and theme files from `examples/config/` and `examples/themes/`
      load without errors (Step 9.4 / 9.5).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean for both
      `--no-default-features` and `--all-features`.
- [ ] Meson / C-only CI was already removed in Step 0.2. Verify no leftover
      references exist in active docs or scripts.

**Only after all boxes above are ticked** should the steps below proceed.
Optionally tag the last C-only commit:
```sh
git tag -a legacy/c-before-rust -m "Last commit before C source removal (Phase 9)"
```

---

## 2. Items to REMOVE

### 2.1 C build system

| Path | Notes |
|------|-------|
| `meson.build` | Root Meson build file |
| `meson_options.txt` | Meson option declarations |
| `doc/meson.build` | Man-page build rules (if present) |
| `test/meson.build` | C test build rules |

### 2.2 C source tree

| Path | Notes |
|------|-------|
| `src/*.c` / `src/*.h` | All C translation units and headers |
| `src/entry_backend/` | Cairo/Pango/HarfBuzz C backends |

### 2.3 C test suite

| Path | Notes |
|------|-------|
| `test/config.c` | C config tests (not ported — new Rust suite replaces) |
| `test/tap.c` / `test/tap.h` | TAP harness used by C tests |
| `test/utf8.c` | UTF-8 C tests (not ported) |

### 2.4 Vendored protocol XML (if unused by Rust)

| Path | Notes |
|------|-------|
| `protocols/wlr-layer-shell-unstable-v1.xml` | Used by `wayland-protocols-wlr` crate; verify Rust build does **not** reference in-tree XML before removing |
| `protocols/fractional-scale-v1.xml` | Same check — `wayland-protocols` crate ships this |

> **Action before removal:** run `grep -r 'protocols/' Cargo.toml libtofi/ tofi/` to confirm no `build.rs` or `include!` uses the in-tree XML.

### 2.5 Old themes tree

| Path | Notes |
|------|-------|
| `themes/dark-paper` | Replaced by `examples/themes/` |
| `themes/dmenu` | Replaced |
| `themes/dos` | Replaced |
| `themes/fullscreen` | Replaced |
| `themes/soy-milk` | Replaced |

Remove **after** canonical replacements in `examples/themes/` exist (Step 9.4).

### 2.6 Old scattered example configs

| Path | Notes |
|------|-------|
| `doc/config` | Upstream keyfile example; replaced by `examples/config/` tree |

Remove after `examples/config/` is populated and tested (Step 9.4 / 9.5).

### 2.7 Build / doc helpers referencing the C toolchain

| Path | Notes |
|------|-------|
| `doc/scd2gfm.sh` | Shell script for `scdoc` → GFM conversion (man-page toolchain) |
| `doc/tofi.1.scd` / `doc/tofi.5.scd` | `scdoc` man-page sources — keep `.md` variants; remove `.scd` if no longer generated |

---

## 3. Items to KEEP

| Path | Reason |
|------|--------|
| `libtofi/` | Rust library crate — the product |
| `tofi/` | Rust CLI crate — the product |
| `Cargo.toml` / `Cargo.lock` | Workspace manifest and lockfile |
| `LICENSE` | Must keep Philip Jones copyright notice (§1.1) |
| `README.md` | Update for Rust workflow after removal |
| `CHANGELOG.md` | Project history |
| `docs/` | Migration plan, this file, and any other living docs |
| `examples/config/` | Canonical app config fixtures (Step 9.4) |
| `examples/themes/` | Canonical theme fixtures (Step 9.4) |
| `.github/workflows/` | Rust CI (C/Meson CI already removed in Step 0.2) |
| `.github/dependabot.yml` | Dependency update automation |
| `doc/tofi.1.md` / `doc/tofi.5.md` | Markdown man-page references (used in-code links) |
| `deny.toml` | `cargo deny` license/advisory policy |

---

## 4. Post-removal verification

After deleting each group above, verify:

```sh
# No dangling references to removed C paths in active docs or scripts
git grep -n '\.c"' -- '*.md' '*.toml' '*.yml' '*.sh' | grep -v CUTOVER | grep -v RUST_MIGRATION_PLAN

# No Meson references in active files
git grep -n 'meson\|ninja' -- '*.md' '*.toml' '*.yml' '*.sh' | grep -v CUTOVER | grep -v RUST_MIGRATION_PLAN

# Protocol XML not referenced by Rust build
grep -r 'protocols/' Cargo.toml libtofi/ tofi/

# Build and tests still green
cargo build --release --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
```
