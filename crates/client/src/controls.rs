//! Controls lent: one plugin at a time may take a body's controls whole,
//! as one takes the pointer. While they are lent the body stands where it
//! is, as in a passage, and takes no movement; what the person presses goes
//! to the holder as a [`Pad`], each turn, in place of moving the body. The
//! eye goes with them: the holder gives a [`Look`], and the camera eases to
//! it from what the person saw, though the holder steps the body somewhere
//! as it takes them, and back to the body's camera once they come home.
//!
//! A key always leaves (`Key::Leave`), and so does the shell letting every
//! key go, a passage taking the body and the holder switched off: nothing
//! holds a person.

use core::ops::BitOr;

use glam::{DQuat, DVec3};

use crate::PartId;
use crate::input::{Input, Key};

/// How long the eye takes to ease to a holder's look, and back, seconds.
pub const EASE_S: f64 = 0.35;

/// How far into the eye's ease a screen's picture starts to be laid over
/// the view: by then the screen stands nearly where it ends.
const LAID_FROM: f64 = 0.8;

/// How far a stick is pushed, of its reach, before it presses one of its
/// four ways.
const STICK_PRESSES: f32 = 0.5;

/// What the controls press, as their holder reads them: the stick's four
/// ways, jump, sprint and start, one bit each.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct Pad(u8);

impl Pad {
    pub const NONE: Pad = Pad(0);
    pub const LEFT: Pad = Pad(1);
    pub const RIGHT: Pad = Pad(1 << 1);
    /// The stick forward.
    pub const UP: Pad = Pad(1 << 2);
    /// The stick back.
    pub const DOWN: Pad = Pad(1 << 3);
    pub const JUMP: Pad = Pad(1 << 4);
    pub const SPRINT: Pad = Pad(1 << 5);
    pub const START: Pad = Pad(1 << 6);

    /// Each control by the name a command gives it.
    const NAMED: [(&'static str, Pad); 7] = [
        ("left", Pad::LEFT),
        ("right", Pad::RIGHT),
        ("up", Pad::UP),
        ("down", Pad::DOWN),
        ("jump", Pad::JUMP),
        ("sprint", Pad::SPRINT),
        ("start", Pad::START),
    ];

    pub const fn bits(self) -> u8 {
        self.0
    }

    /// A game's buttons for what the pad presses: the stick its arrows,
    /// jump A, sprint B, start start. The pad's bits are the cartridge's
    /// contract's, held name by name by a test against it, so a shell hands
    /// a game this and nothing of its own.
    pub const fn buttons(self) -> u32 {
        self.0 as u32
    }

    pub const fn has(self, control: Pad) -> bool {
        self.0 & control.0 == control.0
    }

    /// Every control, one bit each: the contract's buttons of one pad.
    pub const ALL: Pad = Pad(0x7f);

    /// The controls `bits` press, a bit no control has dropped.
    pub const fn from_bits(bits: u8) -> Pad {
        Pad(bits & Pad::ALL.0)
    }

    /// A game's buttons for what the pad presses as the pad at `index` of
    /// the game, from 0: its bits in that pad's place of an update's word.
    pub const fn at(self, index: usize) -> u32 {
        self.buttons() << (arcade::PAD_BITS as usize * index)
    }

    /// What the pad at `index` presses in an update's word: its own bits
    /// alone, never another pad's.
    pub const fn of_pad(word: u32, index: usize) -> Pad {
        match word.checked_shr(arcade::PAD_BITS * index as u32) {
            Some(bits) => Pad::from_bits(bits as u8),
            None => Pad::NONE,
        }
    }

    /// A control by its name: `left`, `right`, `up`, `down`, `jump`,
    /// `sprint`, `start`.
    pub fn named(name: &str) -> Option<Pad> {
        Pad::NAMED
            .iter()
            .find(|(named, _)| *named == name)
            .map(|&(_, pad)| pad)
    }

    /// What a key presses on the pad: the game's keys, one table for a
    /// cabinet and a cartridge's own window, read from the shells' key
    /// bindings.
    pub const fn keyed(key: Key) -> Pad {
        match key {
            Key::Left => Pad::LEFT,
            Key::Right => Pad::RIGHT,
            Key::Forward => Pad::UP,
            Key::Back => Pad::DOWN,
            Key::Up | Key::ButtonA => Pad::JUMP,
            Key::Sprint | Key::ButtonB => Pad::SPRINT,
            Key::Start => Pad::START,
            _ => Pad::NONE,
        }
    }

    /// What the keys that went down since the last turn press: a key
    /// pressed and let go within one frame is pressed all the same.
    pub(crate) fn tapped(keys: &[Key]) -> Pad {
        keys.iter()
            .map(|&key| Pad::keyed(key))
            .fold(Pad::NONE, BitOr::bitor)
    }

    /// What the keys and the stick of `input` press now.
    pub(crate) fn of(input: &Input) -> Pad {
        let [right, forward] = input.movement();
        let pressed = |on: bool, pad: Pad| if on { pad } else { Pad::NONE };
        let buttons = [Key::Up, Key::ButtonA, Key::Sprint, Key::ButtonB, Key::Start];
        pressed(right <= -STICK_PRESSES, Pad::LEFT)
            | pressed(right >= STICK_PRESSES, Pad::RIGHT)
            | pressed(forward >= STICK_PRESSES, Pad::UP)
            | pressed(forward <= -STICK_PRESSES, Pad::DOWN)
            | buttons
                .into_iter()
                .filter(|&key| input.held(key))
                .map(Pad::keyed)
                .fold(Pad::NONE, BitOr::bitor)
    }
}

impl BitOr for Pad {
    type Output = Pad;
    fn bitor(self, other: Pad) -> Pad {
        Pad(self.0 | other.0)
    }
}

/// Where the eye looks from while the controls are lent, in the world.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Look {
    pub position: DVec3,
    /// World from camera: the camera looks down its `-Z`, `+Y` is up.
    pub rotation: DQuat,
    /// The cabinet whose screen the eye is held on: once the eye is there,
    /// its game's picture is laid over the view as drawn
    /// ([`crate::overlay`]). A part that runs no game lays nothing.
    pub shows: Option<PartId>,
}

/// The controls and the eye, and who they are lent to.
#[derive(Default)]
pub(crate) struct Controls {
    holder: Option<&'static str>,
    /// The last look a holder gave, kept while the eye eases back from it.
    look: Option<Look>,
    /// Whether the holder gave a look: the eye eases to it while it does,
    /// and back from it once it does not.
    looking: bool,
    /// How far the eye has eased to the look, 0 to 1.
    eased: f64,
    /// What the eye eased from in the last frame drawn: the body's camera,
    /// or [`Controls::from`] while it holds.
    base: Option<scene::Camera>,
    /// What the eye eases in from while it does: the camera the person saw
    /// when the controls were lent, before a holder moved the body. Taken
    /// back before the eye arrived, it carries how far the eye had eased:
    /// from there home, it fades into the body's own camera.
    from: Option<(scene::Camera, f64)>,
    /// What a command holds down, until a command changes it.
    commanded: Pad,
    /// Who the front end was last told holds them.
    told: Option<&'static str>,
}

impl Controls {
    /// The plugin that holds the controls, while one does.
    pub(crate) fn holder(&self) -> Option<&'static str> {
        self.holder
    }

    /// Lends the controls to `plugin`, in the place of whoever held them,
    /// with the look the eye eases to; with none, the eye stays with the
    /// body, or eases back to it. False for a look that is no place, and
    /// nothing is lent.
    pub(crate) fn lend(&mut self, plugin: &'static str, look: Option<Look>) -> bool {
        if let Some(look) = look
            && !(look.position.is_finite() && look.rotation.is_finite())
        {
            return false;
        }
        if self.holder != Some(plugin) {
            self.commanded = Pad::NONE;
        }
        if look.is_some() && !self.looking {
            self.from = self.base.map(|base| (base, 1.0));
        }
        self.holder = Some(plugin);
        self.looking = look.is_some();
        if let Some(look) = look {
            self.look = Some(Look {
                rotation: look.rotation.normalize(),
                ..look
            });
        }
        true
    }

    /// Takes the controls home: the body moves again at once, and the eye
    /// eases back.
    pub(crate) fn take_back(&mut self) {
        if self.looking
            && let Some((_, left)) = &mut self.from
        {
            *left = self.eased;
        }
        if self.eased == 0.0 {
            self.from = None;
        }
        self.holder = None;
        self.looking = false;
        self.commanded = Pad::NONE;
    }

    /// Holds `pad` down, as a command does, in the place of what a command
    /// held: until a command changes it or the controls go back.
    pub(crate) fn command(&mut self, pad: Pad) {
        self.commanded = pad;
    }

    /// What the holder reads this turn: what the person holds, what went
    /// down since the last turn, and what a command holds.
    pub(crate) fn pad(&self, input: &Input, tapped: Pad) -> Pad {
        Pad::of(input) | tapped | self.commanded
    }

    /// The eye `dt` seconds on: toward the look while they are lent, back
    /// once they are not.
    pub(crate) fn ease(&mut self, dt: f64) {
        let toward = match self.looking {
            true => 1.0,
            false => 0.0,
        };
        // Steps toward where it goes and rests there.
        let step = dt.max(0.0) / EASE_S;
        self.eased = match toward > self.eased {
            true => (self.eased + step).min(toward),
            false => (self.eased - step).max(toward),
        };
        if self.eased == 0.0 && !self.looking {
            self.look = None;
        }
        // Arrived, or home, the eye eases from the body's camera again.
        if self.eased == toward {
            self.from = None;
        }
    }

    /// Whether the eye is more the holder's than the body's: the body's own
    /// figure casts its shadow and is not seen, as from its head, so it
    /// never stands between the eye and what the holder shows.
    pub(crate) fn eye_away(&self) -> bool {
        self.eased > 0.5
    }

    /// The camera, eased toward the holder's look as far as the eye is:
    /// from `camera`, the body's, or from what the person saw when the
    /// controls were lent while the eye eases in from it.
    pub(crate) fn eye(&mut self, camera: scene::Camera) -> scene::Camera {
        let camera = match self.from {
            None => camera,
            Some((from, _)) if self.looking => from,
            // Home from partway in: the body's own camera by the time the
            // eye is.
            Some((from, left)) => between(&camera, &from, smooth(self.eased) / smooth(left)),
        };
        self.base = Some(camera);
        let Some(look) = self.look.filter(|_| self.eased > 0.0) else {
            return camera;
        };
        // Eased all the way, the eye is the look itself, whatever it eased
        // from.
        if self.eased >= 1.0 {
            return scene::Camera {
                position: look.position,
                rotation: look.rotation,
                ..camera
            };
        }
        let to = scene::Camera {
            position: look.position,
            rotation: look.rotation,
            ..camera
        };
        between(&camera, &to, smooth(self.eased))
    }

    /// The part whose screen the look shows, and how far its picture is
    /// laid over the view: from nothing at [`LAID_FROM`] of the ease to
    /// whole at its end, and back as the eye leaves.
    pub(crate) fn shown(&self) -> Option<(PartId, f32)> {
        let shows = self.look?.shows?;
        let laid = ((self.eased - LAID_FROM) / (1.0 - LAID_FROM)).clamp(0.0, 1.0);
        (laid > 0.0).then_some((shows, laid as f32))
    }

    /// Who holds them, when that changed since the front end was told.
    pub(crate) fn tell(&mut self) -> Option<Option<&'static str>> {
        (self.told != self.holder).then(|| {
            self.told = self.holder;
            self.holder
        })
    }
}

/// How far along an ease `eased` of the way is: slow out of the start and
/// into the end.
fn smooth(eased: f64) -> f64 {
    eased * eased * (3.0 - 2.0 * eased)
}

/// The camera `t` of the way from `from` to `to`, seeing as `from` does.
fn between(from: &scene::Camera, to: &scene::Camera, t: f64) -> scene::Camera {
    scene::Camera {
        position: from.position.lerp(to.position, t),
        rotation: from.rotation.slerp(to.rotation, t),
        ..*from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> scene::Camera {
        scene::Camera {
            position: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            fov_y: 1.0,
            near: 0.1,
        }
    }

    #[test]
    fn the_keys_and_the_stick_press_the_pad() {
        let mut input = Input::default();
        input.key(Key::Left, true);
        input.key(Key::Up, true);
        assert_eq!(Pad::of(&input), Pad::LEFT | Pad::JUMP);
        input.key(Key::Left, false);
        input.stick = [0.0, 0.8];
        assert_eq!(Pad::of(&input), Pad::UP | Pad::JUMP);
        input.stick = [0.3, 0.0];
        input.key(Key::Up, false);
        assert_eq!(Pad::of(&input), Pad::NONE, "a stick pushed a little");
    }

    #[test]
    fn each_control_presses_the_games_button_of_its_name() {
        // Each control, and the button the emulator names it plays.
        let controls = [
            ("left", "left"),
            ("right", "right"),
            ("up", "up"),
            ("down", "down"),
            ("jump", "a"),
            ("sprint", "b"),
            ("start", "start"),
        ];
        assert_eq!(controls.len(), Pad::NAMED.len());
        assert_eq!(controls.len(), arcade::BUTTONS.len());
        let button = |name| arcade::BUTTONS.iter().find(|(named, _)| *named == name);
        for (control, name) in controls {
            let bit = button(name).map(|&(_, bit)| bit);
            assert_eq!(Pad::named(control).map(Pad::buttons), bit, "{control}");
        }
        let all = Pad::NAMED
            .iter()
            .fold(Pad::NONE, |all, &(_, pad)| all | pad);
        let every = arcade::BUTTONS.iter().fold(0, |all, &(_, bit)| all | bit);
        assert_eq!(all.buttons(), every);
    }

    #[test]
    fn a_command_holds_until_a_command_changes_it_or_the_controls_go() {
        let mut controls = Controls::default();
        controls.lend("p", None);
        controls.command(Pad::RIGHT | Pad::JUMP);
        let input = Input::default();
        for _ in 0..3 {
            assert_eq!(controls.pad(&input, Pad::NONE), Pad::RIGHT | Pad::JUMP);
        }
        controls.command(Pad::RIGHT);
        assert_eq!(controls.pad(&input, Pad::START), Pad::RIGHT | Pad::START);
        controls.take_back();
        controls.lend("p", None);
        assert_eq!(controls.pad(&input, Pad::NONE), Pad::NONE);
    }

    #[test]
    fn a_key_tapped_within_a_frame_presses() {
        let mut input = Input::default();
        input.key(Key::Start, true);
        input.key(Key::Left, true);
        input.key(Key::Start, false);
        input.key(Key::Left, false);
        let taken = input.take_frame();
        assert_eq!(Pad::of(&input), Pad::NONE);
        assert_eq!(Pad::tapped(&taken.pressed), Pad::START | Pad::LEFT);
    }

    #[test]
    fn the_eye_eases_to_the_look_and_back() {
        let mut controls = Controls::default();
        let look = Look {
            position: DVec3::new(0.0, 0.0, 2.0),
            rotation: DQuat::from_rotation_y(1.0),
            shows: None,
        };
        controls.lend("p", Some(look));
        assert_eq!(controls.eye(camera()).position, DVec3::ZERO);
        controls.ease(EASE_S / 2.0);
        let half = controls.eye(camera()).position.z;
        assert!(half > 0.0 && half < 2.0, "{half}");
        controls.ease(EASE_S);
        assert_eq!(controls.eye(camera()).position, look.position);
        // Held, it stays there frame after frame.
        for _ in 0..3 {
            controls.ease(1.0 / 60.0);
            assert_eq!(controls.eye(camera()).position, look.position);
        }
        controls.take_back();
        controls.ease(EASE_S / 2.0);
        assert!(controls.eye(camera()).position.z > 0.0, "it eases back");
        controls.ease(EASE_S);
        assert_eq!(controls.eye(camera()).position, DVec3::ZERO);
    }

    /// The body's camera `x` metres along.
    fn moved(x: f64) -> scene::Camera {
        scene::Camera {
            position: DVec3::X * x,
            ..camera()
        }
    }

    #[test]
    fn the_eye_eases_in_from_what_was_seen_though_the_body_stepped_away() {
        let mut controls = Controls::default();
        let look = Look {
            position: DVec3::new(0.0, 0.0, 2.0),
            rotation: DQuat::IDENTITY,
            shows: None,
        };
        // Seen from where the body stood, then lent as the holder steps it
        // a metre along.
        assert_eq!(controls.eye(camera()).position, DVec3::ZERO);
        controls.lend("p", Some(look));
        assert_eq!(controls.eye(moved(1.0)).position, DVec3::ZERO, "no cut");
        let mut last = DVec3::ZERO;
        for _ in 0..30 {
            controls.ease(EASE_S / 30.0);
            let now = controls.eye(moved(1.0)).position;
            assert_eq!(now.x, 0.0, "from where it was seen, never the body's step");
            assert!(now.z >= last.z);
            last = now;
        }
        assert_eq!(last, look.position);
        // Home, it eases to where the body now stands.
        controls.take_back();
        controls.ease(EASE_S);
        assert_eq!(controls.eye(moved(1.0)).position, DVec3::X);
    }

    #[test]
    fn taken_back_partway_in_the_eye_turns_round_where_it_is_and_ends_with_the_body() {
        let mut controls = Controls::default();
        let look = Look {
            position: DVec3::new(0.0, 0.0, 2.0),
            rotation: DQuat::IDENTITY,
            shows: None,
        };
        controls.eye(camera());
        controls.lend("p", Some(look));
        controls.ease(EASE_S / 2.0);
        let there = controls.eye(moved(1.0)).position;
        controls.take_back();
        assert_eq!(controls.eye(moved(1.0)).position, there, "no cut");
        let mut last = there;
        for _ in 0..15 {
            controls.ease(EASE_S / 30.0);
            let now = controls.eye(moved(1.0)).position;
            assert!((now - last).length() < 0.25, "{last} to {now}");
            last = now;
        }
        assert!((last - DVec3::X).length() < 1e-9, "{last}");
        controls.ease(EASE_S);
        assert_eq!(controls.eye(moved(1.0)).position, DVec3::X);
    }

    #[test]
    fn lent_with_no_look_the_eye_goes_home_never_to_the_last_look() {
        let mut controls = Controls::default();
        let look = Look {
            position: DVec3::new(0.0, 0.0, 2.0),
            rotation: DQuat::IDENTITY,
            shows: None,
        };
        controls.lend("p", Some(look));
        controls.ease(EASE_S);
        controls.take_back();
        controls.ease(EASE_S / 2.0);
        // Lent again with no look while it eases back: it goes on home.
        controls.lend("q", None);
        let mut away = controls.eye(camera()).position.z;
        for _ in 0..30 {
            controls.ease(1.0 / 60.0);
            let now = controls.eye(camera()).position.z;
            assert!(now <= away, "it turned back to the old look");
            away = now;
        }
        assert_eq!(controls.eye(camera()).position, DVec3::ZERO);
        // Taken from a holder that looks by one that does not: it goes home.
        controls.lend("p", Some(look));
        controls.ease(EASE_S);
        controls.lend("q", None);
        controls.ease(EASE_S);
        assert_eq!(controls.eye(camera()).position, DVec3::ZERO);
    }

    #[test]
    fn a_look_that_is_no_place_is_refused() {
        let mut controls = Controls::default();
        let nowhere = Look {
            position: DVec3::new(f64::NAN, 0.0, 0.0),
            rotation: DQuat::IDENTITY,
            shows: None,
        };
        assert!(!controls.lend("p", Some(nowhere)));
        assert_eq!(controls.holder(), None);
        let turned = Look {
            position: DVec3::ZERO,
            rotation: DQuat::from_xyzw(f64::INFINITY, 0.0, 0.0, 1.0),
            shows: None,
        };
        assert!(!controls.lend("p", Some(turned)));
        assert!(controls.lend("p", None));
    }

    #[test]
    fn a_screen_shown_is_laid_over_the_end_of_the_ease_and_lifted_as_it_leaves() {
        let mut controls = Controls::default();
        let screen = PartId(7);
        let look = Look {
            position: DVec3::new(0.0, 0.0, 2.0),
            rotation: DQuat::IDENTITY,
            shows: Some(screen),
        };
        controls.lend("p", Some(look));
        controls.ease(EASE_S * LAID_FROM);
        assert_eq!(controls.shown(), None, "not before the eye is nearly there");
        controls.ease(EASE_S * (1.0 - LAID_FROM) / 2.0);
        let (shown, half) = controls.shown().expect("laid as the eye arrives");
        assert_eq!(shown, screen);
        assert!((half - 0.5).abs() < 1e-3, "{half}");
        controls.ease(EASE_S);
        assert_eq!(controls.shown(), Some((screen, 1.0)));
        controls.take_back();
        controls.ease(EASE_S * (1.0 - LAID_FROM) / 2.0);
        assert!(
            controls.shown().is_some_and(|(_, laid)| laid < 1.0),
            "lifted as it leaves"
        );
        controls.ease(EASE_S);
        assert_eq!(controls.shown(), None);
    }

    #[test]
    fn who_holds_them_is_told_on_change() {
        let mut controls = Controls::default();
        assert_eq!(controls.tell(), None);
        controls.lend("p", None);
        assert_eq!(controls.tell(), Some(Some("p")));
        controls.lend("p", None);
        assert_eq!(controls.tell(), None);
        controls.take_back();
        assert_eq!(controls.tell(), Some(None));
    }

    #[test]
    fn z_and_x_press_a_and_b_as_space_and_shift_do() {
        let played = [
            (Key::Up, Pad::JUMP),
            (Key::ButtonA, Pad::JUMP),
            (Key::Sprint, Pad::SPRINT),
            (Key::ButtonB, Pad::SPRINT),
        ];
        for (key, pad) in played {
            let mut input = Input::default();
            input.key(key, true);
            assert_eq!(Pad::of(&input), pad, "{key:?} held");
            assert_eq!(Pad::tapped(&[key]), pad, "{key:?} tapped");
        }
    }
}
