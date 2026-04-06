# Tofi → Rust migration plan

This document is both a **human roadmap** and an **agent playbook**: each step is small enough to implement in one focused session, ends in a **verified** state (**build** + §5.2 **fmt/clippy**; **tests** per §5.2 / §5.3 once they exist), and defines **how to verify** it. **Reading this plan, implementing accordingly, and updating it** (checkboxes, revision history when policy changes) is the primary execution discipline. It assumes parity with the current C implementation ([`meson.build`](../meson.build), [`src/`](../src/)). **Porting the existing C tests is not required**; a **new Rust test suite** for **both `libtofi-rs` and `tofi-rs`** is (see §5.3).

---

## 1. Goals and constraints

### 1.1 Goals

- **Functional parity** with the existing `tofi` binary: Wayland-only launcher (wlroots-style compositors), dmenu-like stdin mode, `tofi-run`, `tofi-drun`, config files, theming, matching, history, clipboard paste, performance-oriented defaults.
- **Run mode command list:** the C helper in [`src/compgen.c`](../src/compgen.c) is **not** for shell tab-completion scripts; it caches the executable list for **`tofi-run`** (see §3.5). Implement that logic **inside `libtofi`**—**no** separate `tofi-compgen` binary. **Shell completions** for the `tofi` CLI belong to **`clap`** + **`clap_complete`** (or similar) and are a **post-1.0** item (see §9).
- **Two-crate workspace**: Cargo packages **`libtofi-rs`** (library) and **`tofi-rs`** (CLI). This document uses the shorthand **`libtofi::`** for module paths; in Rust these resolve as **`libtofi_rs::`** (hyphens become underscores in the crate identifier).
- **Compile-time customization** via Cargo `[features]`: **`libtofi-rs`** implements **engine + renderer** (`renderer-cairo`); **`tofi-rs` mirrors** the same names—see §4. **UI definition** stays in the CLI (§1.4).
- **A new automated test suite** in Rust (`cargo test`) for **both** **`libtofi-rs`** and **`tofi-rs`**: library tests per §5.3; **CLI tests** for argument parsing, exit codes, and other testable surface (integration tests under `tofi/tests/`, or `#[cfg(test)]` modules next to split-out CLI code—see §5.3). Coverage should grow with each phase; pure logic should be **well tested**; graphics/Wayland may rely more on manual smoke tests unless you add harnesses later.
- **License:** **Target** permissive licensing (**MIT** for your code, aligned with upstream [`LICENSE`](../LICENSE)). Do not treat a random relicensing as a migration deliverable—**but** dependency choices can **pull in** strong copyleft obligations (see §1.5).
- **Copyright / authors:** Under the MIT license you **must keep** the existing copyright and permission notice for upstream-authored material you still distribute (Philip Jones per current `LICENSE`, plus any other files that carry notices). You **may and should add** yourself as an additional copyright holder and in `authors` / `CONTRIBUTORS` for **your** new or substantially rewritten work—you do not “swap out” the original author for code that remains derived from or under their copyright. For **entirely new** Rust files with no upstream text, your copyright line alone is typical. When in doubt, **add** rather than **replace** notices.

### 1.2 Non-goals

- **Porting or line-for-line translation of the C test suite** ([`test/`](../test/)). Those files are reference only; **new** Rust tests replace them in spirit, not by migration.
- **X11 support** (the C tree is Wayland-only).
- **Pixel-perfect binary compatibility** with the C binary (acceptable: minor timing/layout differences if documented).

### 1.3 Reference map (C → conceptual Rust ownership)

| C area                                                                                                                                           | Role                                                       | Suggested Rust home                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| [`src/main.c`](../src/main.c)                                                                                                                    | Event loop, registry, keyboard/pointer, layer shell, paste | `libtofi::wayland` + **`tofi`** `main` wiring (see §1.4)                                                                     |
| [`src/surface.c`](../src/surface.c), [`src/shm.c`](../src/shm.c)                                                                                 | SHM buffers, double buffering                              | `libtofi::shm` / buffer contract for the UI                                                                                  |
| [`src/entry.c`](../src/entry.c), [`src/entry_backend/*`](../src/entry_backend/)                                                                  | Cairo + Pango + HarfBuzz layout/draw                       | **`libtofi::entry`** / **`libtofi::render`** (implementation); **`tofi::ui`** (or similar) for **UI definition** only (§1.4) |
| [`src/config.c`](../src/config.c)                                                                                                                | Keyfile + CLI application                                  | **Parse + apply** in **`tofi::config`**; **runtime config types** shared with engine in `libtofi::config` (see §1.4)         |
| [`src/matching.c`](../src/matching.c)                                                                                                            | Filter algorithms                                          | `libtofi::matching`                                                                                                          |
| [`src/drun.c`](../src/drun.c)                                                                                                                    | Desktop files, cache, launch                               | `libtofi::drun` (feature)                                                                                                    |
| [`src/compgen.c`](../src/compgen.c)                                                                                                              | Cached PATH / `compgen -c` list for **run mode**           | `libtofi::run_commands` or `compgen` **library module** only (no extra binary)                                               |
| [`src/history.c`](../src/history.c)                                                                                                              | History file                                               | `libtofi::history`                                                                                                           |
| [`src/input.c`](../src/input.c)                                                                                                                  | Key handling, repeat, bindings                             | `libtofi::input`                                                                                                             |
| [`src/clipboard.c`](../src/clipboard.c) + main paste path                                                                                        | `wl_data_device` paste                                     | See §3.3                                                                                                                     |
| [`src/lock.c`](../src/lock.c)                                                                                                                    | Single-instance lock                                       | `libtofi::lock` (feature)                                                                                                    |
| [`src/unicode.c`](../src/unicode.c), [`src/color.c`](../src/color.c), [`src/string_vec.c`](../src/string_vec.c), [`src/scale.c`](../src/scale.c) | Pure helpers                                               | `libtofi::util` (and **`color`** as numeric/types used by shared config model)                                               |

### 1.4 Crate responsibilities — config, UI **definition**, vs renderer **implementation**

| Concern                       | **`tofi-rs` (CLI / application)**                                                                                                                                                                                                            | **`libtofi-rs` (library / engine)**                                                                                                                                                                 |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Config**                    | **Parsing** the keyfile (same format as C), **`include`**, paths, **CLI → config** (`clap` / `apply_key` parity). Anything that reads strings from disk or argv.                                                                             | **`TofiConfig`** (or equivalent) as **plain data**; **no** file I/O or `getopt` in the library. Small **value types** (`Color`, limits) next to that model.                                         |
| **UI (definition)**           | **What** the launcher presents and how it maps from loaded config: modules like **`tofi::ui`** — composition, orchestration, calling into the library. **No** requirement to put Cairo calls here; keep “product shape” in the binary crate. | N/A as a separate crate — the library exposes APIs the CLI drives.                                                                                                                                  |
| **Renderer (implementation)** | Thin glue: pass buffers/config handles into **`libtofi`**.                                                                                                                                                                                   | **Cairo / Pango / HarfBuzz** pipeline (former `entry.c`, `entry_backend/*`), painting into SHM buffers, layout metrics — **`libtofi::entry`**, **`libtofi::render`**, feature **`renderer-cairo`**. |

**Summary:** **UI is _defined_ in the CLI** (structure, wiring). **Renderer bits** (the actual drawing stack) **live in `libtofi-rs`** — testable, optional via **`renderer-cairo`**, same as tying Cairo to the engine.

**Config parser** stays in **`tofi-rs`**; **wrong** would be implementing **only** the full parser inside **`libtofi`** without the CLI owning I/O. **Phase 2 / 5** follow this section.

### 1.5 Dependency licenses and strong copyleft

Rust crates link into your binaries **statically** by default. If you add a dependency whose license is **strong copyleft** (e.g. **GPL-3.0**, **AGPL-3.0**), the **combined** work you distribute may need to comply with that license for the relevant parts—so a **partial** move toward copyleft (e.g. GPL for the shipped `tofi` binary while some files remain MIT-noticed) can become **necessary** **depending on which crates** you choose, even if your **own** source stays MIT. Conversely, staying **fully MIT-compatible** for end users is easier if you **prefer** **MIT / Apache-2.0 / BSD / ISC** (etc.) crates and audit **`Cargo.lock`**.

**Practical:** Use **`cargo deny`** (§9) with an explicit **license allow-list**; when evaluating a crate, read its `LICENSE` / `SPDX` metadata. **Not legal advice**—confirm with counsel if your distro model or linking story is non-obvious.

---

## 2. Repository layout (target)

```text
tofi/
  Cargo.toml                 # [workspace] — members, shared metadata, resolver
  libtofi/
    Cargo.toml               # package `name = "libtofi-rs"`; **[features]** for library capabilities (§4.1)
    src/lib.rs
    src/<module>/mod.rs
    src/<module>/tests.rs    # per-module unit tests (see §5.3)
    tests/                   # optional: integration tests for the library crate
  tofi/
    Cargo.toml               # package `name = "tofi-rs"`; **[features]** mirror §4.1 (each forwards to `libtofi-rs/…`); binary name `tofi` via [[bin]] name = "tofi"
    src/main.rs
    src/…                    # optional: extra modules + per-module tests.rs as CLI grows
    tests/                   # **required:** CLI integration tests (`cargo test -p tofi-rs`; see §5.3)
  examples/
    config/                  # canonical **app config** fixture(s); **do not** mix with themes (Phase 9.4)
    themes/                  # canonical **theme** fixture(s); separate folder + separate test patterns (Phase 9.5)
```

**Workspace vs crate manifests:** Keep a **workspace-level** `Cargo.toml` at the repo root (`[workspace]`, `[workspace.package]` where useful) **and** a **crate-level** `Cargo.toml` inside each of `libtofi/` and `tofi/`. Do not fold everything into a single manifest.

**Naming (your convention):**

| Role    | Cargo package name (`name =` in `Cargo.toml`) | Typical `[[bin]]` / `lib` name                            |
| ------- | --------------------------------------------- | --------------------------------------------------------- |
| Library | `libtofi-rs`                                  | Rust crate id `libtofi_rs` (`use libtofi_rs::…`)          |
| CLI     | `tofi-rs`                                     | Binary installed as **`tofi`** (match C: `/usr/bin/tofi`) |

**Naming note:** On crates.io, `tofi` may be taken; the **package** can be `tofi-rs` while the **installed binary** remains `tofi`. Use `authors` / `repository` fields pointing to this repo.

### 2.1 Toolchain and dependency policy

- **Rust edition:** **`2024`** for the workspace (set via `[workspace.package]` / each crate’s `Cargo.toml` so all members agree).
- **`rust-version`:** **Do not pin** in `Cargo.toml`. Prefer tracking **latest stable** Rust in practice (document in README; CI should use a current stable image). Omitting `rust-version` avoids artificial floor/ceiling drift; if you ever need a documented minimum, put it in prose only.
- **Dependency health:** Before adding a crate, confirm it is **actively maintained**: last meaningful release or maintenance activity within **roughly one year**, no **deprecated** / **obsolete** status on crates.io or the upstream repo. If in doubt, prefer a smaller maintained alternative or a thin in-tree wrapper. Re-check when bumping lockfiles.
- **Version specifiers in `Cargo.toml`:** Prefer **two components** (`x.y`, e.g. `1.2`) rather than **three** (`x.y.z`, e.g. `1.2.3`) in `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`, unless you must pin a specific patch for a known bug or security fix. Shorter requirements reduce noisy manifest diffs, align with caret-style upgrade ranges, and keep **Dependabot** PRs easier to review. **Exact** resolved versions still belong in **`Cargo.lock`** (committed for applications/workspace binaries).
- **License of dependencies:** Check **SPDX** / crate `LICENSE`—strong copyleft can affect how you ship binaries (§1.5).

**Cargo `-p` flag:** Examples below use `libtofi-rs` and `tofi-rs` as the workspace package names. If you choose different `name =` values, substitute accordingly.

### 2.2 CI and quality gates (wayshot-style — adopt in Phase 0)

**Goal:** Run **Rust** CI on **every push and pull request** as soon as the workspace exists, instead of waiting until late phases or post-1.0. Use **[waycrate/wayshot](https://github.com/waycrate/wayshot)** as the **reference layout** for workflows and tooling: multiple small workflows under [`.github/workflows/`](https://github.com/waycrate/wayshot/tree/main/.github/workflows), consistent naming, and the same tools (so you can diff or cherry-pick updates).

| Wayshot workflow (reference)                                                                             | Role                                                                                                                                                                                                                                                                                                       |
| -------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`build.yml`](https://github.com/waycrate/wayshot/blob/main/.github/workflows/build.yml)                 | `cargo build --workspace --release` with a **matrix**: default features, `--all-features`, `--no-default-features` (fail-fast off). Uses **`dtolnay/rust-toolchain@stable`**, **`Swatinem/rust-cache@v2`**, system packages for native deps.                                                               |
| [`fmt-clippy.yml`](https://github.com/waycrate/wayshot/blob/main/.github/workflows/fmt-clippy.yml)       | `cargo fmt -- --check`; **`cargo clippy`** with `-D warnings`, matrix on **`--all-features`** vs **`--no-default-features`**.                                                                                                                                                                              |
| [`test-coverage.yml`](https://github.com/waycrate/wayshot/blob/main/.github/workflows/test-coverage.yml) | Tests + coverage reporting (optional in the first CI PR if you prefer to add once `cargo test` is meaningful).                                                                                                                                                                                             |
| [`typos.yml`](https://github.com/waycrate/wayshot/blob/main/.github/workflows/typos.yml)                 | **`typos`** spell check for docs and strings.                                                                                                                                                                                                                                                              |
| [`deploy.yml`](https://github.com/waycrate/wayshot/blob/main/.github/workflows/deploy.yml)               | Release / crates.io / artifact publishing — **commit a `deploy.yml` alongside other workflows in Step 0.2** (same repo layout as wayshot) but **do not** turn on automatic publishes yet; gate with **`workflow_dispatch`**, a disabled `if:`, or branch/tag filters until **§9 Deploy** (late migration). |

**Dependabot (separate from wayshot’s workflow YAML):** Add **[`.github/dependabot.yml`](https://docs.github.com/en/code-security/dependabot/dependabot-version-updates/configuration-options-for-the-dependabot.yml-file)** in **Step 0.2** with at least **`package-ecosystem: "cargo"`** (directory **`/`** for the workspace root) and **`github-actions`** (bumps Action versions in `.github/workflows/`). That gives **automated dependency update PRs** and **security/version scanning** via GitHub’s Dependabot integration—distinct from a later **`cargo deny`** policy (§9). Keep **`Cargo.toml`** version requirements in the **`x.y`** form (§2.1) so bumps stay small and reviews stay light.

**CI workflow pins (GitHub Actions `uses:`):** Prefer **as current as practical**—reference **major-only** tags where maintainers provide them (e.g. `actions/checkout@v4`, `actions/cache@v4`, `Swatinem/rust-cache@v2`), i.e. **one** version segment **`x`**, **not** `x.y` or `x.y.z` SHAs or patch-level tags unless a security advisory forces a temporary pin. **`dtolnay/rust-toolchain@stable`** (or **`nightly`**) is appropriate for the Rust channel. This is **opposite** to **`Cargo.toml`** (§2.1): crates use **`x.y`**; CI Actions use **`x`** so you ride **latest** minor/patch releases within that major line and Dependabot **`github-actions`** PRs stay **major-bump** focused.

**Intentionally not part of Phase 0 Rust CI:** **`cargo deny`** / **`deny.yml`** (dedicated **`deny.toml` + CI** is its **own** later step—§9 **`cargo-deny`**); **`docs.yml`** / `cargo doc` as a required CI job (this project does **not** target man pages or published rustdoc in CI—skip that job).

**Tofi-specific adaptation:** Install **system** libraries needed for **Wayland**, **Cairo**, **Pango**, **HarfBuzz**, **xkbcommon** (and friends) before `cargo build` / `cargo test` — mirror the dependency set you will document for packagers. Wayshot uses an **`archlinux:latest`** container + **`pacman`**; you may use **`ubuntu-latest`** + **`apt`** instead if maintenance is simpler — either is fine; **document the choice in workflow comments**.

**Legacy Meson CI removal (same milestone as Rust CI):** When **Phase 0 Step 0.2** adds the Rust workflows, **remove** the old C/Meson-only workflow(s) in the **same** change set (this repo previously used a Meson/`ninja` **build-test** workflow). Do **not** keep Meson and Rust CI running in parallel—switch CI to Cargo immediately. The **C source tree** stays in-repo as reference until **Phase 9** (sources vs CI are different: **CI = Rust-only from Step 0.2 onward**).

**Scheduling:** Implement as **Phase 0 Step 0.2** (immediately after the empty workspace compiles — **Step 0.1**). Do not defer “proper” CI to §9; §9 lists **deploy enablement**, **`cargo deny`**, and other **late** polish—not the baseline fmt/clippy/build/test/Dependabot stack.

---

## 3. Dependencies strategy (do not reimplement everything)

### 3.1 Wayland protocols

**Do not vendor XML** in-tree unless a distro absolutely requires it. Prefer:

- [`wayland-client`](https://crates.io/crates/wayland-client) + [`wayland-protocols`](https://crates.io/crates/wayland-protocols) for stable protocols (e.g. `xdg-shell`, `viewporter`, `wp-fractional-scale`).
- For **`wlr-layer-shell-unstable-v1`**, use **[`wayland-protocols-wlr`](https://crates.io/crates/wayland-protocols-wlr)** (generated bindings for wlroots protocols, including layer shell). Align its **`wayland-client` / `wayland-sys` versions** with the rest of the stack so Cargo resolves a single protocol family.

**Fallback (only if needed):** minimal `build.rs` + `wayland-scanner` against upstream XML—still no hand-maintained protocol files beyond upstream sources.

**Agent instruction:** Prefer **`wayland-protocols-wlr`** for layer shell; verify compatible versions with `wayland-client` / `wayland-protocols` before locking `Cargo.lock`. Avoid duplicate `wayland-client` major versions across dependencies.

### 3.2 Text rendering (Cairo / Pango / HarfBuzz)

Parity path: **`cairo-rs`**, **`pango`**, **`pangocairo`**, **`harfbuzz_rs`** (or equivalent maintained bindings), matching [`src/entry_backend/`](../src/entry_backend/).

**Crate:** Depend on these in **`libtofi-rs`**, behind **`renderer-cairo`** in **`libtofi/Cargo.toml`** (§4.1). The **CLI** does not embed Cairo for the main pipeline—it **defines UI** and calls into **`libtofi`** to draw (§1.4).

**Feature gate:** `renderer-cairo` on **`libtofi-rs`** (default ON); **`tofi-rs`** mirrors it with **`renderer-cairo = ["libtofi-rs/renderer-cairo"]`**. A future alternative backend would be a separate **`libtofi-rs`** feature.

### 3.3 Clipboard

The C code uses **Wayland `wl_data_device` / `wl_data_offer`** for paste ([`src/clipboard.h`](../src/clipboard.h), listeners in [`src/main.c`](../src/main.c)).

Options (document tradeoffs for packagers):

| Approach                                     | Pros                             | Cons                                                       |
| -------------------------------------------- | -------------------------------- | ---------------------------------------------------------- |
| **Port listeners** to Rust (same as C)       | True 1:1 behavior, no extra deps | More code                                                  |
| **clipboard / arboard / wl-clipboard–style** | Faster to integrate              | May differ slightly on edge compositors; verify under Sway |

**Recommendation:** Implement **Wayland paste in-library** for default feature `clipboard-wayland` to match C; optionally expose `clipboard-arboard` behind a feature for experimentation **only if** parity tests (manual) pass.

### 3.4 Desktop entries / `drun` (GLib in C)

C uses **glib/gio** ([`meson.build`](../meson.build)). In Rust, prefer:

- [`freedesktop-desktop-entry`](https://crates.io/crates/freedesktop-desktop-entry) and/or [`freedesktop-icons`](https://crates.io/crates/freedesktop-icons) for parsing and paths, **or**
- A thin port of the same discovery logic as [`src/drun.c`](../src/drun.c) using pure Rust + `walkdir`.

**Avoid** pulling full GTK; not needed.

### 3.5 What upstream calls `compgen` (not shell tab completions)

Upstream **reuses the name “compgen”** for a **performance cache**, not for generating bash/zsh/fish completion _scripts_ for the `tofi` binary.

- **What it does:** [`src/compgen.c`](../src/compgen.c) runs **`bash`’s built-in `compgen -c`** (via a subprocess) to obtain the list of command names on **`$PATH`**, then **caches** that list under `XDG_CACHE_HOME` (see `tofi-compgen` cache basename in C). That list feeds **`tofi-run`** mode so the launcher does not rescan PATH from scratch every time.
- **What it is not:** It is **not** the same as **shell completion** for typing `tofi <TAB>` in a terminal. Those belong to **`clap`** + **`clap_complete`** (or similar), generated from the CLI definition—see §9 (post-1.0).

**Rust plan:** Implement this as a **normal library module** (e.g. `libtofi::run_commands` or keep the internal name `compgen`) used only when building the command list for run mode. **`std::process::Command`**, cache files, tests in `tests.rs`—**no** second binary. **Do not** confuse with `clap_complete`.

### 3.6 xkbcommon

Use [`xkbcommon`](https://crates.io/crates/xkbcommon) crate (or `xkbcommon-rs`) matching C usage in [`src/main.c`](../src/main.c).

### 3.7 Logging

Use **`tracing`** + **`tracing-subscriber`** (optional env filter). Map C `log_debug` / `log_error` to `tracing::debug!` / `error!`. Gate verbose logs behind `tracing` or `debug-logs` feature if desired.

---

## 4. Cargo features (compile-time split)

**`libtofi-rs`** gates **engine** and **`renderer-cairo`** (Cairo/Pango drawing — §1.4). **`tofi-rs`** **mirrors every** `libtofi-rs` feature name by forwarding to **`libtofi-rs/...`** so packagers tune the binary without editing the library crate.

Packagers: `cargo build -p tofi-rs --no-default-features --features "…"` — e.g. **no `clipboard-wayland`**, **no `drun`**, **no `renderer-cairo`** (headless/engine-only experiments).

**No man pages** as a project target.

**§4.1** and **§4.2** stay in lockstep (same feature names, including **`renderer-cairo`**).

### 4.1 `libtofi-rs` (library) — engine + renderer implementation

Document in **`libtofi/Cargo.toml`**.

| Feature                | Purpose                                                   | Default                                                                                                                                                                     |
| ---------------------- | --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- |
| `default`              | Full launcher                                             | Enables: `wayland`, `renderer-cairo`, `drun`, `clipboard-wayland`, `history`, `single-instance-lock`, `run-command-cache` (or fold `run-command-cache` into `default` only) | yes |
| `wayland`              | Core Wayland client, SHM, surfaces                        | implied by `default`                                                                                                                                                        | yes |
| `renderer-cairo`       | Cairo + Pango + HarfBuzz **drawing** (`entry` / backends) | yes                                                                                                                                                                         |
| `drun`                 | `.desktop` scanning + `tofi-drun` behavior                | yes                                                                                                                                                                         |
| `run-command-cache`    | Cached PATH command list for **`tofi-run`**               | yes (optional merge into `default` only)                                                                                                                                    |
| `clipboard-wayland`    | Paste                                                     | yes                                                                                                                                                                         |
| `history`              | History file read/write                                   | yes                                                                                                                                                                         |
| `single-instance-lock` | `flock` lock file                                         | yes                                                                                                                                                                         |

**Library rules:**

- Every optional subsystem must **`compile` when disabled**: use `#[cfg(feature = "...")]` on modules and stub APIs that return `Unsupported` or skip Wayland registration.
- Prefer separate files (`clipboard_wayland.rs` vs `clipboard_stub.rs`) over huge `#[cfg]` blocks inside one function.

**Example packager scenarios** (via **`tofi-rs`** forwarding):

- **Minimal stdin-only:** disable `clipboard-wayland`, `drun`, `history` as needed (often keep `wayland`, `renderer-cairo`).
- **No drawing stack / CI stub:** disable **`renderer-cairo`** (only if the code path supports it).

### 4.2 `tofi-rs` (CLI) — mirror the library

Document in **`tofi/Cargo.toml`**. For **each** feature in §4.1, define **`name = ["libtofi-rs/name"]`** — including **`renderer-cairo`**.

```toml
[dependencies]
libtofi-rs = { path = "../libtofi", default-features = false }

[features]
default = ["wayland", "renderer-cairo", "drun", "clipboard-wayland", "history", "single-instance-lock", "run-command-cache"]
wayland = ["libtofi-rs/wayland"]
renderer-cairo = ["libtofi-rs/renderer-cairo"]
drun = ["libtofi-rs/drun"]
clipboard-wayland = ["libtofi-rs/clipboard-wayland"]
history = ["libtofi-rs/history"]
single-instance-lock = ["libtofi-rs/single-instance-lock"]
run-command-cache = ["libtofi-rs/run-command-cache"]
```

(`default` lists must match **§4.1**.)

**CLI rules:**

- **Same names, same defaults** as §4.1 for all **mirrored** features.
- **`tofi-rs`** may add **only** non-colliding extras (e.g. `cli-tracing`) — see earlier §4 text.
- **`cargo build -p tofi-rs --no-default-features --features "…"`** selects the same **`libtofi-rs`** surface.
- **Optional extra `[[bin]]` targets** later: **`required-features`** as needed.

**UI:** Feature flags do not encode “UI definition” — that lives in **`tofi`** source (§1.4), not as a separate Cargo feature.

**Not in 1.0.0 scope:** hand-written shell completion install files—use **generated** completions via **`clap_complete`** later (§9).

**Upstream note:** The **`tofi-compgen`** helper binary is **not** recreated; run-mode caching stays in **`libtofi-rs`** only.

---

## 5. Follow-up process (how to execute this migration)

### 5.1 Workflow per step

1. **Pick** the next unchecked step from §6 (order matters early; later some parallel work is possible).
2. **Branch** (optional): `rust/step-XX-short-name`.
3. **Implement** only that step’s scope.
4. **Verify** using the step’s **Verification** commands **and** §5.2 (**mandatory fmt + clippy matrix**)—do not treat a step as finished until those pass.
5. **Mark** the step complete in this file (checkbox) or in a linked `docs/RUST_MIGRATION_CHECKLIST.md` if you prefer a separate checklist.
6. **Open PR** with: what changed, how verified, any intentional deviations from C.

### 5.2 Agent (AI) execution contract

For each step, the agent should:

- Read the **Goal**, **Scope**, **Deliverables**, **Verification**, and **C reference files** before coding.
- **Do not** delete C/Meson/legacy themes until **Phase 9**—keep them as reference while implementing earlier phases.
- Produce a **small diff**; if the step is too large, split into sub-steps and update this doc.
- **Add or extend Rust tests** for new/changed behavior when the code is **unit-testable** (see §5.3). **Do not** port [`test/`](../test/) from C.
- **Before declaring a step done** (PR, agent handoff, or “finished” in any sense), **all** of the following **must pass** on the workspace (same bar as CI once **Step 0.2** exists):
  - **`cargo fmt --all -- --check`** (or run **`cargo fmt --all`** and ensure a clean diff).
  - **`cargo clippy --workspace --all-targets -- -D warnings`** with **`--no-default-features`** **and** separately with **`--all-features`** (and keep **default** features green when you change defaults). If a step only touches one crate, still run clippy on the **whole** workspace unless the step explicitly documents a narrower scope.
  - **`cargo test --workspace`** (or **`cargo test -p libtofi-rs`** and **`cargo test -p tofi-rs`**) and **`cargo build --workspace`** as appropriate for the change. Before **Step 0.5**, **`cargo test`** may run **zero** tests; it must still **exit successfully**.
- Both workspace members that ship code **must** carry tests **once Step 0.5 is done** (§5.3)—the CLI is **not** exempt. After **Phase 0 Step 0.2**, CI mirrors **fmt**, **clippy** (warnings denied, feature matrix), and **test**—**local** runs should match before merge.

### 5.3 Testing strategy (new suite — not a port of C tests)

**Requirement:** Build a **new** automated test suite in Rust. The legacy Meson/C tests under [`test/`](../test/) are **not** translated line-by-line; they are optional behavioral reference only.

**Layout (per logical module in `libtofi-rs`):**

- Implementation lives under `libtofi/src/<module>/` (e.g. `unicode/`, `color/`). Use **`mod.rs`** as the main file for that directory (equivalent in role to a single `unicode.rs` next to `unicode/tests.rs`; Rust does not allow both `unicode.rs` and `unicode/` as siblings for the same module name).
- **Unit tests** live in a **sibling file** `tests.rs` inside that same directory, loaded only for tests:

```text
libtofi/src/
  unicode/
    mod.rs      # implementation (same module as `unicode.rs` would be)
    tests.rs    # #[cfg(test)] submodule: use super::*; #[test] fn …
```

In `mod.rs`:

```rust
#[cfg(test)]
mod tests;
```

This matches the pattern “`image/image.rs` + `image/tests.rs`” conceptually: **one folder per area**, implementation + **`tests.rs`**, not a monolithic `tests/` tree far from sources.

**Optional:** Crate-level **integration** tests in `libtofi/tests/*.rs` for scenarios that need multiple modules or filesystem fixtures.

**CLI crate (`tofi-rs`):** Must have tests as well—**not** library-only.

- Prefer **`tofi/tests/*.rs`** (integration tests: spawn the `tofi` binary with `std::process::Command`, assert `--help` / `--version` / exit status / stderr for invalid flags). This works even when `src/main.rs` is thin.
- If you split CLI logic into **`tofi/src/` modules** (e.g. `cli/mod.rs`), use the same **`tests.rs` sibling pattern** as `libtofi` for unit tests on parsing helpers.
- **Verification:** `cargo test -p tofi-rs` passes in CI alongside `cargo test -p libtofi-rs`.

**What to test first:** Pure functions in **`libtofi-rs`** (unicode, matching, path logic, **renderer helpers**); **config file parsing, CLI, and UI wiring** in **`tofi-rs`** (§1.4). **Harder:** Wayland/Cairo — prefer small pure helpers extracted for tests, plus manual compositor checks until/unless you invest in headless or snapshot tooling.

**CI expectation:** **`cargo test --workspace`** (with default features) passes on every merge; feature-gated code should use `#[cfg(all(test, feature = "..."))]` or split test modules so `--no-default-features` builds stay green.

**Fixture layout (post–Phase 9):** Canonical **config** files under **`examples/config/`** and **theme** files under **`examples/themes/`**—**separate test suites** (see Phase 9.5); do not merge into one catch-all fixture test.

### 5.4 Definition of “done” for the whole migration

- `cargo build --release --workspace` succeeds with **default features**.
- `cargo test --workspace` succeeds (**`libtofi-rs`** and **`tofi-rs`** both; expand coverage over time per §5.3).
- **`tofi-rs` `[features]`** stay **mirrored** to **`libtofi-rs`** per §4 (including **`renderer-cairo`**).
- `target/release/tofi` runs on Sway (or another wlroots compositor) in **stdin**, **run**, and **drun** modes (subject to enabled features).
- Config and theme files from [`doc/config`](../doc/config) and [`themes/`](../themes/) work or deviations are listed in a short `PARITY.md` (optional file—only if you want to track gaps; not required by this plan).
- **Meson / C-only CI** is **not** part of §5.4 “done”—it must already be **gone** (**Phase 0 Step 0.2**, §2.2). **Legacy removal** of **C sources, Meson build files, old themes tree**, etc., is **Phase 9** once you intentionally cut over the tree.

---

## 6. Phased steps (microsteps)

Each step: **Goal** · **Scope** · **Deliverables** · **Verification** · **C reference** · **Notes for implementers**

---

### Phase 0 — Workspace bootstrap

- [x] **Step 0.1 — Empty workspace compiles**
  - **Goal:** Tooling baseline.
  - **Scope:** Add root `Cargo.toml` `[workspace]` with members `libtofi`, `tofi` (paths **without** a `crates/` segment).
  - **Deliverables:** `libtofi` is a library crate with `pub fn noop()` or similar; `tofi` binary calls it.
  - **Verification:** `cargo build --workspace`; **`cargo fmt --all -- --check`**; **`cargo clippy --workspace --all-targets --no-default-features -- -D warnings`** and **`cargo clippy --workspace --all-targets --all-features -- -D warnings`** (§5.2). **`cargo test --workspace`** (may be 0 tests until **Step 0.5**).
  - **C reference:** N/A
  - **Notes:** Set **`edition = "2024"`** in `[workspace.package]` and/or per-crate `Cargo.toml`. **Do not** set `rust-version`—track latest stable (see §2.1).

- [x] **Step 0.2 — GitHub Actions CI (wayshot-style)**
  - **Goal:** **Automated checks on every push/PR** as soon as Rust code exists—**fmt**, **clippy**, **build** (feature matrix), **test** (once tests exist), optional **typos** / **coverage**; plus **Dependabot** (`.github/dependabot.yml` for **Cargo** and **github-actions**). Add a **`deploy.yml`** (match [wayshot](https://github.com/waycrate/wayshot) layout) **without** enabling publishes yet—see §9 **Deploy**.
  - **Scope:** Add `.github/workflows/*.yml` per §2.2 (**no** `deny.yml` / **`docs.yml`** in Phase 0). Add **`.github/dependabot.yml`**. Copy **`deploy.yml`** from wayshot (or minimal stub with the same triggers **disabled** / **`workflow_dispatch` only**). Adapt install steps and feature matrix for **`libtofi-rs`** / **`tofi-rs`**. Trigger active jobs on **`push`** and **`pull_request`**.
  - **Deliverables:** Green CI on a branch containing **Step 0.1**; Dependabot enabled on the repo; deploy workflow **present** but **not** auto-publishing; **legacy Meson-only workflow(s) removed** from `.github/workflows/` (§2.2). Minimal **`cargo test`** job can **`continue-on-error: true`** only until **Step 0.5** adds real tests—prefer **not** skipping the test job: let Step 0.5 land in the same milestone if needed so **`cargo test --workspace`** is required from day one.
  - **Verification:** PR shows passing **fmt**, **clippy**, **build** matrix; **no** remaining C/Meson-only CI jobs for this repo; **`cargo test --workspace`** passes after **Step 0.5** (or is wired and passes trivial smoke tests from **Step 0.5**); Dependabot config validates (GitHub shows Dependabot enabled / opens no erroneous PRs). Locally: same **§5.2** fmt/clippy commands as **Step 0.1**.
  - **C reference:** N/A
  - **Notes:** **Remove** legacy **Meson-only** CI in the **same** PR as the new Rust workflows (§2.2)—no parallel C/Rust CI. Optional: `paths:` filters on Rust workflows so purely-docs commits skip heavy jobs—do **not** ignore paths that contain **`Cargo.toml`** / **`Cargo.lock`** / Rust **`src/`**. **`cargo deny`** is **out of scope** for this step (§9).

- [ ] **Step 0.3 — CLI version and metadata**
  - **Goal:** User-visible identity for the Rust port.
  - **Scope:** `clap` (derive) or minimal `std::env` for `--version` / `--help`; version from `CARGO_PKG_VERSION`.
  - **Deliverables:** `tofi --version` prints version; help text stub.
  - **Verification:** `cargo run -p tofi-rs -- --version`
  - **C reference:** [`src/main.c`](../src/main.c) `usage()` (expand later).
  - **Notes:** Full option parity comes in Phase 4; here only scaffolding.

- [ ] **Step 0.4 — Feature skeleton (`libtofi-rs` + `tofi-rs`)**
  - **Goal:** Features declared in **both** crates (§4); default build = full stack.
  - **Scope:** Add **`libtofi/Cargo.toml` `[features]`** per §4.1; add **`tofi/Cargo.toml` `[features]`** per §4.2 (**mirror** §4.1 names → `libtofi-rs/…`). Wire empty modules behind `cfg` in the library.
  - **Deliverables:** `cargo build --no-default-features` works with minimal stubs for both packages; feature lists stay **in sync**.
  - **Verification:**
    - `cargo build -p libtofi-rs`
    - `cargo build -p libtofi-rs --no-default-features`
    - `cargo build -p tofi-rs`
    - `cargo build -p tofi-rs --no-default-features`
  - **C reference:** N/A
  - **Notes:** Document features in `libtofi` `//!` docs; in `tofi/Cargo.toml` comment that features **mirror** `libtofi-rs` (§4.2).

- [ ] **Step 0.5 — Unit test layout convention (library + CLI)**
  - **Goal:** Establish test layout for **both** crates before feature code lands.
  - **Scope:** (1) In `libtofi/`, add one trivial module (e.g. `src/sanity/mod.rs` + `src/sanity/tests.rs`) with `#[cfg(test)] mod tests;` and a single `#[test] fn smoke()`. (2) In `tofi/`, add **`tests/cli_smoke.rs`** (or similar) that runs the binary with `--version` or `--help` and asserts success—see §5.3.
  - **Deliverables:** `cargo test -p libtofi-rs` and **`cargo test -p tofi-rs`** each run at least one test.
  - **Verification:** `cargo test --workspace`
  - **C reference:** N/A
  - **Notes:** Remove/repurpose `sanity` later; CLI smoke test can grow into full CLI coverage.

---

### Phase 1 — Pure Rust foundations (no Wayland)

- [ ] **Step 1.1 — Error type**
  - **Goal:** Single error handling style for the library.
  - **Scope:** `thiserror` or manual enum; no `anyhow` inside library public API (CLI may use `anyhow`).
  - **Deliverables:** `libtofi::Error` and `Result<T>`.
  - **Verification:** `cargo test -p libtofi-rs`
  - **C reference:** [`src/log.c`](../src/log.c) patterns.
  - **Notes:** Map Wayland errors later.

- [ ] **Step 1.2 — Unicode helpers**
  - **Goal:** Match [`src/unicode.c`](../src/unicode.c) behavior needed by input and paste.
  - **Scope:** UTF-8 validation, NFC normalization (use `unicode-normalization` crate if appropriate), UTF-8 ↔ UTF-32 for fixed buffer sizes as in C.
  - **Deliverables:** Module `libtofi::unicode` with `unicode/tests.rs` covering edge cases (**new** tests; do not port C tests).
  - **Verification:** `cargo test -p libtofi-rs`
  - **C reference:** [`src/unicode.c`](../src/unicode.c)
  - **Notes:** Mirror `MAX_INPUT_LENGTH`-style limits as constants.

- [ ] **Step 1.3 — Color parsing**
  - **Goal:** Theme colors as in [`src/color.c`](../src/color.c).
  - **Scope:** Parse config color strings into linear/sRGB as C does.
  - **Deliverables:** `libtofi::color` with `Color` type and `color/tests.rs`.
  - **Verification:** `cargo test -p libtofi-rs`
  - **C reference:** [`src/color.c`](../src/color.c)

- [ ] **Step 1.4 — String tables**
  - **Goal:** Replace [`src/string_vec.c`](../src/string_vec.c) patterns with idiomatic `Vec`/`SmallVec`/`String` while keeping deterministic ordering for results.
  - **Deliverables:** `libtofi::string_table` (name as you prefer) + `tests.rs`.
  - **Verification:** `cargo test -p libtofi-rs`
  - **C reference:** [`src/string_vec.h`](../src/string_vec.h)

- [ ] **Step 1.5 — Matching algorithms**
  - **Goal:** Parity with [`src/matching.c`](../src/matching.c).
  - **Scope:** Fuzzy vs non-fuzzy, same public behavior as C for ranking and filtering.
  - **Deliverables:** `libtofi::matching` + `matching/tests.rs` with representative cases.
  - **Verification:** `cargo test -p libtofi-rs`
  - **C reference:** [`src/matching.c`](../src/matching.c), `matching_algorithm` in [`src/tofi.h`](../src/tofi.h)

---

### Phase 2 — Config system

- [ ] **Step 2.1 — Config data structures (shared model)**
  - **Goal:** One Rust struct (or layered structs) covering all keys documented in [`doc/config`](../doc/config) / man.
  - **Scope:** Mirror defaults from [`src/main.c`](../src/main.c) and [`doc/config`](../doc/config).
  - **Deliverables:** **`libtofi::config::TofiConfig`** (name flexible) + `Default` — **types only**, consumed by the engine; **`libtofi/config/tests.rs`** for `Default` / invariants.
  - **Verification:** `cargo test -p libtofi-rs`
  - **C reference:** [`src/tofi.h`](../src/tofi.h), [`src/entry.h`](../src/entry.h)
  - **Notes:** §1.4 — no file parsing in the library.

- [ ] **Step 2.2 — Config file parser**
  - **Goal:** Load the same keyfile format as C.
  - **Scope:** Line-based parser compatible with [`src/config.c`](../src/config.c) (comments, `key=value`, includes if supported).
  - **Deliverables:** **`tofi::config`** (or `tofi::config::load`): read file → populate **`libtofi::config::TofiConfig`**; tests in **`tofi/src/.../tests.rs`** or **`tofi/tests/`** with fixtures + [`doc/config`](../doc/config).
  - **Verification:** **`cargo test -p tofi-rs`**
  - **C reference:** [`src/config.c`](../src/config.c)

- [ ] **Step 2.3 — Config key application**
  - **Goal:** Equivalent to `config_apply()` for every key.
  - **Scope:** **`tofi`**: stringly-typed apply used by both file loader and CLI layer.
  - **Deliverables:** **`tofi::config::apply_key`**, unit tests for representative keys.
  - **Verification:** **`cargo test -p tofi-rs`**
  - **C reference:** [`src/config.c`](../src/config.c)

- [ ] **Step 2.4 — CLI parsing (full)**
  - **Goal:** Replace `getopt_long` parity: all options in [`src/main.c`](../src/main.c) `long_options`.
  - **Scope:** Use `clap` with long names matching exactly; two-pass behavior (config path first, then overrides).
  - **Deliverables:** **`tofi::cli`** (`clap`) producing **`TofiConfig`** + runtime mode flags; **mandatory** **`tofi/tests/`** coverage for help, unknown flags, valid/invalid invocations.
  - **Verification:** Run binary with `--help` listing all flags; `--config` loads file; **`cargo test -p tofi-rs`** passes.
  - **C reference:** [`src/main.c`](../src/main.c) `parse_args`

---

### Phase 3 — History, lock, run-mode command cache, drun (non-Wayland)

- [ ] **Step 3.1 — History** (`feature = "history"`)
  - **Goal:** Same file format and ordering as [`src/history.c`](../src/history.c).
  - **Deliverables:** `libtofi::history` + `history/tests.rs` (tempdir / fixture files).
  - **Verification:** `cargo test -p libtofi-rs --features history` and manual read/write cycle
  - **C reference:** [`src/history.c`](../src/history.c)

- [ ] **Step 3.2 — Single-instance lock** (`feature = "single-instance-lock"`)
  - **Goal:** Same `flock` behavior as [`src/lock.c`](../src/lock.c).
  - **Deliverables:** `libtofi::lock` using `fs2` or direct `flock` syscall via `libc`.
  - **Verification:** Run two instances; second exits when `multi-instance` false.
  - **C reference:** [`src/lock.c`](../src/lock.c)

- [ ] **Step 3.3 — Run-mode command cache** (C `compgen.c`; see §3.5)
  - **Goal:** Same behavior as [`src/compgen.c`](../src/compgen.c): subprocess to bash `compgen -c`, cache under XDG cache, expose list for **`tofi-run`**.
  - **Deliverables:** Library module only (e.g. `libtofi::run_commands`); `tests.rs` for cache path logic and parsing. **No** `tofi-compgen` binary—**shell completions** use **`clap`** later (§9), not this module.
  - **Verification:** `cargo test -p libtofi-rs`; run mode lists commands like C.
  - **C reference:** [`src/compgen.c`](../src/compgen.c) — ignore [`src/main_compgen.c`](../src/main_compgen.c) as a **product** (debug helper only); replicate its stdout only if you need a one-off dev binary, not for release.

- [ ] **Step 3.4 — drun desktop loading** (`feature = "drun"`)
  - **Goal:** Port [`src/drun.c`](../src/drun.c): scan paths, cache file, desktop entry list, `Exec` handling.
  - **Deliverables:** `libtofi::drun`
  - **Verification:** List apps matches C `tofi-drun` for a fixed `XDG_DATA_*` (manual).
  - **C reference:** [`src/drun.c`](../src/drun.c), [`src/desktop_vec.c`](../src/desktop_vec.c)

---

### Phase 4 — Wayland core (no drawing yet)

- [ ] **Step 4.1 — Wayland dependencies wired**
  - **Goal:** Resolve `wayland-client` + protocol crates without duplicate versions.
  - **Deliverables:** `Cargo.lock` shows one version per stack; `libtofi` connects to compositor and exits.
  - **Verification:** `WAYLAND_DISPLAY=... cargo run -p tofi-rs` connects and exits cleanly (stub).
  - **C reference:** [`src/main.c`](../src/main.c) `wl_display_connect`

- [ ] **Step 4.2 — Registry globals**
  - **Goal:** Bind compositor, shm, seat, `zwlr_layer_shell_v1`, `wp_viewporter`, `wp_fractional_scale_manager_v1`, `wl_output` list.
  - **Deliverables:** State struct holding globals; roundtrip after bind.
  - **Verification:** Log (tracing) lists bound globals on Sway.
  - **C reference:** [`src/main.c`](../src/main.c) registry listener

- [ ] **Step 4.3 — Layer surface + opaque region**
  - **Goal:** Create surface, layer surface, anchor, margins as per config.
  - **Deliverables:** Visible solid-color buffer (single color) proves placement.
  - **Verification:** Screen shows rectangle; ESC quits.
  - **C reference:** [`src/main.c`](../src/main.c) layer setup

- [ ] **Step 4.4 — SHM double buffer**
  - **Goal:** Port [`src/surface.c`](../src/surface.c) / [`src/shm.c`](../src/shm.c): allocate shm, two buffers, attach/commit.
  - **Deliverables:** Module `libtofi::shm`
  - **Verification:** Flip buffers without tearing (visual).
  - **C reference:** [`src/surface.c`](../src/surface.c), [`src/shm.c`](../src/shm.c)

- [ ] **Step 4.5 — Output scale & fractional scale**
  - **Goal:** Match [`src/scale.c`](../src/scale.c) and fractional-scale listener behavior.
  - **Deliverables:** Correct buffer dimensions vs logical size.
  - **Verification:** Test on HiDPI output; compare with C.
  - **C reference:** [`src/scale.c`](../src/scale.c), [`src/main.c`](../src/main.c)

---

### Phase 5 — Rendering (Cairo / Pango / HarfBuzz) — **implementation in `libtofi-rs`, UI wiring in `tofi-rs`**

- [ ] **Step 5.1 — Cairo bind to SHM**
  - **Goal:** Create `cairo::ImageSurface` from mapped buffer (ARGB) in **`libtofi-rs`**; draw simple text “hello” with Pango.
  - **Deliverables:** **`libtofi::render`** (or `entry` submodule); feature **`renderer-cairo`**; **`cargo test -p libtofi-rs`** where possible.
  - **Verification:** Visual
  - **C reference:** [`src/entry.c`](../src/entry.c)
  - **Notes:** §1.4 — **drawing stack** in the library.

- [ ] **Step 5.2 — Entry layout port**
  - **Goal:** Port [`src/entry.c`](../src/entry.c) / backends: prompt, input, list, selection, clip rect.
  - **Deliverables:** **`libtofi::entry`** + backends (pango/harfbuzz paths); **`tofi::ui`** (or `main` modules) only as needed to **define** how the running app invokes the renderer with **`TofiConfig`**.
  - **Verification:** Side-by-side screenshot compare with C (same theme file); **`cargo test`** on both crates where testable.
  - **C reference:** [`src/entry.c`](../src/entry.c), [`src/entry_backend/pango.c`](../src/entry_backend/pango.c), [`src/entry_backend/harfbuzz.c`](../src/entry_backend/harfbuzz.c)

- [ ] **Step 5.3 — Result list + scrolling**
  - **Goal:** `first_result`, `num_results`, horizontal mode — behavior in **`libtofi`**; **`tofi`** wires config / mode flags.
  - **Deliverables:** Engine supports scrolling; CLI passes UI-relevant options.
  - **Verification:** Keyboard navigation works (next phase completes input).
  - **C reference:** [`src/entry.h`](../src/entry.h)

---

### Phase 6 — Input and behavior

- [ ] **Step 6.1 — XKB state**
  - **Goal:** Keymap from compositor, state updates, key repeat.
  - **Deliverables:** `libtofi::input::keyboard`
  - **Verification:** Typing UTF-8 text in field.
  - **C reference:** [`src/main.c`](../src/main.c) keyboard listeners, [`src/input.c`](../src/input.c)

- [ ] **Step 6.2 — Key bindings**
  - **Goal:** Same default bindings as C (move selection, accept, cancel, paste, etc.).
  - **Verification:** Manual checklist against **`tofi --help`** and [`doc/tofi.1.md`](../doc/tofi.1.md) (reference doc only; no man-page build required).
  - **C reference:** [`src/input.c`](../src/input.c)

- [ ] **Step 6.3 — Pointer / hide cursor**
  - **Goal:** Pointer motion, hide cursor option.
  - **C reference:** [`src/main.c`](../src/main.c)

- [ ] **Step 6.4 — Modes: stdin / run / drun**
  - **Goal:** Populate `commands` from stdin, `$PATH` scan, or desktop list; `argv[0]` basename detection like C (`strstr` for `-run` / `-drun`).
  - **Verification:** Three modes behave like C.
  - **C reference:** [`src/main.c`](../src/main.c) post-config initialization

- [ ] **Step 6.5 — Submit and stdout**
  - **Goal:** `do_submit` parity from [`src/main.c`](../src/main.c): `drun_launch`, `print_index`, history append.
  - **Verification:** Shell pipelines from README work.
  - **C reference:** [`src/main.c`](../src/main.c) `do_submit`

---

### Phase 7 — Clipboard (`feature = "clipboard-wayland"`)

- [ ] **Step 7.1 — Data device manager**
  - **Goal:** Bind `wl_data_device_manager`, create data device, handle paste shortcut.
  - **Deliverables:** Paste UTF-8 into input at cursor.
  - **Verification:** Copy from another Wayland app, paste into tofi.
  - **C reference:** [`src/main.c`](../src/main.c) `read_clipboard`, clipboard listeners

- [ ] **Step 7.2 — Optional arboard path** (optional step)
  - **Only if** you need a fallback feature for non-Wayland testing; not required for parity.

---

### Phase 8 — Packaging and polish

- [ ] **Step 8.1 — Install paths**
  - **Goal:** Match Meson where still relevant: e.g. default config under `sysconfdir/…/tofi`, theme paths—**no man-page install** (not a target).
  - **Notes:** Use `build.rs` or distro packaging; document install layout for packagers.

- [ ] **Step 8.2 — (Deferred)** Shell completions **not** in 1.0.0
  - **Goal:** Omit shipping hand-maintained completion files in the first Rust release.
  - **Follow-up:** Generated completions via **`clap_complete`** (and friends) for multiple shells—see §9.

- [ ] **Step 8.3 — Performance passes**
  - **Goal:** `MADV_HUGEPAGE` equivalent if applicable, double buffering, minimize redraws—match C hot paths.
  - **C reference:** [`src/surface.c`](../src/surface.c) comments

- [ ] **Step 8.4 — Release hardening**
  - **Goal:** `deny(unsafe_code)` where possible; document `unsafe` blocks for FFI; CI must already enforce **`cargo clippy -- -D warnings`** and **`cargo test --workspace`** (Phase 0 Step 0.2, §2.2). Use §9 for **`cargo deny`**, **deploy**, and other **late** automation (coverage gates, etc.).
  - **Verification:** CI green

---

### Phase 9 — Legacy tree removal & canonical examples

**When:** Only after the Rust port meets §5.4 (and you are willing to drop the C build). Optionally **tag** the last C commit (e.g. `legacy/c-before-rust`) so history stays recoverable.

**Goal:** Delete upstream C/Meson/legacy assets that are obsolete, and replace scattered **themes** / **examples** with **dedicated trees**: **`examples/config/`** (app config only) and **`examples/themes/`** (theme files only)—each with at least one **large** reference file listing defaults. **Run separate test patterns** for config vs theme fixtures so they stay in sync with parsers and with each other (see 9.5).

- [ ] **Step 9.1 — Inventory & cutover criteria**
  - **Goal:** Written checklist of what gets removed vs kept; agreement that **`cargo build --release`** is the only supported build.
  - **Deliverables:** Short note in repo (e.g. `docs/CUTOVER.md` or a section in README) — optional; can be a PR description if you prefer minimal docs.
  - **Verification:** Team sign-off / self-review.

- [ ] **Step 9.2 — Remove C build and sources**
  - **Goal:** No Meson/C toolchain in-tree for the product.
  - **Scope (typical):** Delete or move to an archive branch: **`meson.build`**, **`src/**/_.c`**, **`src/\*\*/_.h`**, **`test/`** (C tests — already not ported), **`protocols/\*.xml`** if unused by Rust, any **Meson-only** scripts. **C-only CI** should already have been removed in **Step 0.2** (§2.2); here, scrub any **leftover\*\* references to Meson CI in docs or scripts.
  - **Verification:** Repo has no dangling references to removed paths in active README/install instructions; **`git grep meson`** / **`git grep '\.c'`** clean where intended.

- [ ] **Step 9.3 — Replace themes / scattered examples**
  - **Goal:** Remove the **old** [`themes/`](../themes/) tree and ad hoc example configs that duplicated defaults—**after** you have new canonical files (Step 9.4).
  - **Scope:** Delete or archive per 9.1; avoid leaving users with no examples.

- [ ] **Step 9.4 — Canonical fixtures in **separate** folders**
  - **Goal:** **Configs and themes live in different directories**—no single mixed `examples/` flat file dump. Clear convention for users and for tests.
  - **Deliverables:**
    - **`examples/config/`** — at least one **full** app config (e.g. `full` or `default-keys`) with **all** (or nearly all) keys set to **documented defaults** + comments.
    - **`examples/themes/`** — at least one **full** theme file the same way (defaults + comments).
  - **Rules:** Do **not** place theme content under `examples/config/` or app-only keys under `examples/themes/`. Document both paths in README. Align with future **TOML** (§9) when you migrate; until then, match the supported keyfile format.
  - **Verification:** `tofi --config examples/config/…` and theme loading (however the binary references themes) succeed without error.

- [ ] **Step 9.5 — **Different** test patterns for config vs themes**
  - **Goal:** CI fails if either fixture tree drifts from code defaults or breaks parsing—not one monolithic test that only checks “something loaded.”
  - **Deliverables (patterns — implement as modules or test files):**
    - **Config pattern:** tests under **`libtofi`** and/or **`tofi`** that load **`examples/config/\***`via`CARGO_MANIFEST_DIR` (or workspace-root env), assert parsed **`TofiConfig`** (or subset) matches **`Default`\*\* / golden expectations, error on unknown keys if applicable.
    - **Theme pattern:** **separate** tests that load **`examples/themes/\***`only, validating theme-specific parsing / colors / spacing — **not** the same`#[test]` as config unless shared helpers only.
  - **Verification:** **`cargo test --workspace`** green; changing **`examples/config/`** updates **config** tests; changing **`examples/themes/`** updates **theme** tests.
  - **Notes:** Optional: `include_str!` snapshots or small golden files checked into `tofi/tests/fixtures/` if you need stable baselines—still keep **folder split** at the source of truth under **`examples/`**.

**Notes:** Do **not** delete the legacy **source tree** in **Phase 0**—keep C sources as reference until Phases 1–8 are done. **CI** switches to Rust-only when **Step 0.2** lands (§2.2). Phase 9 is a deliberate **cleanup** milestone for **files**, not CI.

---

## 7. Parallelism (after Phase 2)

After config + pure modules exist, **Phase 3 sub-steps** (history, lock, run-mode cache, drun) can proceed in parallel branches if coordination on `libtofi` API is agreed. **Wayland (Phase 4)** should stay linear until layer shell works.

---

## 8. Risk register (short)

| Risk                                        | Mitigation                                                                                                                       |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Layer-shell bindings                        | Primary: **[`wayland-protocols-wlr`](https://crates.io/crates/wayland-protocols-wlr)**; fallback: `build.rs` + `wayland-scanner` |
| Font/layout drift vs Cairo/Pango C          | Side-by-side screenshots; same font files                                                                                        |
| Clipboard differences                       | Prefer porting `wl_data_device` path first                                                                                       |
| Stale / unmaintained dependencies           | Enforce §2.1 and §3 before adding crates                                                                                         |
| `libtofi-rs` / `tofi-rs` feature drift      | Single source of truth: §4.1; **mirror** all flags in §4.2 (including **`renderer-cairo`**)                                      |
| Config/UI vs renderer in wrong crate        | §1.4: **config parse + UI definition** in **`tofi-rs`**; **Cairo/render implementation** in **`libtofi-rs`**                     |
| Deleting legacy tree too early              | Run **Phase 9** only after §5.4; **tag** last C commit if you need a rollback reference                                          |
| Accidental strong copyleft via `Cargo.lock` | §1.5; **`cargo deny`** license policy; prefer permissive crates when you must ship MIT-compatible binaries                       |

---

## 9. Future goals (post-1.0.0 or parallel track)

These are **not** required to declare the C→Rust migration “done” for §5.4, but they are explicit project intentions:

| Goal                  | Notes                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Config format**     | Replace the legacy keyfile-style config with **`.toml`** (or split: themes vs app config). Plan migration and compatibility window in a dedicated doc when you start.                                                                                                                                                                                                                                                                                       |
| **Documentation**     | Refresh **README** and in-repo docs for the Rust workflow, install, and **Cargo feature flags** (§4); man pages optional/out of scope unless you add them later outside this plan.                                                                                                                                                                                                                                                                          |
| **CI/CD**             | **Baseline in Phase 0** (§2.2, Step 0.2): **`cargo fmt`**, **`cargo clippy`**, **`cargo test --workspace`**, **`cargo build --release`**, feature matrix, **Dependabot** (`dependabot.yml`), plus optional **typos** / **coverage**—modeled on [wayshot](https://github.com/waycrate/wayshot) (excluding **`deny.yml`** / **`docs.yml`** from that baseline). **Here:** optional extras—scheduled runs, stricter policies, badge hygiene—aligned with §2.1. |
| **`cargo-deny`**      | **Separate** milestone (not Phase 0): add **`deny.toml`** + **`deny.yml`** (or equivalent job) when the dependency graph is stable enough to justify **`cargo deny check`** for licenses/advisories/bans—aligns with §1.5; **after** Dependabot is already filing crate bumps.                                                                                                                                                                              |
| **Deploy**            | **`deploy.yml`** is **added in Step 0.2** (see [wayshot](https://github.com/waycrate/wayshot/blob/main/.github/workflows/deploy.yml)) but **kept idle** (`workflow_dispatch` only, `if: false`, or no tags) until the Rust port is **release-ready**. **Late migration:** enable triggers for **crates.io**, **GitHub Releases**, or distro artifacts; wire secrets; align with §5.4 and packaging docs.                                                    |
| **Shell completions** | Generate completions for **bash, zsh, fish, …** using **`clap_complete`** (or the ecosystem standard that matches your `clap` version)—**not** a separate `tofi-compgen`-style binary. **Not** a 1.0.0 release blocker (see §8 Step 8.2).                                                                                                                                                                                                                   |
| **Legacy deletion**   | **C-only CI:** removed in **Phase 0 Step 0.2** (§2.2). **Phase 9:** remove C sources, Meson build, old [`themes/`](../themes/) once Rust is default; replace with **`examples/config/`** + **`examples/themes/`** + **split** config/theme test patterns (9.4–9.5).                                                                                                                                                                                         |

**License note:** A **permissive** outcome for your own code is a **core migration goal** (§1.1); **strong copyleft** may still apply **via dependencies**—see §1.5.

---

## 10. Document maintenance

- Update **checkboxes** in §6 (Phases **0–9**) as steps complete.
- When a step is split or reordered, **add a one-line changelog** at the bottom of this file (`### Revision history`).
- After **policy or workflow changes** (CI, §5.2, Phase 9 scope, etc.), **re-read** §2.2, §5, and §6 for **contradictions** and fix them in the same edit series—**implementation and plan stay aligned**.

### Revision history

- **2026-04-06:** Typos config file: **`_typos.toml`** → **`.typos.toml`** (same contents).
- **2026-04-06:** **Phase 0 Step 0.2** — Rust CI (`build.yml`, `fmt-clippy.yml`, `test.yml`, `typos.yml`, idle `deploy.yml`), **`dependabot.yml`**, **`.typos.toml`** (exclude legacy trees); removed Meson **`build-test.yml`**; §6 checkbox.
- **2026-04-06:** **Phase 0 Step 0.1** complete — empty workspace (`libtofi-rs` + `tofi-rs`), §6 checkbox.
- **2026-04-06:** **§1 / §5–6 / §10:** Playbook priority — **read plan → implement → update plan**; intro + **Step 0.1** verification include **§5.2** fmt/clippy; **§5.2** clarifies tests required **after Step 0.5**; **Step 0.2** deliverables/verification include **Meson CI removal**; fix **Step 9.2** typo; **§10** — re-read for contradictions after policy changes.
- **2026-04-06:** **§2.2:** CI **`uses:`** pins — **major only** (`x`), not **`x.y`** / patch; contrast **§2.1** **`Cargo.toml`** **`x.y`**.
- **2026-04-06:** **§2.1:** **`Cargo.toml`** dependency versions prefer **`x.y`** over **`x.y.z`** (exact pins in **`Cargo.lock`**); **§2.2** Dependabot note cross-links. Simplifies upgrades and Dependabot PRs.
- **2026-04-06:** **§2.2 / Step 0.2:** **Dependabot** (`dependabot.yml`); **no** Phase 0 **`deny.yml`** / **`docs.yml`**; **`deploy.yml`** present but idle until §9 **Deploy**. §9 **`cargo-deny`** = later dedicated step.
- **2026-04-06:** **§2.2** + **Phase 0 Step 0.2:** CI tooling **early**, modeled on [waycrate/wayshot](https://github.com/waycrate/wayshot) (`build.yml`, `fmt-clippy.yml`, `test-coverage.yml`, `typos.yml`); **remove** Meson-only CI **when** Rust CI is added (same milestone). Renumbered Phase 0: **0.3** CLI metadata, **0.4** feature skeleton, **0.5** test layout. **§9** / **Step 8.4:** CI baseline is Phase 0, not deferred.
- **2026-04-06:** **§5.2:** Mandatory **`cargo fmt --check`** and **`cargo clippy`** (**`--no-default-features`** and **`--all-features`**, **`-D warnings`**) before declaring a step done; align with §2.2 CI matrix.
- **2026-04-06:** Tooling: **edition 2024**, workspace + per-crate `Cargo.toml`, **no** `rust-version` pin, **no** `crates/` path segment; dependency freshness (§2.1); **`compgen` clarified** (§3.5); removed **`tofi-compgen`** binary from plan; Phase 3/8 adjusted. **§1.1 / §1.5:** permissive target; **strong copyleft via deps** possible; copyright (retain upstream, add yourself for your work). **§9:** TOML config, docs, CI/CD, **`cargo-deny`**, completions, **Phase 9 legacy deletion**; GPL future goal removed. **Tests:** **CLI (`tofi-rs`) must have tests** (`tofi/tests/`, `cargo test --workspace`); §5.2–5.4, Step 0.5/2.4/8.4, layout. **§4:** full feature mirror including **`renderer-cairo`** on **`libtofi-rs`**; **no man pages**; layout + Step 0.4; Phase 8.1 / §9 / risk. **§3.1 / §8:** **`wayland-protocols-wlr`**. **§1.4:** **UI definition** (`tofi-rs`) vs **renderer implementation** (`libtofi-rs`); config parse in **`tofi`**, types in **`libtofi`**. **Phase 0.2:** C-only CI out; **Phase 9:** remove C/Meson/themes; **`examples/config/`** + **`examples/themes/`**; **separate** config vs theme test patterns.
- **2026-04-06:** Initial plan from C codebase survey (`meson.build`, `src/`). **Update same day:** testing policy — **new** Rust suite with per-module `tests.rs` (§5.3); **no** port of C `test/`; Step 0.5; Phase 1–3 and 8 updated to require `cargo test` where appropriate.
