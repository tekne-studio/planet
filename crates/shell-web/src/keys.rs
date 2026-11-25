//! The core's keys on the web: which key a code is, and where it goes.
//! Plain logic, so its tests run natively.

use client::{Chord, Input, Key};

/// The core's key bindings. Mirrors `shell-desktop`, with DOM
/// `KeyboardEvent.code` names. A plugin's keys are asked for by name
/// ([`client::Client::asks`]) and reach it by the same names.
pub fn binding(code: &str) -> Option<Key> {
    Some(match code {
        "KeyW" | "ArrowUp" => Key::Forward,
        "KeyS" | "ArrowDown" => Key::Back,
        "KeyA" | "ArrowLeft" => Key::Left,
        "KeyD" | "ArrowRight" => Key::Right,
        "Space" => Key::Up,
        "KeyC" | "ControlLeft" => Key::Down,
        "ShiftLeft" | "ShiftRight" => Key::Sprint,
        "KeyF" => Key::ToggleMode,
        "KeyR" => Key::NewSeed,
        "KeyV" => Key::NextAvatar,
        "KeyP" => Key::FirstPerson,
        "KeyE" => Key::Act,
        "KeyQ" => Key::Whistle,
        "Enter" | "NumpadEnter" => Key::Start,
        "KeyZ" => Key::ButtonA,
        "KeyX" => Key::ButtonB,
        "Escape" => Key::Leave,
        _ => return None,
    })
}

/// A key going down or up into `input`, by the name the web gives it, with
/// what is held with it, `(down, repeat)`: `asked` whether a plugin asked
/// for it, `lent` whether the controls are. Says whether the world took it,
/// so the page leaves it be.
pub fn press(
    input: &mut Input,
    code: &str,
    chord: Chord,
    (down, repeat): (bool, bool),
    asked: bool,
    lent: bool,
) -> bool {
    input.chord = chord;
    if !down {
        // A key let go is let go whatever is held with it: Z let go under
        // Control would otherwise hold a game's A on.
        input.hold(code, false);
        if let Some(key) = binding(code) {
            input.key(key, false);
        }
    }
    // Command Z takes back, with Shift puts back, as Command Y does: the
    // core's chords, which no plugin asks for.
    if chord.command && (code == "KeyZ" || code == "KeyY") {
        if down && !repeat {
            let key = match code == "KeyY" || chord.shift {
                true => Key::Redo,
                false => Key::Undo,
            };
            input.key(key, true);
        }
        true
    } else if asked {
        // A key a plugin asked for is the plugin's, and the page has
        // nothing of its own to do with it here.
        if down && !repeat {
            input.code(code, chord, true);
        }
        true
    } else if let Some(key) = binding(code) {
        let Reach {
            input: heard,
            taken,
        } = reach(key, down, lent);
        // One shot keys must not fire again while held.
        if heard && !(down && repeat) {
            input.key(key, down);
        }
        taken
    } else {
        false
    }
}

/// Where a bound key goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reach {
    /// Whether the engine's input hears it.
    pub input: bool,
    /// Whether the world takes it from the page.
    pub taken: bool,
}

/// Where `key` going down or up goes, the controls `lent` or not. Enter,
/// Escape, Z and X are the page's on foot, Enter and Escape the chat's and
/// the panels'; lent, they are start, leave, A and B. A key let go is heard
/// whoever holds them: Enter held through a leave would otherwise hold the
/// next game's start.
pub fn reach(key: Key, down: bool, lent: bool) -> Reach {
    let lent_only = matches!(key, Key::Start | Key::Leave | Key::ButtonA | Key::ButtonB);
    let pages = lent_only && !lent;
    Reach {
        input: !(pages && down),
        taken: !pages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_escape_are_the_pages_on_foot_and_let_go_either_way() {
        let lent = |down| reach(Key::Start, down, true);
        let foot = |down| reach(Key::Start, down, false);
        assert_eq!(
            lent(true),
            Reach {
                input: true,
                taken: true
            }
        );
        assert_eq!(
            foot(true),
            Reach {
                input: false,
                taken: false
            }
        );
        assert_eq!(
            foot(false),
            Reach {
                input: true,
                taken: false
            },
            "Enter let go after leaving is let go"
        );
        assert_eq!(
            reach(Key::Act, true, false),
            Reach {
                input: true,
                taken: true
            }
        );
    }

    #[test]
    fn z_and_x_are_a_and_b_lent_and_the_pages_on_foot() {
        assert_eq!(binding("KeyZ"), Some(Key::ButtonA));
        assert_eq!(binding("KeyX"), Some(Key::ButtonB));
        for key in [Key::ButtonA, Key::ButtonB] {
            assert!(reach(key, true, true).taken && reach(key, true, true).input);
            assert!(!reach(key, true, false).taken);
        }
    }

    #[test]
    fn a_key_let_go_with_command_held_is_let_go() {
        // Z held as A while a game is played, Control pressed, Z let go: A
        // is let go with it.
        let mut input = Input::default();
        let command = Chord {
            command: true,
            shift: false,
        };
        press(
            &mut input,
            "KeyZ",
            Chord::default(),
            (true, false),
            false,
            true,
        );
        assert!(input.held(Key::ButtonA));
        press(&mut input, "KeyZ", command, (false, false), false, true);
        assert!(
            !input.held(Key::ButtonA),
            "Z let go under Control lets A go"
        );
    }
}
