# Audit

A whole-application audit of tofi-rs and libtofi-rs (2026-11-17) in five
parts: bugs (§2), performance (§3), security and robustness (§4), UI
usability (§5), and code quality, tests and project hygiene (§6). §7 lists
what was left open, and §8 is the checklist to re-run the audit.

Each finding says what was found, how serious it is, and what was done:
**fixed** (with the commit), **accepted** (with the reason), or **OK**
(checked, nothing to do). Every fix is its own commit, with a regression test
where one can be written.

Severity: **high** (wrong output in normal use, a hang the user notices),
**medium** (wrong behaviour in a reachable case, an ignored setting),
**low** (hardening, polish, or needs an unusual setup).

## 1. Method

Every source file was read in full, and behaviour was compared with upstream
C tofi, whose sources are in this repository's history (`git log --all --
src/main.c`). Each finding was then reproduced with the release build in a
private headless sway (wlroots headless backend, pixman renderer, 1280×720
or 1920×1080 at scales 1, 1.5 and 2) with its own runtime, config, cache and
state directories, so nothing touched the desktop session or the real
history. Keys came from a small `zwp_virtual_keyboard_v1` client; sizes and
scales were read from tofi's debug log. That harness is now
`tofi/tests/live.rs` (§6).

Static checks: `cargo clippy` at the three gate levels, `cargo deny check`,
`typos`, `cargo llvm-cov`. Matching was timed with a small benchmark binary
against libtofi-rs.

## 2. Bugs

| Area | Finding | Status |
| ---- | ------- | ------ |
| Logs on stdout | tracing-subscriber writes to stdout by default, so in `run` and `dmenu` mode any warning (shown at the default `warn` level) came out in front of the selection, with colour codes: `tofi --mode dmenu \| …` handed log text to the script. With `anchor = "top"` this happened on every run (next row). Logs now go to stderr, coloured only on a terminal. | Fixed, high (e7b1ec0) |
| False "STRETCH" warning | With an edge anchor (`top`, `bottom`, `left`, `right`) tofi warned that the compositor would stretch the window and ignore its size. Layer-shell only stretches for a size of 0; sway kept 640 px and centred it. | Fixed, low (2ec6cb6) |
| Percentages on HiDPI | `width = "50%"` was taken of the output's physical pixels and used as logical ones, so at scale 1.5 it filled 75 % of the screen. Now taken of the logical size from `xdg-output` (needed for fractional scales), else the mode over the integer scale. | Fixed, medium (7a0adc8) |
| `[window].scale` | Parsed, never used. Now, as upstream: with `false`, pixel values are physical pixels and the text is drawn 1:1. | Fixed, medium (d806e00) |
| Scale of the text | Without the fractional-scale protocol, the text took the first output's scale while the buffer used the selected output's, so they disagreed on outputs with different scales. The surface now records its scale and the renderer uses it. | Fixed, low (d806e00) |
| `[font].hint` | Parsed, never used. `false` now turns hinting off; `true` leaves it to fontconfig as before. | Fixed, low (9385a41) |
| drun keywords | Upstream finds apps by `Keywords` when the name doesn't match ("browser" → Firefox). They were parsed and cached but never matched. | Fixed, medium (5d58a44) |
| Silent config mistakes | A theme with one wrong type (`size = "16"`) silently fell back to the defaults for *every* key; unknown keys, a missing `--config` or theme file, and invalid values (anchor, mode, algorithm, colours, sizes, cursor style) were ignored without a word. All are now logged, with the line for TOML errors. | Fixed, medium (3ef9304) |
| `--completions` while open | Took the single-instance lock first, so it failed while a launcher was open. | Fixed, low (a902a45) |
| Key repeat after focus loss | A key held while focus moved away kept repeating until exit; `wl_keyboard.leave` now stops it. | Fixed, low (0ef8ee1) |
| Paste of non-ASCII text | A pipe read ending inside a UTF-8 character discarded that chunk and ended the paste (`clipboard` feature). | Fixed, low (2c0fb11) |
| Selection underflow | After paging wrapped past the end, moving back could subtract a page larger than the position: a panic in debug builds, an empty list in release. Reachable in horizontal mode, where pages differ in size. | Fixed, low (44e0a01) |
| drun cache vs. language and desktop | Names are localised and `OnlyShowIn`/`NotShowIn` applied at scan time, but the cache only followed directory mtimes, so after changing `LANG` or compositor the old names and apps stayed. The cache now records both. | Fixed, low (eda3e26) |
| Example config | Named three CLI flags that don't exist, described `ascii` wrongly, and put the history in the cache directory. | Fixed (110e2a5) |
| drun history, duplicate names | History is keyed by the displayed name, and two apps with the same name launch the first one. Same as upstream, and the history files stay compatible with it. | Accepted |
| Window size | Read once; later configure events (e.g. a resized output) are acknowledged but the buffer keeps its size. tofi lives for seconds. | Accepted |

## 3. Performance

| Area | Finding | Status |
| ---- | ------- | ------ |
| Fuzzy matching | Tried every way to match the query against each line (up to 100 characters): exponential. Eight repeated letters against one 60-character line took **48 s**; ordinary queries took 0.25–0.6 ms per line, so seconds per keystroke for a few thousand dmenu lines, on the UI thread. It now computes the same best score by dynamic programming: 3–13 µs. A test checks it against the old search on 20,000 generated cases. | Fixed, high (52414d4) |
| Re-filtering on navigation | Every redraw, including arrow keys and Tab, matched and sorted the whole list again. Now only when the query changes. | Fixed, medium (d6af9c4) |
| Large lists | 200,000 dmenu lines: 66 ms from keystroke to filtered list. | OK |
| Start-up | The run and drun lists come from caches checked by directory mtime, as upstream. | OK |

## 4. Security and robustness

| Area | Finding | Status |
| ---- | ------- | ------ |
| Caches in /tmp | With neither `XDG_CACHE_HOME` nor `HOME`, the caches went to `/tmp/tofi-drun` and `/tmp/tofi-compgen`. Another user could plant a drun cache, whose `Exec` lines tofi would run when an app was picked. Now nothing is cached without a cache directory. | Fixed, low (374f60d) |
| Launching apps | `Exec` is split into an argument vector per the Desktop Entry spec and spawned without a shell; field codes become single arguments. No injection. | OK |
| History file | Written with mode 0600; one tofi at a time (lock), so no concurrent writers. | OK |
| Lock file | In `$XDG_RUNTIME_DIR`, `flock` based, released on exit. | OK |
| SHM and Pango | `unsafe` is limited to the mmap'd SHM pool and the Cairo surfaces over it; the pool outlives the surfaces (field order in `WaylandState`). | OK |
| Dependencies | `cargo deny check` passes; all direct dependencies are on their current major versions. | OK |

## 5. UI usability

| Area | Finding | Status |
| ---- | ------- | ------ |
| Config feedback | See §2: mistakes are now reported, and each option's default can be named (`"center"`, `"bar"`, `"normal"`, `"vertical"`). | Fixed (3ef9304) |
| No compositor | Without Wayland tofi panics with "Failed to initialize Wayland" and a backtrace hint, rather than a one-line error. | Open (§7) |

## 6. Code quality, tests and hygiene

| Area | Finding | Status |
| ---- | ------- | ------ |
| Untested Wayland and app layers | `wayland/`, `surface.rs`, `shm/` and `app/` had 0 % coverage. `tofi/tests/live.rs` runs the real binary in a private headless sway and types into it (dmenu selection, arrows, cancel, raw input, drun launching and keywords, history, scales, warnings, config mistakes). CI installs sway and sets `TOFI_LIVE_TESTS=1`; locally the tests skip without sway. | Fixed (322f2aa, 833fa04) |
| Coverage | 55 % → 85 % of lines (`cargo llvm-cov --workspace --all-features` with sway installed). | Fixed |
| Logger module | Subscriber setup was inline in `main`; moved to `logger/` as `docs/ARCHITECTURE.md` asks. | Fixed (e7b1ec0) |
| Raw NUL bytes in a test | Made grep treat `drun/tests.rs` as binary and skip it. | Fixed (193bd94) |
| Cargo warning | `homepage` repeated `repository`; Cargo warned on every build. | Fixed (214803b) |
| Gates | fmt, typos, deny, clippy and tests at the three feature levels, and docs, pass on every commit. | OK |

## 7. Left open

- **Unused library modules.** `libtofi_rs::string_table`, `libtofi_rs::renderer`
  and `libtofi_rs::noop()` are public but unused by the binary. Removing them
  breaks the library's API; decide with the next major version.
- **One wrong type invalidates a whole file.** It is now reported, but upstream
  only skipped the bad line. Per-key leniency would need deserializing each
  key separately.
- **No compositor.** A clean error and exit code instead of a panic (§5).
- **Buffer reuse.** The two SHM buffers alternate without waiting for
  `wl_buffer.release`, so tofi can draw into a buffer the compositor still
  reads (possible tearing). Upstream does the same.
- **Clipboard offers** are replaced without `wl_data_offer.destroy`, and a
  drag-and-drop offer replaces the clipboard one (`clipboard` feature). The
  comment saying dropping the proxy destroys it is wrong for wayland-client
  0.31.
- **Case-insensitive offsets.** `utf8_strcasestr` maps offsets back wrongly for
  characters whose lowercase is longer (e.g. `İ`); only scores are affected.
- **drun details.** `LANG` is preferred over `LC_ALL`/`LC_MESSAGES`, locale
  modifiers (`@euro`) are not stripped, and `Path=` is not used as the working
  directory.
- **Zero-size configure.** A configure with a 0 width or height is not
  acknowledged; tofi always requests a non-zero size, so it is not reached.

## 8. Re-running the audit

```sh
# Gates (docs/ARCHITECTURE.md §8); the live tests run when sway is on PATH
cargo fmt --all -- --check && typos && cargo deny check
for f in --no-default-features "" --all-features; do
  cargo clippy --workspace --all-targets $f -- -D warnings
  TOFI_LIVE_TESTS=1 cargo test --workspace $f
done
cargo doc --workspace --no-deps

# Coverage (system LLVM tools on Arch: LLVM_COV / LLVM_PROFDATA)
LLVM_COV=/usr/bin/llvm-cov LLVM_PROFDATA=/usr/bin/llvm-profdata \
  TOFI_LIVE_TESTS=1 cargo llvm-cov --workspace --all-features --summary-only
```

Then compare with upstream for anything new, and check by hand on a real
session: HiDPI percentages, `scale = false`, drun keywords, and that
`tofi --mode dmenu` prints only the selection.
