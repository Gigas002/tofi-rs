//! XKB keyboard state — keymap, modifier tracking, and key repeat.
//!
//! Wraps `libxkbcommon` via the [`xkbcommon`] crate.  The keymap string is
//! received from the compositor via the `wl_keyboard::keymap` event.

use std::time::{Duration, Instant};

use xkbcommon::xkb::keysyms;
use xkbcommon::xkb::{self, Keysym};

use super::{
    KEY_B, KEY_BACKSPACE, KEY_C, KEY_DOWN, KEY_ENTER, KEY_ESC, KEY_F, KEY_G, KEY_H, KEY_HOME,
    KEY_J, KEY_K, KEY_KPENTER, KEY_L, KEY_LEFT, KEY_LEFTBRACE, KEY_M, KEY_N, KEY_P, KEY_PAGEDOWN,
    KEY_PAGEUP, KEY_RIGHT, KEY_TAB, KEY_U, KEY_UP, KEY_V, KEY_W,
};

// ── RepeatInfo ────────────────────────────────────────────────────────────────

/// Per-key repeat tracking.
#[derive(Debug)]
pub struct RepeatInfo {
    /// Repeat rate in events per second; 0 = repeat disabled.
    pub rate: i32,
    /// Initial delay before the first repeat event, in milliseconds.
    pub delay_ms: i32,
    /// `true` while a key is held and repeat is armed.
    pub active: bool,
    /// XKB keycode of the key being repeated (`evdev_key + 8`).
    pub keycode: u32,
    /// Monotonic deadline for the next repeat event.
    pub next: Instant,
}

impl Default for RepeatInfo {
    fn default() -> Self {
        Self {
            rate: 0,
            delay_ms: 200,
            active: false,
            keycode: 0,
            next: Instant::now(),
        }
    }
}

impl RepeatInfo {
    /// Returns the timeout until the next repeat event, or `None` if no repeat
    /// is scheduled.  A `Duration::ZERO` means the repeat is overdue.
    pub fn timeout(&self) -> Option<Duration> {
        if !self.active || self.rate == 0 {
            return None;
        }
        let now = Instant::now();
        Some(self.next.saturating_duration_since(now))
    }
}

// ── KeyboardState ─────────────────────────────────────────────────────────────

/// XKB keyboard context, keymap, and modifier state.
pub struct KeyboardState {
    context: xkb::Context,
    keymap: Option<xkb::Keymap>,
    state: Option<xkb::State>,
    /// Key repeat timing and arming state.
    pub repeat: RepeatInfo,
    /// If `true`, shortcuts use physical key positions (layout-independent).
    pub physical_keybindings: bool,
}

impl KeyboardState {
    /// Create a new keyboard state with the given `physical_keybindings` mode.
    ///
    /// Panics if `libxkbcommon` cannot create a context (same as C's abort).
    pub fn new(physical_keybindings: bool) -> Self {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        tracing::debug!("KeyboardState: XKB context created");
        Self {
            context,
            keymap: None,
            state: None,
            repeat: RepeatInfo::default(),
            physical_keybindings,
        }
    }

    /// Parse and install a keymap from its XKB text representation.
    ///
    /// Called from the `wl_keyboard::keymap` event handler.
    pub fn load_keymap(&mut self, keymap_str: &str) {
        let Some(km) = xkb::Keymap::new_from_string(
            &self.context,
            keymap_str.to_owned(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            tracing::error!("KeyboardState: failed to compile keymap");
            return;
        };
        let st = xkb::State::new(&km);
        self.keymap = Some(km);
        self.state = Some(st);
        tracing::debug!("KeyboardState: keymap loaded");
    }

    /// Update the modifier state from a `wl_keyboard::modifiers` event.
    pub fn update_modifiers(
        &mut self,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        if let Some(st) = &mut self.state {
            st.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
        }
    }

    /// Record the compositor's key-repeat settings.
    pub fn set_repeat_info(&mut self, rate: i32, delay_ms: i32) {
        self.repeat.rate = rate;
        self.repeat.delay_ms = delay_ms;
        if rate > 0 {
            tracing::debug!(
                "Key repeat: rate={rate}/s, delay={delay_ms}ms (every {}ms)",
                1000 / rate
            );
        } else {
            tracing::debug!("Key repeat: disabled");
        }
    }

    /// Returns `true` if `keycode` is configured to repeat in the current keymap.
    pub fn key_repeats(&self, keycode: u32) -> bool {
        self.keymap
            .as_ref()
            .is_some_and(|km| km.key_repeats(keycode.into()))
    }

    /// Returns `true` if the XKB state is ready (keymap successfully loaded).
    pub fn is_ready(&self) -> bool {
        self.state.is_some()
    }

    /// Get the Unicode codepoint produced by `keycode` in the current state.
    ///
    /// Returns `0` when the state is not initialised or the key maps to no
    /// character.
    pub fn key_get_utf32(&self, keycode: u32) -> u32 {
        self.state
            .as_ref()
            .map(|s| s.key_get_utf32(keycode.into()))
            .unwrap_or(0)
    }

    /// Returns `true` if Ctrl is currently held.
    pub fn mod_ctrl(&self) -> bool {
        self.mod_active(xkb::MOD_NAME_CTRL)
    }

    /// Returns `true` if Alt is currently held.
    pub fn mod_alt(&self) -> bool {
        self.mod_active(xkb::MOD_NAME_ALT)
    }

    /// Returns `true` if Shift is currently held.
    pub fn mod_shift(&self) -> bool {
        self.mod_active(xkb::MOD_NAME_SHIFT)
    }

    fn mod_active(&self, name: &str) -> bool {
        self.state
            .as_ref()
            .is_some_and(|s| s.mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE))
    }

    /// Resolve `keycode` to a Linux evdev key code for shortcut matching.
    ///
    /// With `physical_keybindings = true` (the default): `keycode − 8`
    /// (layout-independent physical positions).  With `false`: the keysym is
    /// resolved first, then mapped to a Linux key code, so shortcuts track the
    /// active layout.
    pub fn keycode_to_linux_key(&self, keycode: u32) -> u32 {
        if self.physical_keybindings {
            keycode - 8
        } else if let Some(st) = &self.state {
            keysym_to_linux_key(st.key_get_one_sym(keycode.into()))
        } else {
            keycode - 8
        }
    }

    // ── Repeat arming ─────────────────────────────────────────────────────────

    /// Arm key repeat for `keycode` (called on key-press when the key repeats
    /// and the repeat rate is non-zero).
    pub fn arm_repeat(&mut self, keycode: u32) {
        self.repeat.active = true;
        self.repeat.keycode = keycode;
        self.repeat.next = Instant::now() + Duration::from_millis(self.repeat.delay_ms as u64);
    }

    /// Disarm key repeat (called on key-release for the repeated key).
    pub fn disarm_repeat(&mut self, keycode: u32) {
        if self.repeat.keycode == keycode {
            self.repeat.active = false;
        }
    }

    /// Advance the repeat deadline after a repeat event fires.
    pub fn advance_repeat(&mut self) {
        if self.repeat.rate > 0 {
            self.repeat.next += Duration::from_millis(1000 / self.repeat.rate as u64);
        }
    }
}

// ── keysym_to_linux_key ───────────────────────────────────────────────────────

/// Map an XKB keysym to a Linux evdev key code for non-physical-keybinding
/// shortcut resolution.
///
/// Returns `u32::MAX` for keysyms that are not mapped.
pub fn keysym_to_linux_key(sym: Keysym) -> u32 {
    match sym.raw() {
        keysyms::KEY_BackSpace => KEY_BACKSPACE,
        keysyms::KEY_w => KEY_W,
        keysyms::KEY_u => KEY_U,
        keysyms::KEY_v => KEY_V,
        keysyms::KEY_Left => KEY_LEFT,
        keysyms::KEY_Right => KEY_RIGHT,
        keysyms::KEY_Up => KEY_UP,
        keysyms::KEY_ISO_Left_Tab => KEY_TAB,
        keysyms::KEY_h => KEY_H,
        keysyms::KEY_k => KEY_K,
        keysyms::KEY_p => KEY_P,
        keysyms::KEY_Down => KEY_DOWN,
        keysyms::KEY_Tab => KEY_TAB,
        keysyms::KEY_l => KEY_L,
        keysyms::KEY_j => KEY_J,
        keysyms::KEY_n => KEY_N,
        keysyms::KEY_b => KEY_B,
        keysyms::KEY_f => KEY_F,
        keysyms::KEY_Home => KEY_HOME,
        keysyms::KEY_Page_Up => KEY_PAGEUP,
        keysyms::KEY_Page_Down => KEY_PAGEDOWN,
        keysyms::KEY_Escape => KEY_ESC,
        keysyms::KEY_c => KEY_C,
        keysyms::KEY_bracketleft => KEY_LEFTBRACE,
        keysyms::KEY_g => KEY_G,
        keysyms::KEY_Return => KEY_ENTER,
        keysyms::KEY_KP_Enter => KEY_KPENTER,
        keysyms::KEY_m => KEY_M,
        _ => u32::MAX,
    }
}
