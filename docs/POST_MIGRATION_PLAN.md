# Post-migration plan (0.10.x → 1.0.0)

This document picks up where [`RUST_MIGRATION_PLAN.md`](RUST_MIGRATION_PLAN.md) leaves off: the Rust port is largely complete, but **bugfixes**, **compatibility with upstream [tofi](https://github.com/philj56/tofi)**, and **release discipline** still need a clear path before tagging.

**Scope of this file**

- **Primary focus:** the **v0 series**, culminating in **`0.10.0`**, where the goal is **maximal behavioral compatibility** with upstream `tofi` (see baseline below).
- **Secondary:** **`1.0.0`** — a **narrow, opinionated** fork (desktop launcher only, TOML config/theme, heavy deletion). Part B is the working roadmap for that release.

---

## Reference baseline

| Item | Choice |
|------|--------|
| **Upstream compatibility target** | [philj56/tofi](https://github.com/philj56/tofi) at **`0.9.1`** (see [`CHANGELOG.md`](../CHANGELOG.md)) |
| **This fork’s compatibility tag** | **`0.10.0`** — “theoretical” parity with upstream; v0 releases refine that claim with testing and fixes |
| **Non-goals for parity** | Pixel-identical frames every time; documented minor timing/layout differences are acceptable (see migration plan §1.2) |

Keep this section updated if the upstream reference tag changes.

---

## Part A — Pre-0.10.0 (v0 series)

### A.1 Goal

Ship **`0.10.0`** (and any **`0.10.x` patches**) so that users migrating from **C `tofi`** can treat **tofi-rs** as a **drop-in** for supported workflows: same config concepts, same CLI modes, and **no surprises** on wlroots-style Wayland compositors you care about (e.g. Sway, Hyprland, etc.—list the ones you actually test).

### A.2 Bugfix and compatibility work (checklist)

Use this as a living backlog; tick items when verified fixed or explicitly documented as different.

**Versions and release metadata**

- [x] Bump **`version`** in all **`Cargo.toml`** manifests that participate in the release (**workspace** root if used, **`libtofi`**, **`tofi`**, and any other publishable crates) so installed binaries and crates.io/GitHub releases report **0.10.x** consistently.

**Code hygiene (pre-0.10 finish line)**

- [x] **Purge migration-plan noise from source:** remove or rewrite **comments**, **module/file names**, **string literals**, and **docs embedded in code** that reference the Rust migration, `RUST_MIGRATION_PLAN`, `POST_MIGRATION_PLAN`, “phase” checklists, or other migration-era scaffolding. The codebase on **`main`** should read like a normal application, not a migration diary. Use repo history and the **planning archives branch** (see below) for archival planning text.

**Planning archives (pre-0.10.0 release — not 1.0)**

- [ ] Before tagging **`0.10.0`**, move **`docs/RUST_MIGRATION_PLAN.md`**, **`docs/POST_MIGRATION_PLAN.md`**, and any other **migration-era PLAN** files to a **dedicated long-lived branch** (e.g. `docs/migration-history`). They should **not** remain on **`main`** once **0.10** ships (history stays in git on that branch and in tags).
- [ ] On **`main`**, replace deep links with pointers to that branch or to a **tag** snapshot (e.g. `v0.10.0`) so readers can still open the old plans without carrying them in the default tree.
- [ ] After this move, ongoing work toward **1.0** uses a **`main`** that is free of migration-plan documents.

**Build / packaging**

- [ ] Release **`tofi-rs`** builds cleanly on **default features** and documented **feature matrices** (match `Cargo.toml` / CI).
- [ ] Document **system dependencies** for packagers (Wayland, Cairo, Pango, HarfBuzz, xkbcommon, etc.) and any divergence from upstream’s Meson story.
- [ ] **Binary name** remains **`tofi`** when installed (per migration plan); document conflicts if both C and Rust packages install the same path.

**CLI and config parity**

- [ ] **`--help` / `--version`** align with upstream expectations (wording can differ; **flags and semantics** should not surprise migrators).
- [ ] **Keyfile format** and **`include`**: same keys accepted; unknown keys handled consistently (warn vs ignore—match upstream or document).
- [ ] **CLI overrides** apply in the same order / precedence as upstream (or document differences).

**Modes and features**

- [ ] **Stdin / dmenu-style** mode: selection, cancellation, exit codes.
- [ ] **`tofi-run`** (cached PATH / compgen-style list): cache location, invalidation, behavior when `bash` / `compgen` unavailable if applicable.
- [ ] **`tofi-drun`**: desktop entry discovery, ordering, launching, icons if supported.
- [ ] **Matching** algorithms and case sensitivity match upstream for the same config.
- [ ] **History** file path, format, and deduplication behavior.
- [ ] **Clipboard paste** (Wayland): parity with upstream on compositors you test; document any protocol/backend difference (`wl_data_device` vs data-control, etc.).
- [ ] **Single-instance lock** (if enabled): same rough semantics (no deadlocks, clear error when locked).

**Wayland / input / rendering**

- [ ] **Layer shell** placement, keyboard focus, and **exit on Escape / accept**.
- [ ] **Pointer** behavior if upstream supports it for your build.
- [ ] **HiDPI / fractional scale**: no broken scaling vs upstream on the same setup.
- [ ] **Text layout**: RTL, combining characters, and ellipsis behavior **close enough** to upstream (full pixel parity is not required).

**Tests and automation**

- [ ] **`cargo test`** (and CI) green on **fmt, clippy, test** matrices you use in production.
- [ ] Expand **CLI/unit tests** for any bugfix that can be locked in without a Wayland harness.

**CI/CD**

- [ ] **Deploy job** in CI (e.g. `.github/workflows/deploy.yml`): builds release **artifacts**, attaches them to **GitHub Releases** (and/or other targets you use), with **triggers** and **permissions** documented (`workflow_dispatch`, tag filters, or branch rules as appropriate). Align with the migration plan’s suggestion to **gate** automatic publishing until you are ready (see [`RUST_MIGRATION_PLAN.md`](RUST_MIGRATION_PLAN.md) §2.2).
- [ ] **Secrets** and **environments** (if any) configured for the deploy workflow; dry-run or test tag verified end-to-end.

**Shell completions**

- [ ] Generate **tab-completion scripts** from the CLI definition (e.g. **`clap`** + **`clap_complete`**) so flags stay aligned with `--help`.
- [ ] Support the shells you care about (**bash**, **zsh**, **fish**, etc.) and document **install locations** for packagers (e.g. bash-completion, `fish` vendor dir, zsh `site-functions`).
- [ ] Optional: CI check that **checked-in** completion files match a **`cargo xtask`** or **build script** regeneration (fail on drift).

**Documentation**

- [ ] **Migration note** in README or changelog: how to compare against upstream, where to report parity bugs.
- [ ] **Known differences** list (even if empty): builds confidence.

### A.3 Manual comparison: upstream `tofi` vs tofi-rs (step-by-step)

These steps are for **you** (or a tester) on a **Wayland session** with a **wlroots-class** compositor. Adjust paths and compositor names to your machine.

#### A.3.1 Install two binaries without PATH clashes

1. **Build upstream C `tofi`** from [philj56/tofi](https://github.com/philj56/tofi) at tag **`0.9.1`** (or the tag you locked in §Reference baseline). Install to a prefix, e.g. `~/opt/tofi-c`, so the binary is `~/opt/tofi-c/bin/tofi`.
2. **Build this repo** with `cargo build --release -p tofi-rs` and copy the binary to a distinct name, e.g. `~/bin/tofi-rs` (or install with a package name that provides `/usr/bin/tofi-rs`).
3. Confirm versions:
   - `~/opt/tofi-c/bin/tofi --version`
   - `~/bin/tofi-rs --version`

#### A.3.2 Use matching config for each run

1. Create a **dedicated test config directory**, e.g. `~/tofi-parity/` with:
   - `config` — minimal shared options.
   - One or more **theme** files referenced by that config (copy from upstream examples or your own known-good themes).
2. For **each** test below, run **both** binaries with the **same**:
   - `TOFI_*` env vars if you use them
   - `--config` / `--theme` / paths
   - Same **stdin** or mode-specific inputs

Example wrapper idea (repeat for C vs Rust):

```bash
# C build
~/opt/tofi-c/bin/tofi --config ~/tofi-parity/config < /tmp/tofi-input.txt

# Rust build
~/bin/tofi-rs --config ~/tofi-parity/config < /tmp/tofi-input.txt
```

#### A.3.3 Scenario checklist (same session, same theme)

For each row, run C then Rust; record **pass / fail / different** and a one-line note.

| # | Scenario | What to verify |
|---|----------|----------------|
| 1 | Stdin list, select item | Correct item on Enter; exit code 0 |
| 2 | Stdin list, cancel (Escape) | No selection; exit code matches upstream |
| 3 | Empty stdin / edge cases | No crash; behavior matches or is documented |
| 4 | `tofi-run` | List content, launch selected command, cache behavior |
| 5 | `tofi-drun` | Apps appear; launch works; hide nodisplay if applicable |
| 6 | Custom **keybindings** in config | Each binding matches |
| 7 | **Paste** (if used) | Middle-click or binding inserts text as upstream |
| 8 | **History** | Previous choices appear; ordering; write on select |
| 9 | **Matching** modes | Fuzzy / normal / case rules per config |
|10 | **Multi-monitor** (if applicable) | Window appears on expected output |

#### A.3.4 When behavior differs

1. Reproduce with **minimal** config and **smallest** stdin case.
2. Note **compositor + version**, **scale factor**, and **upstream vs Rust** command lines.
3. File an issue or add to §A.2 checklist with that minimal repro.

---

### A.4 Pre-0.10.0 release checklist

- [ ] §A.2 items relevant to your **0.10.0** scope are done or explicitly deferred with docs.
- [ ] **Cargo.toml `version`** fields updated for the **0.10.x** release (see §A.2 “Versions”).
- [ ] **Migration-plan references** purged from **code** (see §A.2 “Code hygiene”).
- [ ] **Planning archives:** PLAN docs moved off **`main`** per §A.2 “Planning archives”.
- [ ] §A.3 manual matrix passed on **your** target compositor(s).
- [ ] **CI/CD:** **Deploy** workflow tested for this release (triggers, artifacts, permissions).
- [ ] **Shell completions:** shipped or documented for **0.10.0** as decided in §A.2.
- [ ] `CHANGELOG.md` updated; tag **`v0.10.0`** with release notes pointing to this compatibility baseline.

---

## Part B — Pre-1.0.0 (narrow fork)

**Intent:** **`1.0.0`** is **not** upstream parity. It matches **one** usage pattern: **desktop application launcher** (what upstream called **`tofi-drun`**). Everything else is removed or made non-default so the tree stays small and aligned with how you actually use the program (see [`README.md`](../README.md)).

**Prerequisites**

1. Ship and stabilize **`0.10.x`** for anyone who still wants C-like behavior (includes moving PLAN docs off **`main`** — see Part A §A.2 “Planning archives”).

### B.1 Modes and CLI surface

**Single mode**

- [ ] **Desktop run / drun** is the **only** mode: invoking **`tofi`** behaves like upstream **`tofi-drun`** (no separate subcommand or mode flag). Remove **stdin / dmenu-style** launcher, **`tofi-run`**, and any other modes.

**CLI**

- [ ] Expose only **`--config`** and **`--theme`** as **user-facing options** for selecting files (plus the usual **`--help`** / **`--version`** as needed). Both are **optional** when launching: paths can be supplied entirely from disk or overridden per invocation. Remove the rest of the CLI surface that existed for deleted modes or upstream compatibility.

### B.2 Config and theme format

- [ ] Migrate **application config** from the legacy keyfile format to **`config.toml`**. Drop **unused** keys so the schema matches the reduced feature set.
- [ ] Migrate **theme** to **`toml`** in its own file. Parse the **theme document separately** from the config document — **no** `source`-ing theme content inside the config file; the theme file is always loaded and parsed on its own.
- [ ] **Resolution order** (document in **`settings.rs`** and user-facing docs): **`--config`** / **`--theme`** override defaults; then try **default paths** on disk (e.g. XDG **`config.toml`**); **`theme`** in a loaded config points at a theme file when **`--theme`** was not passed.
- [ ] **Compiled-in defaults:** if **no** config path was given or **no file exists** at the default config location, **and** there is **no** usable theme path from CLI, disk, or **`theme`** in config (missing key, bad path, or missing file), **do not fail startup** — apply **default config and theme values defined in code** (minimal built-in TOML-equivalent structs or literals) so the launcher always has a coherent baseline.
- [ ] Centralize **settings resolution** (defaults, file load order, CLI overrides, fallbacks, validation) in **`settings.rs`** (or equivalent single module). Expect **large deletion** of today’s CLI resolver files, config models, and merge logic that only existed for the old surface area.

### B.3 Examples and tests

- [ ] Convert **example configs and themes** to the new **TOML** shapes; remove examples for deleted modes.
- [ ] Update **automated tests** so fixtures and assertions match TOML + single-mode behavior, including **startup with no config/theme files** (compiled-in defaults path); **`cargo test`** (and CI) must pass before tagging **1.0.0**.

### B.4 Cargo features and implementation cleanup

- [ ] **`clipboard`:** keep as a **`[features]`** flag but turn it **off** by default (opt-in). Remove or isolate code paths so default builds do not pull clipboard unless requested.
- [ ] **Hugepages / THP:** remove **documentation** (e.g. README “Bonus Round”) and any **code or hints** tied to transparent huge pages — not a supported knob in 1.0.
- [ ] **`drun` feature:** remove the **`drun`** feature from **`libtofi`** and **`tofi`** `Cargo.toml` — desktop launch is **always** compiled in; there is no alternate “non-drun” binary story via features.
- [ ] **General purge:** delete modules, dependencies, and tests that only served **stdin**, **run**, **upstream CLI parity**, or other patterns you do not use. When in doubt, remove; **1.0** is allowed to be aggressively smaller than **0.10**.

### B.5 Release artifacts for 1.0

- [ ] **Deprecation / migration:** publish a short **0.10.x → 1.0.0** migration note (config/theme TOML examples, removed flags).
- [ ] **Semver:** **`1.0.0`** marks the breaking fork; subsequent **1.x** follows normal semver for API/config you still maintain.
- [ ] **CHANGELOG** and **git tag** `v1.0.0` with the above summarized.

---

## Revision history

| Date | Change |
|------|--------|
| 2026-04-16 | Initial post-migration plan: 0.10 focus (parity, manual steps), 1.0 placeholder |
| 2026-04-16 | Removed benchmark / performance measurement sections |
| 2026-04-16 | Added CI/CD (Deploy job) and shell-completions items for 0.10 |
| 2026-04-16 | Pre-0.10: Cargo versions + purge migration refs from code; Part B: detailed 1.0 (TOML, drun-only, features, docs branch) |
| 2026-04-16 | Planning docs branch moved to pre-0.10; 1.0 theme path rules (`theme` in config when no `--theme`) |
| 2026-04-16 | 1.0: compiled-in defaults when no config/theme files and no `theme` in config |
