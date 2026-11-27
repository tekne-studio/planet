//! Input as the controller sees it: intent, not key codes.
//!
//! Platform shells translate their own key and pointer events into [`Key`]s
//! and deltas. Bindings live in the shells; meaning lives here.
//!
//! A plugin's keys are its own: it asks for each by name, as a [`KeyAsk`],
//! and a shell hands over the keys of its keyboard that a plugin asked for,
//! by the name the web gives them.

use crate::boom::ZOOM_PER_LINE;
use crate::controller::LOOK_RAD_PER_PX;

/// How far a stick is pushed, of its whole reach, when it runs: on foot,
/// and on a mount at the pace sprint means.
const RUN_EXTENT: f32 = 0.9;
/// How far past an edge between two of a stick's bands it is pushed, of its
/// reach, before it leaves the band it was in: a thumb resting on an edge
/// keeps its gait.
pub(crate) const SLACK: f32 = 0.05;

/// Where each band of a stick's reach starts, for `n` speeds slowest first,
/// past the first: the last from [`RUN_EXTENT`], where it runs, and the
/// rest sharing the reach below it evenly. Two speeds, a walk and a run,
/// start the run at the extent; a horse's four start the trot at 0.3, the
/// gallop at 0.6 and the run at 0.9.
pub(crate) fn edges(n: usize) -> impl Iterator<Item = f32> + Clone {
    let shares = n.saturating_sub(1).max(1) as f32;
    (1..n).map(move |at| RUN_EXTENT * (at as f32 / shares))
}

/// The band a stick pushed `reach` of its reach asks, among those `edges`
/// start, from the band `was` it asked last: it leaves a band only once
/// past an edge by [`SLACK`], up or down.
pub(crate) fn band(reach: f32, edges: impl Iterator<Item = f32> + Clone, was: usize) -> usize {
    let past = |at: f32| edges.clone().filter(|&edge| edge <= at).count();
    was.clamp(past(reach - SLACK), past(reach + SLACK))
}

/// A held or pressed control.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Forward,
    Back,
    Left,
    Right,
    /// Jump on foot, rise in flight.
    Up,
    /// Sink in flight.
    Down,
    Sprint,
    /// Switch between walking and flying.
    ToggleMode,
    /// Explore another planet: a fresh random seed.
    NewSeed,
    /// Wear the next avatar on offer.
    NextAvatar,
    /// Swing the camera to the eye, or out of it: first person and third.
    FirstPerson,
    /// The pointer's button while a plugin has the pointer: what its hand
    /// does is the plugin's.
    Use,
    /// Take back the last change, of whatever owner, and put it back.
    Undo,
    Redo,
    /// Act on what the prompt offers: what is in front of the body.
    Act,
    /// Whistle: [`crate::Command::Sound`] with the core's whistle.
    Whistle,
    /// Start, while the controls are lent: what a pad's start does is the
    /// holder's. Nothing on foot.
    Start,
    /// A pad's A and B while the controls are lent, beside jump and sprint
    /// that press them too: the keys a game is played with on a keyboard.
    /// Nothing on foot.
    ButtonA,
    ButtonB,
    /// Take the controls back from whoever holds them: the key that always
    /// leaves.
    Leave,
}

/// What is held with a key as it goes down.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Chord {
    /// Command on a Mac, Control elsewhere.
    pub command: bool,
    pub shift: bool,
}

/// A key a plugin asks for by name. It is data: a shell with a keyboard
/// binds it by `code`, and one without binds the name to what it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct KeyAsk {
    /// The name the plugin hears it by. Two keys may share one.
    pub name: &'static str,
    /// The key on a keyboard, as the web names it: `KeyB`, `Digit1`,
    /// `AltLeft`, `Escape`.
    pub code: &'static str,
    /// What is held with it. A plain key is heard whatever the shift.
    pub chord: Chord,
    /// Read while it is held, in the plugin's turn. Otherwise it is heard
    /// once, as it goes down.
    pub held: bool,
}

impl KeyAsk {
    /// A key heard once as it goes down, with nothing held.
    pub const fn pressed(name: &'static str, code: &'static str) -> KeyAsk {
        KeyAsk {
            name,
            code,
            chord: Chord {
                command: false,
                shift: false,
            },
            held: false,
        }
    }

    /// A key read while it is held.
    pub const fn held(name: &'static str, code: &'static str) -> KeyAsk {
        KeyAsk {
            held: true,
            ..KeyAsk::pressed(name, code)
        }
    }

    /// A key heard once as it goes down with command held, and with shift
    /// or without it.
    pub const fn command(name: &'static str, code: &'static str, shift: bool) -> KeyAsk {
        KeyAsk {
            chord: Chord {
                command: true,
                shift,
            },
            ..KeyAsk::pressed(name, code)
        }
    }

    /// Whether a key that went down with a chord is this one.
    pub(crate) fn is(&self, code: &str, chord: Chord) -> bool {
        self.code == code
            && self.chord.command == chord.command
            && (!self.chord.command || self.chord.shift == chord.shift)
    }
}

/// Input gathered by a shell since the last update.
#[derive(Clone, Debug, Default)]
pub struct Input {
    held: [bool; 12],
    /// Keys that went down since the last update.
    pressed: Vec<Key>,
    /// The keys of a keyboard that are down, as the web names them: what a
    /// plugin may have asked to read held.
    codes_held: Vec<String>,
    /// Those that went down since the last update, each with what was held
    /// with it.
    codes_pressed: Vec<(String, Chord)>,
    /// Whether the shell let everything go since the last update.
    interrupted: bool,
    /// Whether the pointer's secondary button was tapped since the last
    /// update: pressed and let go without a drag.
    inspected: bool,
    /// Pointer motion in pixels, x right, y down.
    pub look: [f32; 2],
    /// Wheel motion in lines, positive zooms in: a line brings the camera a
    /// tenth nearer, and a line past the nearest takes it to the eye.
    pub zoom: f32,
    /// Where a stick points, `[right, forward]`, no longer than one: how
    /// far of its reach it is pushed. The shell that has one, drawn on a
    /// screen or on a pad, keeps it here while it is held and clears it as
    /// it is let go; the keys add to it.
    pub stick: [f32; 2],
    /// Where the pointer is, as fractions of the view from the top left.
    /// `None` while it is captured: then it aims through the middle.
    pub pointer: Option<[f32; 2]>,
    /// What is held right now, command and shift: the core's keys, which a
    /// plugin reads in its turn and never asks for.
    pub chord: Chord,
    /// The stick asked the run when last read: what [`Input::sprinting`]
    /// holds its edge by.
    ran: bool,
}

impl Input {
    /// Records a key transition. Repeats are harmless. Every key is heard
    /// as it goes down; a held one is also read down until it comes up.
    pub fn key(&mut self, key: Key, down: bool) {
        let held = held_slot(key).map(|slot| core::mem::replace(&mut self.held[slot], down));
        if down && held != Some(true) {
            self.pressed.push(key);
        }
    }

    pub fn held(&self, key: Key) -> bool {
        held_slot(key).is_some_and(|slot| self.held[slot])
    }

    /// Records a transition of a key of a keyboard by the name the web gives
    /// it, with what is held with it: what the plugins hear. A shell passes
    /// the keys [`crate::Client::asks`] for as they go down, leaving out the
    /// repeats of a key held, and every key as it comes up.
    pub fn code(&mut self, code: &str, chord: Chord, down: bool) {
        self.hold(code, down);
        if down {
            self.codes_pressed.push((code.to_owned(), chord));
        }
    }

    /// Says whether a key of a keyboard is down, and nothing went down: how
    /// a shell keeps a held key in step with what a pointer says is held
    /// with it. Repeats are harmless.
    pub fn hold(&mut self, code: &str, down: bool) {
        let at = self.codes_held.iter().position(|held| held == code);
        match (down, at) {
            (true, None) => self.codes_held.push(code.to_owned()),
            (false, Some(at)) => {
                self.codes_held.swap_remove(at);
            }
            _ => {}
        }
    }

    pub(crate) fn code_held(&self, code: &str) -> bool {
        self.codes_held.iter().any(|held| held == code)
    }

    /// `[right, forward]`, each `-1..=1`: the keys and the stick together.
    pub fn movement(&self) -> [f32; 2] {
        let axis = |pos: Key, neg: Key| f32::from(self.held(pos)) - f32::from(self.held(neg));
        let keys = [axis(Key::Right, Key::Left), axis(Key::Forward, Key::Back)];
        [0, 1].map(|at| (keys[at] + self.stick[at]).clamp(-1.0, 1.0))
    }

    /// Whether to run: sprint held, or the stick pushed to its extent, its
    /// band of the run among two speeds, a walk and a run, as a mount's
    /// paces are banded ([`band`]).
    pub fn sprinting(&mut self) -> bool {
        let [x, y] = self.stick;
        let reach = (x * x + y * y).sqrt();
        self.ran = band(reach, edges(2), usize::from(self.ran)) == 1;
        self.held(Key::Sprint) || self.ran
    }

    /// How far forward the stick is pushed, of its reach, while one is
    /// held off its centre: negative pulled back. What paces a mount
    /// ([`crate::Wish::reach`]); none with no stick, or one back at its
    /// centre.
    pub fn reach(&self) -> Option<f32> {
        (self.stick != [0.0; 2]).then_some(self.stick[1])
    }

    /// Turns the eye by angles, radians: right, and down, as the pointer's
    /// motion that would.
    pub fn look_by(&mut self, [right_rad, down_rad]: [f64; 2]) {
        let [x, y] = [right_rad, down_rad].map(|rad| (rad / LOOK_RAD_PER_PX) as f32);
        self.look = [self.look[0] + x, self.look[1] + y];
    }

    /// Brings the eye nearer by a ratio, as two fingers spreading apart do:
    /// at two the eye is half as far, at a half twice as far.
    pub fn zoom_by(&mut self, ratio: f32) {
        if ratio > 0.0 && ratio.is_finite() {
            self.zoom += (1.0 - 1.0 / ratio) / ZOOM_PER_LINE as f32;
        }
    }

    /// Releases everything, for when the window loses focus. What a held key
    /// was in the middle of is dropped, never landed: a stroke half drawn
    /// when the window blurs goes the way of Escape, not of a click let go.
    pub fn release_all(&mut self) {
        self.held = Default::default();
        self.codes_held.clear();
        self.interrupted = true;
    }

    /// The pointer's secondary button was tapped: pressed and let go where
    /// it was, with no drag between. What picks a thing in the world to
    /// look at it again, whoever has the pointer.
    pub fn inspect(&mut self) {
        self.inspected = true;
    }

    /// Takes the one shot input, leaving held keys in place.
    pub(crate) fn take_frame(&mut self) -> Taken {
        let taken = Taken {
            pressed: core::mem::take(&mut self.pressed),
            codes: core::mem::take(&mut self.codes_pressed),
            look: self.look,
            zoom: self.zoom,
            interrupted: self.interrupted,
            inspected: self.inspected,
        };
        self.look = [0.0; 2];
        self.zoom = 0.0;
        self.interrupted = false;
        self.inspected = false;
        taken
    }
}

/// The one shot input of a frame.
pub(crate) struct Taken {
    pub pressed: Vec<Key>,
    /// Keys of a keyboard that went down, each with what was held with it.
    pub codes: Vec<(String, Chord)>,
    pub look: [f32; 2],
    pub zoom: f32,
    /// Whether the shell let everything go.
    pub interrupted: bool,
    /// Whether the pointer's secondary button was tapped.
    pub inspected: bool,
}

fn held_slot(key: Key) -> Option<usize> {
    match key {
        Key::Forward => Some(0),
        Key::Back => Some(1),
        Key::Left => Some(2),
        Key::Right => Some(3),
        Key::Up => Some(4),
        Key::Down => Some(5),
        Key::Sprint => Some(6),
        Key::Use => Some(7),
        Key::Act => Some(8),
        Key::Start => Some(9),
        Key::ButtonA => Some(10),
        Key::ButtonB => Some(11),
        Key::Undo | Key::Redo | Key::Leave => None,
        Key::ToggleMode | Key::NewSeed | Key::NextAvatar | Key::FirstPerson | Key::Whistle => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losing_focus_lets_go_and_interrupts_once() {
        let mut input = Input::default();
        input.key(Key::Use, true);
        input.release_all();
        assert!(!input.held(Key::Use));
        assert!(input.take_frame().interrupted);
        assert!(!input.take_frame().interrupted);
    }

    #[test]
    fn a_key_of_the_keyboard_is_heard_once_and_read_while_held() {
        let mut input = Input::default();
        let command = Chord {
            command: true,
            shift: false,
        };
        // A key that never came up, as under a held command, is heard again.
        input.code("KeyZ", command, true);
        input.code("KeyZ", command, true);
        input.code("AltLeft", Chord::default(), true);
        assert_eq!(input.take_frame().codes.len(), 3);
        assert!(input.code_held("AltLeft"));
        input.code("AltLeft", Chord::default(), false);
        assert!(!input.code_held("AltLeft"));
        // Held in step with a pointer, nothing went down.
        input.hold("AltLeft", true);
        input.hold("AltLeft", true);
        assert!(input.code_held("AltLeft"));
        assert!(input.take_frame().codes.is_empty());
        // Letting everything go lets these go too.
        input.release_all();
        assert!(!input.code_held("KeyZ"));
    }

    #[test]
    fn a_stick_adds_to_the_keys_and_runs_at_its_extent() {
        let mut input = Input {
            stick: [0.5, 0.0],
            ..Input::default()
        };
        assert_eq!(input.movement(), [0.5, 0.0]);
        assert!(!input.sprinting());
        input.key(Key::Right, true);
        input.key(Key::Forward, true);
        assert_eq!(input.movement(), [1.0, 1.0]);
        input.stick = [0.0, -1.0];
        assert!(input.sprinting());
        // Letting the keys go leaves the stick: the shell clears it.
        input.release_all();
        assert_eq!(input.movement(), [0.0, -1.0]);
    }

    #[test]
    fn the_run_holds_its_edge_by_the_slack() {
        let mut input = Input::default();
        let mut at = |reach: f32| {
            input.stick = [0.0, reach];
            input.sprinting()
        };
        assert!(!at(0.9), "on the edge, it walks on");
        assert!(at(0.96), "past it by the slack, it runs");
        assert!(at(0.86), "a thumb resting under the edge runs on");
        assert!(!at(0.84), "under it by the slack, it walks");
        assert!(
            !at(0.0) && input.reach().is_none(),
            "at its centre it is no stick"
        );
    }

    #[test]
    fn the_bands_share_the_reach_below_the_run() {
        let at = |n| edges(n).collect::<Vec<_>>();
        assert_eq!(at(1), Vec::<f32>::new());
        assert_eq!(at(2), vec![0.9]);
        let four = at(4);
        assert!((four[0] - 0.3).abs() < 1e-6 && (four[1] - 0.6).abs() < 1e-6);
        assert_eq!(four[2], 0.9);
        assert_eq!(band(0.2, edges(4), 0), 0);
        assert_eq!(
            band(0.62, edges(4), 0),
            1,
            "a jump of two bands stops short of an edge's slack"
        );
        assert_eq!(band(0.66, edges(4), 0), 2);
        assert_eq!(band(1.0, edges(4), 0), 3);
        assert_eq!(band(0.0, edges(4), 3), 0);
    }

    #[test]
    fn a_ratio_zooms_as_far_as_it_says() {
        let mut input = Input::default();
        input.zoom_by(2.0);
        let lines = input.take_frame().zoom;
        assert!((1.0 - lines * ZOOM_PER_LINE as f32 - 0.5).abs() < 1e-6);
        input.zoom_by(0.5);
        let lines = input.take_frame().zoom;
        assert!((1.0 - lines * ZOOM_PER_LINE as f32 - 2.0).abs() < 1e-6);
        input.zoom_by(0.0);
        input.zoom_by(f32::INFINITY);
        assert_eq!(input.take_frame().zoom, 0.0);
    }

    #[test]
    fn an_angle_turns_the_eye_as_far_as_it_says() {
        let mut input = Input::default();
        input.look_by([0.3, -0.1]);
        input.look_by([0.1, 0.0]);
        let turned = input
            .take_frame()
            .look
            .map(|px| f64::from(px) * LOOK_RAD_PER_PX);
        assert!((turned[0] - 0.4).abs() < 1e-6 && (turned[1] + 0.1).abs() < 1e-6);
    }

    #[test]
    fn a_plain_key_is_heard_whatever_the_shift_and_a_chord_exactly() {
        let plain = KeyAsk::pressed("build", "KeyB");
        let shift = Chord {
            command: false,
            shift: true,
        };
        assert!(plain.is("KeyB", Chord::default()) && plain.is("KeyB", shift));
        let undo = KeyAsk::command("undo", "KeyZ", false);
        let redo = KeyAsk::command("redo", "KeyZ", true);
        let with = |shift| Chord {
            command: true,
            shift,
        };
        assert!(undo.is("KeyZ", with(false)) && !undo.is("KeyZ", with(true)));
        assert!(redo.is("KeyZ", with(true)) && !redo.is("KeyZ", with(false)));
        assert!(!undo.is("KeyZ", Chord::default()) && !plain.is("KeyB", with(false)));
    }
}
