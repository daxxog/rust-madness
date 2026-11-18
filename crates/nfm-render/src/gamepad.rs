//! `gamepad` — the gilrs adapter: a Steam Deck pad feeding the SAME Java
//! `Event` code edges the keyboard path produces.
//!
//! The keyboard path is [`crate::window::key_edges`] over the `WATCH` table:
//! one `KeyDown(code)` on a control's down edge, one `KeyUp(code)` on its
//! release, and **nothing while it is held** (no repeats — the shell's
//! `key_down` toggles on every call, so a repeat would double-fire
//! `arrace`/`mutem`/`mutes`/`view`). This adapter folds every pad control
//! into the same `WATCH` rows and runs the SAME edge detector over them, so
//! `nfm_core`'s handlers cannot tell the two sources apart — core needs no
//! change at all.
//!
//! Locked Deck mapping (user-established; do not redesign):
//!
//! Face-button legend — gilrs names are **positional**, the Deck's labels are
//! **Xbox-style**, and the two do not line up: **A = `South` (bottom),
//! B = `East` (right), X = `West` (left), Y = `North` (top)**. Read the
//! table by physical label; mixing a positional name with a printed label is
//! what bound brake to the wrong button in D2b.
//!
//! | pad                          | game concept                  | code |
//! |------------------------------|-------------------------------|------|
//! | RT (`RightZ` / `RightTrigger2`) | accelerate                 | 1004 |
//! | X (left, `West`)             | brake / reverse               | 1005 |
//! | LT (`LeftZ` / `LeftTrigger2`) | handbrake                    | 32   |
//! | left stick X + D-pad L/R     | steer (binary, hysteresis)    | 1006 / 1007 |
//! | A (bottom, `South`)          | confirm / enter (every screen)| 10   |
//! | Y (top, `North`)             | arrow-switch (`arrace`) toggle| 97   |
//! | B (right, `East`)            | unmapped (free)               | —    |
//! | Start (`Menu`/`Start`)       | always enter (pause in race)  | 10   |
//! | left stick Y                 | in race: unmapped             | —    |
//! |                              | elsewhere: menu up / down     | 1004 / 1005 |
//! | D-pad up                     | in race: camera view cycle    | 118  |
//! |                              | elsewhere: menu up            | 1004 |
//! | D-pad down                   | in race: unmapped             | —    |
//! |                              | elsewhere: menu down          | 1005 |
//! | LB (`LeftTrigger`) / L4 / L5 | music mute                    | 109  |
//! | RB (`RightTrigger`) / R4 / R5| sound-effects mute            | 110  |
//!
//! The face buttons are not screen-routed at all: A is `enter` and Y is the
//! keyboard's `a` key on every screen, so whether `arrace` does anything in
//! a menu is the shell's business, not the adapter's.
//!
//! The vertical d-pad route is per press: its target is latched at press
//! time so a screen change while it is held releases the slot it pressed,
//! never a second one. The race frame (`fase == 0`) keeps the pre-D3 mapping
//! for the vertical controls exactly; the stick Y and D-pad down are
//! unmapped there.
//!
//! The rear paddles (L4/L5/R4/R5) stay mapped, but on the Deck Steam Input
//! swallows them (`gilrs` sees only "Steam Virtual Gamepad"; the presses
//! produce no events), so the **bumpers are their stand-in**, the same
//! handedness: LB → music mute, RB → sfx mute. Bumpers are unconditioned
//! (the mutes are global toggles). In gilrs the bumpers are
//! `Button::LeftTrigger`/`RightTrigger` — the evdev DB maps `BTN_TL`/
//! `BTN_TR` to `"leftshoulder"`/`"rightshoulder"`, and the ANALOG triggers
//! are `LeftTrigger2`/`RightTrigger2` (`BTN_TL2`/`BTN_TR2`,
//! `"lefttrigger"`/`"righttrigger"`), which is why RT rides `RightZ` plus
//! `RightTrigger2`.
//!
//! One host-side route: at fase 111 (the click-to-start boot screen, a
//! mouse gate — `Shell::mouse_down` sets `mouses` and the fase-111 block
//! only tests that) both A and Start synthesize a `MouseDown` at the
//! click panel's centre, (275, 355). It is input adaptation for the
//! Deck's missing pointer, not a fidelity change; it ships in every
//! build (a pure-pad boot is the Deck's production entry).
//!
//! Menus are driven by the Up/Down cursor codes (1004/1005), the same
//! `WATCH` rows the race throttle/brake use, so the stick Y and the D-pad
//! reach them with no core change. The shared codes keep separate
//! down-states (`Accel`/`Brake` vs `MenuUp`/`MenuDown`), so a release of
//! one never releases the other.
//!
//! Only play mode constructs [`Gamepad`] (see `main.rs::play_mode`); every
//! headless mode and the mirror suites stay input-free and deterministic.
//! Construction failure (no `/dev/input` access, no udev, no device) is one
//! log line and `None` — keyboard-only play, never a panic.

use gilrs::{Axis, Button, EventType, Gilrs, GilrsBuilder};
use nfm_core::host::InputEvent;

use crate::window::{self, NKEYS};

// --- the retarget tables ---------------------------------------------------
//
// Everything the D3 on-Deck probe can change is a single row in one of the
// three tables below. Rows are (source → control); the control's Java code
// lives in `SLOTS`, so a retarget never touches the edge machinery.

/// gilrs' named buttons → pad controls, one row per physical control the
/// keyboard has a distinct action for. Face buttons are listed by their
/// POSITIONAL gilrs name with the printed Deck label beside it (A = South
/// bottom, B = East right, X = West left, Y = North top). Deliberately
/// unmapped (no invented bindings — the keyboard has no separate action for
/// them): East/B, the stick clicks L3/R3, Select/View, Mode/Steam (the only
/// residual `WATCH` codes are `enter` aliases — 27 and 112 — and the view
/// cycle, which is D-pad up in the race only, so it is routed in
/// [`PadState::dpad_up`] rather than from this table).
///
/// In gilrs the BUMPERS are `LeftTrigger`/`RightTrigger` (evdev `BTN_TL`/
/// `BTN_TR` → SDL `"leftshoulder"`/`"rightshoulder"`); the analog triggers
/// are the `*Trigger2` rows. The bumpers stand in for the Deck's
/// Steam-Input-swallowed rear paddles: same handedness as L4/L5/R4/R5.
const BUTTONS: &[(Button, Slot)] = &[
    (Button::North, Slot::ARace),         // Y (top) — arrow-switch toggle
    (Button::West, Slot::Brake),          // X (left) — brake / reverse
    (Button::South, Slot::Enter),         // A (bottom) — confirm / enter
    (Button::Start, Slot::Start),         // Menu/Start — always enter
    (Button::DPadLeft, Slot::Left),       // steer
    (Button::DPadRight, Slot::Right),     // steer
    (Button::RightTrigger2, Slot::Accel), // RT digital fallback (RightZ is analog)
    (Button::LeftTrigger2, Slot::Handb),  // LT digital fallback (LeftZ is analog)
    (Button::LeftTrigger, Slot::Music1),  // LB — music mute (rear-paddle stand-in)
    (Button::RightTrigger, Slot::Sfx1),   // RB — sfx mute (rear-paddle stand-in)
];

/// The rear paddles, by **raw evdev KEY code**: gilrs has no named `Button`
/// for `BTN_TRIGGER_HAPPY*`, so a driver that exposes them delivers
/// `Button::Unknown` and keeps the raw code (D1's probe read that path).
/// 0x2c0..0x2c3 = `BTN_TRIGGER_HAPPY1..4`; which pad that is (L4/L5/R4/R5)
/// is what D3's on-Deck probe settles — retarget a row here.
const REAR: &[(u16, Slot)] = &[
    (0x2c0, Slot::Music1), // L4 → music mute
    (0x2c1, Slot::Music2), // L5 → music mute
    (0x2c2, Slot::Sfx1),   // R4 → sound-effects mute
    (0x2c3, Slot::Sfx2),   // R5 → sound-effects mute
];

/// Analog axes → the raw readings [`PadState::sample_axes`] folds. The
/// trigger axes are the real analog path the D1 probe saw (`LeftZ`/`RightZ`,
/// 0..1); the hat axes are the fallback for a driver that reports the D-pad
/// as `DPadX/DPadY` instead of synthesizing `DPad*` buttons (gilrs normally
/// does the latter, and then [`PadState::dpad_buttons`] ignores the hat so
/// the two paths cannot disagree); `LeftStickY` is the vertical stick.
///
/// gilrs normalizes every axis to −1..1, and on Y axes it negates the raw
/// evdev value (`gilrs-core` `IS_Y_AXIS_REVERSED = true` on Linux), so
/// **+1 is up** for `LeftStickY` and `DPadY` on every platform. That is
/// also what the hat→button filter assumes (`DPadY == 1.0` → `DPadUp`).
const ACTIONS: &[(Axis, AxisTarget)] = &[
    (Axis::LeftStickX, AxisTarget::Steer),
    (Axis::DPadX, AxisTarget::Steer),
    (Axis::LeftStickY, AxisTarget::StickY),
    (Axis::RightZ, AxisTarget::Accel),
    (Axis::LeftZ, AxisTarget::Handb),
    (Axis::DPadY, AxisTarget::HatY),
];

/// Every control the pad can hold, with the Java `Event` code it emits. All
/// of them are `window::WATCH` rows, so the keyboard's edge detector applies
/// unchanged (a test pins that). Two controls may share a code — Start and A
/// both emit `enter`, MenuUp shares 1004 with Accel, MenuDown shares 1005
/// with Brake — but each keeps its OWN down-state, so releasing one never
/// releases the other.
///
/// The declaration order of [`Slot`] must match this table (pinned by test).
const SLOTS: [(Slot, i32); 15] = [
    (Slot::Accel, 1004),    // up / throttle
    (Slot::Brake, 1005),    // down / brake-reverse
    (Slot::Left, 1006),     // left
    (Slot::Right, 1007),    // right
    (Slot::Handb, 32),      // space / handbrake
    (Slot::Start, 10),      // enter (Start; pause in the race)
    (Slot::ARace, 97),      // A key / arrace toggle (pad Y)
    (Slot::Enter, 10),      // enter (pad A, every screen)
    (Slot::Music1, 109),    // M / music mute
    (Slot::Music2, 109),    // M / music mute (second rear button)
    (Slot::Sfx1, 110),      // N / sound-effects mute
    (Slot::Sfx2, 110),      // N / sound-effects mute (second rear button)
    (Slot::View, 118),      // V / camera view cycle
    (Slot::MenuUp, 1004),   // up cursor (stick Y / D-pad up outside the race)
    (Slot::MenuDown, 1005), // down cursor (stick Y / D-pad down outside the race)
];

/// Steer/trigger/hat hysteresis: engage at `_HI`, release only below `_LO`,
/// so a resting stick (a Deck's centres around ±0.05..0.2) never chatters
/// the steer edges.
const STICK_HI: f32 = 0.5;
const STICK_LO: f32 = 0.4;
const TRIG_HI: f32 = 0.5;
const TRIG_LO: f32 = 0.4;
const HAT_HI: f32 = 0.5;
const HAT_LO: f32 = 0.4;

/// The fase-111 click-to-start screen (the shell's boot mouse gate).
const BOOT_FASE: i32 = 111;

/// The synthesized click point: the centre of the clicknow panel
/// (`xtGraphics.clicknow` paints 125,315 300x80). The gate ignores the
/// position — `Shell::mouse_down` stores it and the fase-111 block only
/// tests `mouses == 1` — but the point is the button's own middle.
const BOOT_CLICK_X: i32 = 275;
const BOOT_CLICK_Y: i32 = 355;

/// One physical control's slot. `as usize` indexes [`SLOTS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Slot {
    Accel,
    Brake,
    Left,
    Right,
    Handb,
    Start,
    ARace,
    Enter,
    Music1,
    Music2,
    Sfx1,
    Sfx2,
    View,
    MenuUp,
    MenuDown,
}

/// What an analog axis drives.
#[derive(Clone, Copy)]
enum AxisTarget {
    /// Signed steer: negative value = left, positive = right.
    Steer,
    /// 0..1 trigger travel.
    Accel,
    Handb,
    /// Signed left-stick Y: positive = up (gilrs normalizes Y).
    StickY,
    /// Signed hat Y: positive = up (same normalization).
    HatY,
}

/// The analog inputs' last reported values, folded into [`PadState::axis`]
/// once per frame (edges are recomputed from the current value, so a held
/// position is a held key and only a threshold crossing emits).
#[derive(Default, Clone, Copy)]
struct Axes {
    stick_x: f32,
    stick_y: f32,
    hat_x: f32,
    hat_y: f32,
    left_z: f32,
    right_z: f32,
}

/// The pure pad → keyboard-edge folder: no gilrs types in its state, so the
/// whole contract is testable without a device or `/dev/input`.
pub struct PadState {
    /// Controls held by a digital button (or trigger/hat axis).
    btn: [bool; SLOTS.len()],
    /// Controls held by an analog axis past its threshold.
    axis: [bool; SLOTS.len()],
    /// The `WATCH`-shaped down-state the last edge pass saw — exactly the
    /// keyboard detector's `prev`.
    prev: [bool; NKEYS],
    /// D-pad up/down's route for the current press, latched the same way:
    /// the race frame maps up to the view cycle and everything else to the
    /// menu cursor, so a screen change while held must not re-route.
    up_route: Option<usize>,
    down_route: Option<usize>,
    /// The current screen (the shell's `xt.fase`), refreshed by [`Gamepad::poll`]
    /// before the frame's edge pass: the vertical axes are routed by it.
    fase: i32,
    /// This pad reports its D-pad as `DPad*` buttons, so the `DPadY` axis is
    /// redundant and is not folded (the latched button route is authoritative).
    dpad_buttons: bool,
    analog: Axes,
    /// A/Start held on the fase-111 boot screen: the press is latched so
    /// the release is consumed on whatever screen it lands on.
    boot_held: bool,
    /// A boot-screen click edge is waiting to be emitted.
    boot_armed: bool,
}

impl Default for PadState {
    fn default() -> Self {
        Self::new()
    }
}

impl PadState {
    pub fn new() -> Self {
        PadState {
            btn: [false; SLOTS.len()],
            axis: [false; SLOTS.len()],
            prev: [false; NKEYS],
            up_route: None,
            down_route: None,
            fase: 0,
            dpad_buttons: false,
            analog: Axes::default(),
            boot_held: false,
            boot_armed: false,
        }
    }

    /// Report the frame's screen before the edge pass. [`Gamepad::poll`] sets
    /// it from the shell's `xt.fase`; the tests set it directly.
    fn set_fase(&mut self, fase: i32) {
        self.fase = fase;
    }

    /// A digital edge on a control (press or release).
    fn set(&mut self, slot: Slot, down: bool) {
        self.btn[slot as usize] = down;
    }

    /// D-pad up: the camera view cycle (118) in the race frame, the menu
    /// cursor up (1004) everywhere else. The target is latched at press time
    /// so a screen change while the button is down releases the slot it
    /// pressed — never a second one.
    fn dpad_up(&mut self, down: bool, fase: i32) {
        if down {
            if self.up_route.is_none() {
                let slot = if fase == 0 { Slot::View } else { Slot::MenuUp };
                self.up_route = Some(slot as usize);
                self.btn[slot as usize] = true;
            }
        } else if let Some(row) = self.up_route.take() {
            self.btn[row] = false;
        }
    }

    /// D-pad down: the menu cursor down (1005) outside the race; unmapped in
    /// the race frame (the camera has no down control), as before. Latched
    /// like [`Self::dpad_up`]: a race-frame press still owns its release.
    fn dpad_down(&mut self, down: bool, fase: i32) {
        if down {
            if self.down_route.is_none() {
                self.down_route = Some(Slot::MenuDown as usize);
                if fase != 0 {
                    self.btn[Slot::MenuDown as usize] = true;
                }
            }
        } else if let Some(row) = self.down_route.take() {
            self.btn[row] = false;
        }
    }

    /// Note that this pad delivers its D-pad as `DPad*` buttons. The `DPadY`
    /// axis then only repeats what the buttons say, so it is not folded —
    /// the latched button route stays the single authority across a screen
    /// change. (A driver that reports the hat as an axis never sets this.)
    fn dpad_button_seen(&mut self) {
        self.dpad_buttons = true;
    }

    /// The fase-111 (click-to-start) route. The boot screen is a mouse
    /// gate (`Shell::mouse_down` arms `mouses`; the fase-111 block only
    /// tests `mouses == 1`), so A and Start synthesize the applet's click
    /// instead of a key edge. Returns true when this route consumed the
    /// event.
    ///
    /// The press is latched (`boot_held`) so its release is consumed on
    /// whatever screen it lands on — otherwise a release after the fade
    /// started would fall through to a menu route that never saw the press.
    fn boot_route(&mut self, down: bool, fase: i32) -> bool {
        if down {
            if fase != BOOT_FASE {
                return false;
            }
            if !self.boot_held {
                self.boot_held = true;
                self.boot_armed = true;
            }
            true
        } else if self.boot_held {
            self.boot_held = false;
            true
        } else {
            false
        }
    }

    /// Emit the armed boot-screen click once (its `KeyUp` equivalent is
    /// silence: the shell's mouse state is one-shot).
    fn take_boot_click(&mut self, out: &mut Vec<InputEvent>) {
        if self.boot_armed {
            self.boot_armed = false;
            out.push(InputEvent::MouseDown(BOOT_CLICK_X, BOOT_CLICK_Y));
        }
    }

    /// Store one axis reading (the frame's edge pass reads them back).
    /// gilrs normalizes every axis to −1..1 (`gamepad.rs::axis_value`), so a
    /// trigger at rest reads −1 and full travel +1: fold it into 0..1 travel
    /// first, or the engage threshold would sit at 75% travel.
    fn axis_value(&mut self, axis: Axis, value: f32) {
        if let Some((_, target)) = ACTIONS.iter().find(|(a, _)| *a == axis) {
            match target {
                AxisTarget::Steer => {
                    if axis == Axis::DPadX {
                        self.analog.hat_x = value;
                    } else {
                        self.analog.stick_x = value;
                    }
                }
                AxisTarget::Accel => self.analog.right_z = travel(value),
                AxisTarget::Handb => self.analog.left_z = travel(value),
                AxisTarget::StickY => self.analog.stick_y = value,
                AxisTarget::HatY => self.analog.hat_y = value,
            }
        }
    }

    /// Fold the analog readings into the thresholded controls. The stick and
    /// the hat share the steer rows; the larger magnitude wins so a held
    /// D-pad direction is not cancelled by a centred stick.
    ///
    /// The vertical fold is screen-dependent. In the race frame (`fase == 0`)
    /// the mapping is the pre-D3 one: the hat drives the camera view cycle and
    /// the stick Y is unmapped. Everywhere else the stick Y and the hat fold
    /// into the menu cursor rows (larger magnitude wins, like steer), with the
    /// same 0.5/0.4 hysteresis as stick X. gilrs normalizes Y so **+1 is up**
    /// (see [`ACTIONS`]), hence the positive side is the up row.
    fn sample_axes(&mut self) {
        let steer = if self.analog.hat_x.abs() > self.analog.stick_x.abs() {
            self.analog.hat_x
        } else {
            self.analog.stick_x
        };
        self.axis[Slot::Left as usize] =
            schmitt(self.axis[Slot::Left as usize], -steer, STICK_HI, STICK_LO);
        self.axis[Slot::Right as usize] =
            schmitt(self.axis[Slot::Right as usize], steer, STICK_HI, STICK_LO);
        self.axis[Slot::Accel as usize] = schmitt(
            self.axis[Slot::Accel as usize],
            self.analog.right_z,
            TRIG_HI,
            TRIG_LO,
        );
        self.axis[Slot::Handb as usize] = schmitt(
            self.axis[Slot::Handb as usize],
            self.analog.left_z,
            TRIG_HI,
            TRIG_LO,
        );

        // A pad with D-pad buttons drives the vertical route itself, so its
        // hat axis is dropped here rather than racing the latched button.
        let hat_y = if self.dpad_buttons {
            0.0
        } else {
            self.analog.hat_y
        };
        let (up, down, view) = if self.fase == 0 {
            (0.0, 0.0, hat_y)
        } else {
            let vertical = if hat_y.abs() > self.analog.stick_y.abs() {
                hat_y
            } else {
                self.analog.stick_y
            };
            (vertical, -vertical, 0.0)
        };
        self.axis[Slot::MenuUp as usize] =
            schmitt(self.axis[Slot::MenuUp as usize], up, STICK_HI, STICK_LO);
        self.axis[Slot::MenuDown as usize] =
            schmitt(self.axis[Slot::MenuDown as usize], down, STICK_HI, STICK_LO);
        self.axis[Slot::View as usize] =
            schmitt(self.axis[Slot::View as usize], view, HAT_HI, HAT_LO);
    }

    /// Forget every held control and report the resulting up-edges — the
    /// disconnect path, so a car can never keep the throttle held.
    pub fn release_all(&mut self) -> Vec<InputEvent> {
        self.btn = [false; SLOTS.len()];
        self.axis = [false; SLOTS.len()];
        self.up_route = None;
        self.down_route = None;
        self.analog = Axes::default();
        self.boot_held = false;
        self.boot_armed = false;
        self.edges()
    }

    /// One frame's edges: fold the controls into the `WATCH`-shaped
    /// down-state, then run the keyboard's own detector over it.
    pub fn edges(&mut self) -> Vec<InputEvent> {
        self.sample_axes();
        let mut pressed = [false; NKEYS];
        for (i, &(_, code)) in SLOTS.iter().enumerate() {
            if !(self.btn[i] || self.axis[i]) {
                continue;
            }
            if let Some(row) = window::watch_index(code) {
                pressed[row] = true;
            }
        }
        let mut out = window::key_edges(&mut self.prev, pressed);
        self.take_boot_click(&mut out);
        out
    }
}

/// Hysteresis: `held` stays down until the value drops below `lo`; an idle
/// control engages only at `hi`. Hats are exact ±1, triggers 0..1 after
/// [`travel`], stick X is negative-left.
fn schmitt(held: bool, value: f32, hi: f32, lo: f32) -> bool {
    if held { value >= lo } else { value >= hi }
}

/// gilrs' −1..1 axis reading → 0..1 trigger travel (rest −1 → 0, full +1 → 1).
fn travel(value: f32) -> f32 {
    (value + 1.0) * 0.5
}

/// The live adapter: gilrs plus the pure folder. `None` from [`Gamepad::open`]
/// means keyboard-only play.
pub struct Gamepad {
    gilrs: Gilrs,
    state: PadState,
}

impl Gamepad {
    /// Open gilrs. A failure (unreadable `/dev/input`, no udev, no device) is
    /// one log line and `None`; opening never panics.
    pub fn open() -> Option<Gamepad> {
        match GilrsBuilder::new().with_force_feedback(false).build() {
            Ok(gilrs) => {
                let pads = gilrs.gamepads().count();
                crate::logger::line(&format!(
                    "gamepad: gilrs ready, {pads} pad{}",
                    if pads == 1 { "" } else { "s" }
                ));
                Some(Gamepad {
                    gilrs,
                    state: PadState::new(),
                })
            }
            Err(e) => {
                crate::logger::error(&format!("gamepad: unavailable ({e}); keyboard only"));
                None
            }
        }
    }

    /// The known-device inventory (`id: name`), names only — the debug
    /// server's `/env` summary. Cheap; no device state is touched.
    #[cfg(debug_assertions)]
    pub fn names(&self) -> Vec<String> {
        self.gilrs
            .gamepads()
            .map(|(id, pad)| format!("{}: {}", usize::from(id), pad.name()))
            .collect()
    }

    /// Drain this frame's events and return the code edges they made. `fase`
    /// is the current screen (the shell's `xt.fase`), which the boot gate and
    /// the vertical controls need.
    pub fn poll(&mut self, fase: i32) -> Vec<InputEvent> {
        self.state.set_fase(fase);
        let mut evs = Vec::new();
        while let Some(ev) = self.gilrs.next_event() {
            self.handle(ev, fase, &mut evs);
        }
        evs.extend(self.state.edges());
        evs
    }

    fn handle(&mut self, ev: gilrs::Event, fase: i32, out: &mut Vec<InputEvent>) {
        match ev.event {
            EventType::ButtonPressed(button, code) => self.down(button, code, true, fase),
            EventType::ButtonReleased(button, code) => self.down(button, code, false, fase),
            // The keyboard contract has no repeat arm (`key_edges` emits on
            // state change only) and the shell's toggles would double-fire,
            // so a pad-held button re-emits nothing either.
            EventType::ButtonRepeated(_, _) => {}
            // Trigger pressure also arrives as `ButtonChanged`; the analog
            // path above is the one that decides, so this is ignored.
            EventType::ButtonChanged(_, _, _) => {}
            EventType::AxisChanged(axis, value, _) => self.state.axis_value(axis, value),
            EventType::Connected => {
                let name = self
                    .gilrs
                    .connected_gamepad(ev.id)
                    .map_or_else(|| "unknown".to_string(), |g| g.name().to_string());
                crate::logger::line(&format!(
                    "gamepad: connected {name} (id {})",
                    usize::from(ev.id)
                ));
            }
            EventType::Disconnected => {
                crate::logger::line(&format!("gamepad: disconnected id {}", usize::from(ev.id)));
                // Release everything so no control stays latched.
                out.extend(self.state.release_all());
            }
            _ => {}
        }
    }

    /// One digital edge: the fase-111 boot gate first, then the fase-gated
    /// D-pad up/down routes, then the named button table, then the raw
    /// rear-paddle codes. Every face button is a plain table row.
    fn down(&mut self, button: Button, code: gilrs::ev::Code, down: bool, fase: i32) {
        // fase 111 is the applet's click-to-start mouse gate, so A and
        // Start synthesize its MouseDown instead of a key edge.
        if (button == Button::South || button == Button::Start) && self.state.boot_route(down, fase)
        {
            return;
        }
        // The D-pad's vertical half is context-dependent (view in the race,
        // cursor elsewhere), so it is routed here rather than from BUTTONS.
        match button {
            Button::DPadUp => {
                self.state.dpad_button_seen();
                self.state.dpad_up(down, fase);
                return;
            }
            Button::DPadDown => {
                self.state.dpad_button_seen();
                self.state.dpad_down(down, fase);
                return;
            }
            _ => {}
        }
        if let Some(slot) = BUTTONS.iter().find(|(b, _)| *b == button).map(|&(_, s)| s) {
            self.state.set(slot, down);
            return;
        }
        if let Some(slot) = rear_slot(code.into_u32()) {
            self.state.set(slot, down);
        }
    }
}

/// Resolve a raw packed evdev code (gilrs' `Code`, `kind << 16 | code`) to a
/// rear-paddle control. Only `EV_KEY` (kind 1) rows are in [`REAR`].
fn rear_slot(packed: u32) -> Option<Slot> {
    const EV_KEY: u32 = 1;
    if packed >> 16 != EV_KEY {
        return None;
    }
    let code = (packed & 0xffff) as u16;
    REAR.iter().find(|(c, _)| *c == code).map(|&(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every control's code is a `WATCH` row (so the keyboard detector is
    /// the whole contract) and the slot order matches the table.
    #[test]
    fn slot_table_is_the_watch_table() {
        for (i, &(slot, code)) in SLOTS.iter().enumerate() {
            assert_eq!(slot as usize, i, "slot order matches SLOTS");
            assert!(
                window::watch_index(code).is_some(),
                "{slot:?} → {code} is not a WATCH row"
            );
        }
        // The codes the locked mapping promises, spelled out.
        let code_of = |s: Slot| SLOTS.iter().find(|(x, _)| *x == s).unwrap().1;
        assert_eq!(code_of(Slot::Accel), 1004);
        assert_eq!(code_of(Slot::Brake), 1005);
        assert_eq!(code_of(Slot::Handb), 32);
        assert_eq!(code_of(Slot::Left), 1006);
        assert_eq!(code_of(Slot::Right), 1007);
        assert_eq!(code_of(Slot::Start), 10);
        assert_eq!(code_of(Slot::ARace), 97);
        assert_eq!(code_of(Slot::Enter), 10);
        assert_eq!(code_of(Slot::View), 118);
        assert_eq!(code_of(Slot::Music1), 109);
        assert_eq!(code_of(Slot::Sfx1), 110);
        assert_eq!(
            code_of(Slot::MenuUp),
            1004,
            "cursor up shares the throttle row"
        );
        assert_eq!(
            code_of(Slot::MenuDown),
            1005,
            "cursor down shares the brake row"
        );
    }

    /// A button press is one KeyDown, the hold is silent (no repeats — the
    /// shell's toggles must not double-fire), the release one KeyUp.
    #[test]
    fn press_hold_release_is_one_edge_each() {
        let mut p = PadState::new();
        p.set(Slot::Accel, true);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1004)]);
        for _ in 0..8 {
            assert_eq!(p.edges(), vec![], "hold is silent");
        }
        p.set(Slot::Accel, false);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1004)]);
        assert_eq!(p.edges(), vec![]);
    }

    /// The face buttons by physical label: A (`South`, bottom) is confirm
    /// (10) everywhere, Y (`North`, top) is the keyboard's `a` (97), X
    /// (`West`, left) is brake (1005), and B (`East`, right) stays free.
    /// None of them is screen-routed: the same codes in the race and in a
    /// menu (`arrace` being inert outside the race is the shell's business).
    #[test]
    fn face_buttons_follow_the_xbox_labels() {
        let slot_of = |b: Button| BUTTONS.iter().find(|(x, _)| *x == b).map(|&(_, s)| s);
        assert_eq!(slot_of(Button::South), Some(Slot::Enter), "A (bottom)");
        assert_eq!(slot_of(Button::North), Some(Slot::ARace), "Y (top)");
        assert_eq!(slot_of(Button::West), Some(Slot::Brake), "X (left)");
        assert_eq!(slot_of(Button::East), None, "B (right) is free");

        // A → enter, on every screen, unconditioned.
        for fase in [0, 10, 11, 7, 1, 6, 5, 176, -5, -7] {
            let mut p = PadState::new();
            p.set_fase(fase);
            p.set(Slot::Enter, true);
            assert_eq!(
                p.edges(),
                vec![InputEvent::KeyDown(10)],
                "fase {fase}: A confirms"
            );
            p.set(Slot::Enter, false);
            assert_eq!(p.edges(), vec![InputEvent::KeyUp(10)]);
        }

        // Y → arrace, on every screen (inert in a menu is not ours to gate).
        for fase in [0, 10, 7] {
            let mut p = PadState::new();
            p.set_fase(fase);
            p.set(Slot::ARace, true);
            assert_eq!(
                p.edges(),
                vec![InputEvent::KeyDown(97)],
                "fase {fase}: Y is the keyboard's `a`"
            );
            p.set_fase(fase + 1); // a screen change mid-press changes nothing
            p.set(Slot::ARace, false);
            assert_eq!(p.edges(), vec![InputEvent::KeyUp(97)]);
        }

        // X → brake, on every screen, unconditioned (like the keyboard's down).
        for fase in [0, 10] {
            let mut p = PadState::new();
            p.set_fase(fase);
            p.set(Slot::Brake, true);
            assert_eq!(p.edges(), vec![InputEvent::KeyDown(1005)]);
            p.set(Slot::Brake, false);
            assert_eq!(p.edges(), vec![InputEvent::KeyUp(1005)]);
        }
    }

    /// The fase-111 boot screen is the applet's mouse gate, so the
    /// adapter's A/Start route synthesizes one MouseDown at the click
    /// panel's centre (275,355) — no key edge — on the down edge only, and
    /// consumes the latched release even after the screen has moved on.
    #[test]
    fn boot_screen_click_route_latches_one_mouse_down() {
        let mut p = PadState::new();
        // A confirms everywhere else: not consumed by the boot route
        assert!(!p.boot_route(true, 10));
        assert!(!p.boot_route(false, 10));

        // at 111 the press is consumed and arms one click
        assert!(p.boot_route(true, 111));
        assert_eq!(p.edges(), vec![InputEvent::MouseDown(275, 355)]);
        assert_eq!(p.edges(), vec![], "hold emits nothing");
        // a repeat press does not re-arm
        assert!(p.boot_route(true, 111));
        assert_eq!(p.edges(), vec![]);
        // the fade starts (fase 9) while held; the release is still ours
        assert!(p.boot_route(false, 9));
        assert_eq!(p.edges(), vec![], "a click has no release edge");
        assert!(!p.boot_route(false, 9), "the latch is clear again");

        // and a fresh press on a real menu is A's confirm once more
        p.set(Slot::Enter, true);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(10)]);
        assert_eq!(p.release_all(), vec![InputEvent::KeyUp(10)]);
    }

    /// A boot-screen press that never released is dropped by a disconnect.
    #[test]
    fn release_all_drops_an_armed_boot_click() {
        let mut p = PadState::new();
        assert!(p.boot_route(true, 111));
        assert_eq!(p.release_all(), vec![], "the click was armed, not emitted");
        assert_eq!(p.edges(), vec![], "and the arm is gone");
    }

    /// Two controls sharing a code (Start and A both emit 10) keep separate
    /// down-states: releasing one never releases the other.
    #[test]
    fn shared_code_controls_do_not_alias() {
        let mut p = PadState::new();
        p.set(Slot::Start, true);
        p.set(Slot::Enter, true);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(10)]);
        p.set(Slot::Enter, false);
        assert_eq!(p.edges(), vec![], "Start still holds enter");
        p.set(Slot::Start, false);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(10)]);
    }

    /// Stick hysteresis: a resting stick never chatters, the engage is a
    /// single edge past 0.5, the release waits for < 0.4.
    #[test]
    fn stick_threshold_has_hysteresis() {
        let mut p = PadState::new();
        for v in [0.0, 0.1, 0.2, 0.3, 0.45, 0.3, 0.1] {
            p.axis_value(Axis::LeftStickX, v);
            assert_eq!(p.edges(), vec![], "{v} is below the engage threshold");
        }
        p.axis_value(Axis::LeftStickX, 0.5);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1007)]);
        for v in [0.45, 0.41, 0.4, 0.5] {
            p.axis_value(Axis::LeftStickX, v);
            assert_eq!(p.edges(), vec![], "{v} still holds the steer");
        }
        p.axis_value(Axis::LeftStickX, 0.39);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1007)]);

        // ... and the left direction, from the negative side.
        p.axis_value(Axis::LeftStickX, -0.8);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1006)]);
        p.axis_value(Axis::LeftStickX, -0.1);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1006)]);
    }

    /// Both steer directions at once (stick one way, D-pad axis the other)
    /// fold into the two steer rows without cancelling each other: the
    /// larger magnitude wins, so a held hat is not zeroed by a resting stick.
    #[test]
    fn hat_axis_and_stick_share_the_steer_rows() {
        let mut p = PadState::new();
        p.axis_value(Axis::LeftStickX, 0.9);
        p.axis_value(Axis::DPadX, -1.0);
        // the hat wins by magnitude: left engages, right stays idle
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1006)]);
        p.axis_value(Axis::DPadX, 0.0);
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyDown(1007), InputEvent::KeyUp(1006)],
            "the stick takes over on the next frame, in WATCH order (right, then left)"
        );
    }

    /// Triggers arrive as axes and as synthesized buttons; either path holds
    /// the same control and the release is one edge. The axis values are
    /// gilrs' −1..1 (rest = −1, full = +1).
    #[test]
    fn triggers_drive_accel_and_handbrake() {
        let mut p = PadState::new();
        p.axis_value(Axis::RightZ, -1.0);
        p.axis_value(Axis::LeftZ, -1.0);
        assert_eq!(p.edges(), vec![], "both at rest");
        p.axis_value(Axis::RightZ, 1.0);
        p.axis_value(Axis::LeftZ, 0.2); // 0.6 travel, a light pull
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyDown(1004), InputEvent::KeyDown(32)],
            "WATCH order, both in one frame"
        );
        p.axis_value(Axis::RightZ, -0.9); // 0.05 travel
        p.axis_value(Axis::LeftZ, -0.5); // 0.25 travel
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyUp(1004), InputEvent::KeyUp(32)]
        );

        // the digital fallback path
        p.set(Slot::Accel, true);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1004)]);
        p.set(Slot::Accel, false);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1004)]);
    }

    /// The rear paddles resolve by raw evdev code, and only EV_KEY rows.
    #[test]
    fn rear_paddles_resolve_by_raw_code() {
        assert!(matches!(rear_slot(0x1_02c0), Some(Slot::Music1)));
        assert!(matches!(rear_slot(0x1_02c1), Some(Slot::Music2)));
        assert!(matches!(rear_slot(0x1_02c2), Some(Slot::Sfx1)));
        assert!(matches!(rear_slot(0x1_02c3), Some(Slot::Sfx2)));
        assert!(rear_slot(0x3_02c0).is_none(), "ABS codes are not buttons");
        assert!(rear_slot(0x1_02c4).is_none(), "beyond the mapped four");
        // and they land on the two mute codes of the keyboard
        let mut p = PadState::new();
        p.set(Slot::Music1, true);
        p.set(Slot::Sfx1, true);
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyDown(109), InputEvent::KeyDown(110)]
        );
    }

    /// A disconnect drops every held control with one up-edge each, and the
    /// state is then quiet.
    #[test]
    fn release_all_drops_every_held_control() {
        let mut p = PadState::new();
        p.set(Slot::Accel, true);
        p.set(Slot::Right, true);
        p.axis_value(Axis::LeftZ, 1.0);
        assert_eq!(
            p.edges(),
            vec![
                InputEvent::KeyDown(1004),
                InputEvent::KeyDown(1007),
                InputEvent::KeyDown(32)
            ]
        );
        assert_eq!(
            p.release_all(),
            vec![
                InputEvent::KeyUp(1004),
                InputEvent::KeyUp(1007),
                InputEvent::KeyUp(32)
            ],
            "WATCH order; only the held rows fire"
        );
        assert_eq!(p.edges(), vec![]);
    }

    /// Stick Y is the menu cursor outside the race, with the same 0.5/0.4
    /// hysteresis as stick X; in the race frame it is unmapped in both
    /// directions. Positive is up (gilrs normalizes the Y sign).
    #[test]
    fn stick_y_is_the_menu_cursor_outside_the_race() {
        let mut p = PadState::new();
        p.set_fase(10);
        for v in [0.0, 0.1, 0.2, 0.3, 0.45, 0.2] {
            p.axis_value(Axis::LeftStickY, v);
            assert_eq!(p.edges(), vec![], "{v} is below the engage threshold");
        }
        p.axis_value(Axis::LeftStickY, 0.5);
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyDown(1004)],
            "stick up = cursor up"
        );
        for v in [0.45, 0.41, 0.4, 0.6] {
            p.axis_value(Axis::LeftStickY, v);
            assert_eq!(p.edges(), vec![], "{v} still holds the cursor");
        }
        p.axis_value(Axis::LeftStickY, 0.39);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1004)]);
        p.axis_value(Axis::LeftStickY, -0.8);
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyDown(1005)],
            "stick down = cursor down"
        );
        p.axis_value(Axis::LeftStickY, -0.1);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1005)]);

        // the race frame keeps the stick Y unmapped, both directions
        let mut race = PadState::new();
        for v in [0.9, 0.0, -0.9, 0.0] {
            race.axis_value(Axis::LeftStickY, v);
            assert_eq!(race.edges(), vec![], "stick Y is not a race control");
        }
    }

    /// D-pad up is the camera view cycle (118) in the race frame and the
    /// menu cursor up (1004) on every other screen.
    #[test]
    fn dpad_up_is_view_in_the_race_and_cursor_in_menus() {
        let mut race = PadState::new();
        race.dpad_up(true, 0);
        assert_eq!(race.edges(), vec![InputEvent::KeyDown(118)]);
        race.dpad_up(true, 0);
        assert_eq!(race.edges(), vec![], "hold is silent");
        race.dpad_up(false, 0);
        assert_eq!(race.edges(), vec![InputEvent::KeyUp(118)]);

        for fase in [10, 11, 7, 1, 6, 5, 176, -5, -7] {
            let mut menu = PadState::new();
            menu.dpad_up(true, fase);
            assert_eq!(
                menu.edges(),
                vec![InputEvent::KeyDown(1004)],
                "fase {fase} is a menu screen: D-pad up moves the cursor"
            );
            menu.dpad_up(false, fase);
            assert_eq!(menu.edges(), vec![InputEvent::KeyUp(1004)]);
        }
    }

    /// D-pad down is the menu cursor down (1005) outside the race and
    /// unmapped in the race frame — today's behaviour exactly.
    #[test]
    fn dpad_down_is_cursor_down_only_outside_the_race() {
        let mut menu = PadState::new();
        menu.dpad_down(true, 10);
        assert_eq!(menu.edges(), vec![InputEvent::KeyDown(1005)]);
        menu.dpad_down(false, 10);
        assert_eq!(menu.edges(), vec![InputEvent::KeyUp(1005)]);

        let mut race = PadState::new();
        race.dpad_down(true, 0);
        assert_eq!(race.edges(), vec![], "D-pad down is unmapped in the race");
        race.dpad_down(true, 0);
        assert_eq!(race.edges(), vec![]);
        race.dpad_down(false, 0);
        assert_eq!(race.edges(), vec![], "and the release is silent too");
    }

    /// The vertical d-pad route is latched per press: a screen change while
    /// the button is held releases the slot it pressed and never emits a
    /// second key.
    #[test]
    fn dpad_route_is_latched_across_a_screen_change() {
        // pressed in the race (view); the race ends while it is held
        let mut p = PadState::new();
        p.dpad_up(true, 0);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(118)]);
        p.dpad_up(true, 10);
        assert_eq!(p.edges(), vec![], "a re-press on a new screen adds no edge");
        p.dpad_up(false, 10);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(118)]);

        // ... and the mirror: pressed in a menu, released after the race began
        p.dpad_up(true, 10);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1004)]);
        p.dpad_up(false, 0);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1004)]);

        // D-pad down latches the same way, including its race no-op
        p.dpad_down(true, 10);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(1005)]);
        p.dpad_down(false, 0);
        assert_eq!(p.edges(), vec![InputEvent::KeyUp(1005)]);
        p.dpad_down(true, 0);
        assert_eq!(p.edges(), vec![], "no key in the race");
        p.dpad_down(false, 10);
        assert_eq!(p.edges(), vec![], "the race press had no key to release");
    }

    /// The bumpers stand in for the Steam-Input-swallowed rear paddles:
    /// LB = music mute (109), RB = sfx mute (110), unconditioned. In gilrs
    /// the bumpers are `*Trigger` and the analog triggers `*Trigger2` — this
    /// pins that they are not swapped.
    #[test]
    fn bumpers_are_the_paddle_stand_ins() {
        let slot_of = |b: Button| BUTTONS.iter().find(|(x, _)| *x == b).map(|&(_, s)| s);
        assert_eq!(
            slot_of(Button::LeftTrigger),
            Some(Slot::Music1),
            "LB is the music mute"
        );
        assert_eq!(
            slot_of(Button::RightTrigger),
            Some(Slot::Sfx1),
            "RB is the sfx mute"
        );
        // the analog triggers keep their race controls
        assert_eq!(slot_of(Button::LeftTrigger2), Some(Slot::Handb));
        assert_eq!(slot_of(Button::RightTrigger2), Some(Slot::Accel));

        let mut p = PadState::new();
        p.set(Slot::Music1, true);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(109)]);
        p.set(Slot::Sfx1, true);
        assert_eq!(p.edges(), vec![InputEvent::KeyDown(110)]);
        p.set_fase(10); // global toggles: no fase gate
        p.set(Slot::Music1, false);
        p.set(Slot::Sfx1, false);
        assert_eq!(
            p.edges(),
            vec![InputEvent::KeyUp(109), InputEvent::KeyUp(110)],
            "WATCH order"
        );
    }

    /// A driver that reports the D-pad as the `DPadY` hat (no synthesized
    /// buttons) still drives the view in the race and the cursor in menus;
    /// once the pad is known to deliver `DPad*` buttons the hat is ignored so
    /// the two paths cannot fight the latched button route.
    #[test]
    fn hat_axis_is_the_fallback_when_no_dpad_buttons_report() {
        let mut race = PadState::new();
        race.axis_value(Axis::DPadY, 1.0);
        assert_eq!(
            race.edges(),
            vec![InputEvent::KeyDown(118)],
            "+1 is up: the race view"
        );
        race.axis_value(Axis::DPadY, 0.0);
        assert_eq!(race.edges(), vec![InputEvent::KeyUp(118)]);

        let mut menu = PadState::new();
        menu.set_fase(10);
        menu.axis_value(Axis::DPadY, 1.0);
        assert_eq!(menu.edges(), vec![InputEvent::KeyDown(1004)]);
        menu.axis_value(Axis::DPadY, -1.0);
        assert_eq!(
            menu.edges(),
            vec![InputEvent::KeyUp(1004), InputEvent::KeyDown(1005)],
            "WATCH order: the up row releases before the down row engages"
        );

        let mut both = PadState::new();
        both.set_fase(10);
        both.dpad_button_seen();
        both.axis_value(Axis::DPadY, 1.0);
        assert_eq!(
            both.edges(),
            vec![],
            "a pad with D-pad buttons is driven by them, not the hat"
        );
    }
}
