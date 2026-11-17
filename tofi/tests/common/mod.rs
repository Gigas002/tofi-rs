//! A headless sway to run tofi in, and a virtual keyboard to type into it.
//!
//! Each test starts its own sway in a private runtime directory, and runs
//! tofi with private config, cache, state and data directories, so tests run
//! in parallel and never touch the session or the files of whoever runs them.
//! Without `sway` the tests are skipped, unless `TOFI_LIVE_TESTS=1` (as in
//! CI), where that is a failure.

#![allow(dead_code)]

use std::io::Write as _;
use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};
use xkbcommon::xkb;

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 720;

/// Start a [`Sway`] at `scale`, or return from the test when sway is missing.
#[macro_export]
macro_rules! sway_or_skip {
    ($scale:expr) => {
        match common::Sway::start($scale) {
            Some(sway) => sway,
            None => return,
        }
    };
    () => {
        sway_or_skip!("1")
    };
}

/// Poll `f` until it returns `Some`, for up to 10 s.
pub fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

// ── Sway ─────────────────────────────────────────────────────────────────────

pub struct Sway {
    child: Child,
    runtime: tempfile::TempDir,
    display: String,
}

impl Sway {
    /// A headless sway with one `WIDTH`×`HEIGHT` output at `scale`.
    pub fn start(scale: &str) -> Option<Self> {
        let required = std::env::var_os("TOFI_LIVE_TESTS").is_some_and(|v| v == "1");
        if !have("sway") {
            assert!(!required, "TOFI_LIVE_TESTS=1 but sway is missing");
            eprintln!("skipped: sway is needed for live tests");
            return None;
        }
        // Unix socket paths are short; keep the runtime directory in /tmp.
        let runtime = tempfile::Builder::new()
            .prefix("tofi-live")
            .tempdir_in("/tmp")
            .unwrap();
        let conf = runtime.path().join("sway.conf");
        std::fs::write(
            &conf,
            format!(
                "output HEADLESS-1 resolution {WIDTH}x{HEIGHT} position 0 0 scale {scale}\n\
                 xwayland disable\n"
            ),
        )
        .unwrap();
        let log = std::fs::File::create(runtime.path().join("sway.log")).unwrap();
        let child = Command::new("sway")
            .arg("-c")
            .arg(&conf)
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .env_remove("SWAYSOCK")
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .env_remove("NIRI_SOCKET")
            .env("XDG_RUNTIME_DIR", runtime.path())
            .env("WLR_BACKENDS", "headless")
            .env("WLR_RENDERER", "pixman")
            .env("WLR_LIBINPUT_NO_DEVICES", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .expect("spawn sway");
        let mut sway = Self {
            child,
            runtime,
            display: String::new(),
        };
        sway.display = wait_for("sway's Wayland socket", || {
            std::fs::read_dir(sway.runtime.path())
                .ok()?
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .find(|n| n.starts_with("wayland-") && !n.ends_with(".lock"))
        });
        Some(sway)
    }

    pub fn runtime_dir(&self) -> &Path {
        self.runtime.path()
    }

    pub fn connect(&self) -> Connection {
        let path = self.runtime.path().join(&self.display);
        let stream = std::os::unix::net::UnixStream::connect(path).unwrap();
        Connection::from_socket(stream).unwrap()
    }
}

impl Drop for Sway {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ── Virtual keyboard ─────────────────────────────────────────────────────────

#[derive(Default)]
struct KbState {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<ZwpVirtualKeyboardManagerV1>,
}

impl Dispatch<wl_registry::WlRegistry, ()> for KbState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
                "zwp_virtual_keyboard_manager_v1" => {
                    state.manager = Some(registry.bind(name, 1, qh, ()))
                }
                _ => {}
            }
        }
    }
}
delegate_noop!(KbState: ignore wl_seat::WlSeat);
delegate_noop!(KbState: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(KbState: ignore ZwpVirtualKeyboardV1);

const SHIFT: u32 = 1;
const CTRL: u32 = 4;

/// A US-layout keyboard on sway's seat. Create it before starting tofi so
/// the seat already has a keyboard when tofi binds it.
pub struct Keyboard {
    conn: Connection,
    queue: EventQueue<KbState>,
    state: KbState,
    kb: ZwpVirtualKeyboardV1,
    start: Instant,
}

impl Keyboard {
    pub fn new(sway: &Sway) -> Self {
        let conn = sway.connect();
        let mut queue = conn.new_event_queue();
        let qh = queue.handle();
        conn.display().get_registry(&qh, ());
        let mut state = KbState::default();
        queue.roundtrip(&mut state).unwrap();
        let kb = state
            .manager
            .as_ref()
            .expect("sway offers zwp_virtual_keyboard_manager_v1")
            .create_virtual_keyboard(state.seat.as_ref().expect("a seat"), &qh, ());

        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(&ctx, "", "", "us", "", None, 0)
            .expect("compile the us keymap");
        let mut text = keymap
            .get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)
            .into_bytes();
        text.push(0);
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&text).unwrap();
        kb.keymap(xkb::KEYMAP_FORMAT_TEXT_V1, file.as_fd(), text.len() as u32);
        queue.roundtrip(&mut state).unwrap();
        Self {
            conn,
            queue,
            state,
            kb,
            start: Instant::now(),
        }
    }

    fn time(&self) -> u32 {
        self.start.elapsed().as_millis() as u32
    }

    fn tap_code(&mut self, code: u32, mods: u32) {
        if mods != 0 {
            self.kb.modifiers(mods, 0, 0, 0);
        }
        self.kb.key(self.time(), code, 1);
        self.kb.key(self.time(), code, 0);
        if mods != 0 {
            self.kb.modifiers(0, 0, 0, 0);
        }
        self.queue.roundtrip(&mut self.state).unwrap();
        // Give tofi a moment, so key events stay in order with its redraws.
        std::thread::sleep(Duration::from_millis(10));
    }

    /// Press and release the key `name` (an xkb keysym name like `Return`).
    pub fn key(&mut self, name: &str) {
        let code = evdev_for_name(name).unwrap_or_else(|| panic!("no key for {name}"));
        self.tap_code(code, 0);
    }

    /// Press `Ctrl` + `letter`.
    pub fn ctrl(&mut self, letter: char) {
        let (code, _) = evdev_for_char(letter).expect("a letter");
        self.tap_code(code, CTRL);
    }

    /// Type `text` (ASCII only).
    pub fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let (code, shift) = evdev_for_char(c).unwrap_or_else(|| panic!("no key for {c:?}"));
            self.tap_code(code, if shift { SHIFT } else { 0 });
        }
    }

    pub fn flush(&self) {
        self.conn.flush().unwrap();
    }
}

fn evdev_for_name(name: &str) -> Option<u32> {
    Some(match name {
        "Escape" => 1,
        "BackSpace" => 14,
        "Tab" => 15,
        "Return" => 28,
        "space" => 57,
        "Home" => 102,
        "Up" => 103,
        "Page_Up" => 104,
        "Left" => 105,
        "Right" => 106,
        "Down" => 108,
        "Page_Down" => 109,
        _ => return None,
    })
}

/// The evdev code for `c` on a US layout, and whether it needs Shift.
fn evdev_for_char(c: char) -> Option<(u32, bool)> {
    const ROWS: [(&str, &str, u32); 4] = [
        ("1234567890-=", "!@#$%^&*()_+", 2),
        ("qwertyuiop[]", "QWERTYUIOP{}", 16),
        ("asdfghjkl;'", "ASDFGHJKL:\"", 30),
        ("zxcvbnm,./", "ZXCVBNM<>?", 44),
    ];
    if c == ' ' {
        return Some((57, false));
    }
    for (plain, shifted, base) in ROWS {
        if let Some(i) = plain.find(c) {
            return Some((base + i as u32, false));
        }
        if let Some(i) = shifted.find(c) {
            return Some((base + shifted[..i].chars().count() as u32, true));
        }
    }
    None
}

// ── Tofi ─────────────────────────────────────────────────────────────────────

/// Private config, cache, state and data directories for one tofi run.
pub struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        for sub in [
            "config/tofi/themes",
            "cache",
            "state",
            "data/applications",
            "home",
        ] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
        }
        Self { dir }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Write `config.toml`.
    pub fn config(&self, toml: &str) -> &Self {
        std::fs::write(self.path().join("config/tofi/config.toml"), toml).unwrap();
        self
    }

    /// Write `themes/test.toml` and point `config.toml` at it.
    pub fn theme(&self, toml: &str) -> &Self {
        std::fs::write(self.path().join("config/tofi/themes/test.toml"), toml).unwrap();
        self.config("[base]\ntheme = \"test.toml\"\n")
    }

    /// Write a desktop entry into the private `applications` directory.
    pub fn desktop_entry(&self, id: &str, body: &str) -> &Self {
        std::fs::write(
            self.path()
                .join("data/applications")
                .join(format!("{id}.desktop")),
            format!("[Desktop Entry]\nType=Application\n{body}\n"),
        )
        .unwrap();
        self
    }

    pub fn file(&self, rel: &str) -> PathBuf {
        self.path().join(rel)
    }
}

pub struct Tofi {
    child: Child,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl Tofi {
    /// Start tofi with `args`, feeding `stdin`, and wait until it has focus.
    pub fn start(sway: &Sway, home: &Home, args: &[&str], stdin: &str) -> Self {
        let stdout = home.file("stdout.txt");
        let stderr = home.file("stderr.txt");
        let mut child = Command::new(env!("CARGO_BIN_EXE_tofi"))
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            // Under cargo llvm-cov, where to write coverage data.
            .envs(std::env::var_os("LLVM_PROFILE_FILE").map(|v| ("LLVM_PROFILE_FILE", v)))
            .env("XDG_RUNTIME_DIR", sway.runtime_dir())
            .env("WAYLAND_DISPLAY", &sway.display)
            .env("HOME", home.file("home"))
            .env("XDG_CONFIG_HOME", home.file("config"))
            .env("XDG_CACHE_HOME", home.file("cache"))
            .env("XDG_STATE_HOME", home.file("state"))
            .env("XDG_DATA_HOME", home.file("data"))
            .env("XDG_DATA_DIRS", home.file("no-system-data"))
            .env("RUST_LOG", "debug")
            .stdin(Stdio::piped())
            .stdout(std::fs::File::create(&stdout).unwrap())
            .stderr(std::fs::File::create(&stderr).unwrap())
            .spawn()
            .expect("spawn tofi");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        let tofi = Self {
            child,
            stdout,
            stderr,
        };
        wait_for("tofi to get keyboard focus", || {
            tofi.stderr().contains("wl_keyboard: Enter").then_some(())
        });
        tofi
    }

    pub fn stdout(&self) -> String {
        std::fs::read_to_string(&self.stdout).unwrap_or_default()
    }

    pub fn stderr(&self) -> String {
        std::fs::read_to_string(&self.stderr).unwrap_or_default()
    }

    /// Wait for tofi to exit; panics if it takes more than 10 s.
    pub fn wait(&mut self) -> ExitStatus {
        let child = &mut self.child;
        wait_for("tofi to exit", || child.try_wait().unwrap())
    }

    /// Wait until stderr contains `needle`.
    pub fn wait_for_log(&self, needle: &str) {
        wait_for(needle, || self.stderr().contains(needle).then_some(()));
    }
}

impl Drop for Tofi {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
