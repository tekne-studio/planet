//! The reset a cabinet's players hold, as a console's button: Start held
//! [`RESET_S`] runs the game played again from its start, its cover shown
//! until its first frame. It is the cabinet's, never the game's: the game
//! is given Start held all the while, as it would be, so a Start let go
//! sooner was only its Start, and a game that reads a long Start reads it
//! until the reset. The Start that reset it is the game's no more until it
//! is let go: the game starts over with nothing held.
//!
//! A game of one pad counts the hold by the clock ([`Held`]). A game of
//! several counts it by the updates its machines agreed ([`Starts`]): each
//! machine finds the reset at the same update of the same buttons, so all
//! start the game over there and stay in step, and nothing but the buttons
//! is ever sent. Any player may hold it, and both see its ring fill.

use client::{Host, PADS, Pad, TAP_S, UPDATES_PER_S};
use serde::Serialize;

/// How long Start is held to reset the game, seconds: longer than any
/// press a game asks for, short enough to reach for.
pub(crate) const RESET_S: f64 = 2.0;

/// [`RESET_S`] in updates.
const RESET_UPDATES: u64 = (RESET_S * UPDATES_PER_S as f64) as u64;

/// Each pad's Start along the updates agreed: how long it is held toward a
/// reset, and whether it reset the game and is held still.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Starts {
    /// By pad, the updates its Start has been held through toward a reset.
    held: [u64; PADS],
    /// By pad, whether its Start reset the game and is held still: the
    /// game is given it released until it is let go.
    swallowed: [bool; PADS],
}

impl Starts {
    /// `word` as the game is given it: a Start that reset it, released.
    pub(crate) fn given(&self, word: u32) -> u32 {
        (0..PADS)
            .filter(|&pad| self.swallowed[pad])
            .fold(word, |word, pad| word & !Pad::START.at(pad))
    }

    /// `count` updates of `word` go by: after how many of them the game
    /// resets, if it does. A reset swallows every Start held then, so
    /// there is at most one in a run of one word.
    pub(crate) fn run(&mut self, word: u32, count: u64) -> Option<u64> {
        let held = |pad: usize| word & Pad::START.at(pad) != 0;
        let mut reset: Option<u64> = None;
        for pad in 0..PADS {
            if !held(pad) {
                self.held[pad] = 0;
                self.swallowed[pad] = false;
            } else if !self.swallowed[pad] {
                let to = RESET_UPDATES - self.held[pad];
                if to <= count {
                    reset = Some(reset.map_or(to, |at| at.min(to)));
                }
                self.held[pad] += count;
            }
        }
        if reset.is_some() {
            for pad in (0..PADS).filter(|&pad| held(pad)) {
                self.held[pad] = 0;
                self.swallowed[pad] = true;
            }
        }
        reset
    }

    /// How far the Start held longest has gone toward a reset
    /// ([`closed`]).
    pub(crate) fn closed(&self) -> Option<f32> {
        let held = self.held.iter().copied().max()?;
        closed(held as f64 / f64::from(UPDATES_PER_S))
    }
}

/// Start held on a game of one pad, by the clock: its one player's.
#[derive(Default, Debug)]
pub(crate) struct Held {
    /// Since when Start is held toward a reset, seconds.
    since_s: Option<f64>,
    /// Whether the Start held reset the game: the game is pressed it no
    /// more until it is let go.
    swallowed: bool,
}

impl Held {
    /// A turn at `now_s`, `pad` pressed: what the game is pressed, and
    /// whether it resets now.
    pub(crate) fn turn(&mut self, now_s: f64, pad: Pad) -> (Pad, bool) {
        let without = Pad::from_bits(pad.bits() & !Pad::START.bits());
        if !pad.has(Pad::START) {
            *self = Held::default();
            return (pad, false);
        }
        if self.swallowed {
            return (without, false);
        }
        let since_s = *self.since_s.get_or_insert(now_s);
        if now_s - since_s < RESET_S {
            return (pad, false);
        }
        *self = Held {
            since_s: None,
            swallowed: true,
        };
        (without, true)
    }

    /// How far Start held has gone toward a reset at `now_s` ([`closed`]).
    pub(crate) fn closed(&self, now_s: f64) -> Option<f32> {
        closed(now_s - self.since_s?)
    }
}

/// How far a Start held `held_s` has gone toward a reset, 0 to 1, from a
/// tap's length on, as the prompt's ring of a hold closes: a Start tapped
/// shows nothing.
fn closed(held_s: f64) -> Option<f32> {
    (held_s >= TAP_S).then(|| ((held_s - TAP_S) / (RESET_S - TAP_S)).clamp(0.0, 1.0) as f32)
}

/// The ring over the game played while Start is held toward a reset, as
/// last said to the page.
#[derive(Default)]
pub(crate) struct Ring {
    shown: bool,
}

/// The event `items.reset`: how far the ring has closed, `null` once it is
/// gone, and where the game's middle is on the view, fractions from the top
/// left.
#[derive(Serialize)]
struct Said {
    closed: Option<f32>,
    x: f64,
    y: f64,
}

impl Ring {
    /// Says the ring every turn it shows, and once as it goes.
    pub(crate) fn say(&mut self, host: &mut Host<'_>, closed: Option<f32>, [x, y]: [f64; 2]) {
        if closed.is_none() && !self.shown {
            return;
        }
        self.shown = closed.is_some();
        host.emit("reset", &Said { closed, x, y });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: u32 = Pad::START.at(0);
    const START_2: u32 = Pad::START.at(1);

    #[test]
    fn start_held_resets_once_at_its_update_and_is_given_released_until_let_go() {
        let mut starts = Starts::default();
        assert_eq!(starts.run(START, RESET_UPDATES - 1), None);
        assert_eq!(starts.given(START), START, "the game reads it held");
        assert_eq!(starts.run(START, 10), Some(1), "at the update it is due");
        assert_eq!(starts.given(START | 1), 1, "swallowed");
        assert_eq!(starts.run(START, RESET_UPDATES * 3), None, "once");
        assert_eq!(starts.run(0, 1), None);
        assert_eq!(starts.given(START), START, "let go, it is the game's");
    }

    #[test]
    fn start_let_go_sooner_counts_nothing() {
        let mut starts = Starts::default();
        for _ in 0..10 {
            assert_eq!(starts.run(START, RESET_UPDATES - 1), None);
            assert_eq!(starts.run(0, 1), None);
        }
        assert_eq!(starts.closed(), None);
    }

    #[test]
    fn either_pad_resets_and_swallows_every_start_held() {
        let mut starts = Starts::default();
        assert_eq!(starts.run(START_2, 100), None);
        assert_eq!(starts.run(START | START_2, 30), Some(20));
        assert_eq!(starts.given(START | START_2), 0);
        // Player 1 lets go and holds again: theirs counts afresh.
        assert_eq!(starts.run(START_2, 5), None);
        assert_eq!(
            starts.given(START | START_2),
            START,
            "the swallowed one only"
        );
    }

    #[test]
    fn the_ring_shows_past_a_tap_and_closes_at_the_reset() {
        let mut starts = Starts::default();
        starts.run(START, 6);
        assert_eq!(starts.closed(), None, "a tap shows nothing");
        starts.run(START, RESET_UPDATES - 7);
        assert!(starts.closed().is_some_and(|closed| closed > 0.99));
        starts.run(START, 1);
        assert_eq!(starts.closed(), None, "reset, it is gone");
    }

    #[test]
    fn a_game_of_one_pad_resets_by_the_clock_and_is_pressed_start_no_more() {
        let mut held = Held::default();
        assert_eq!(held.turn(0.0, Pad::START), (Pad::START, false));
        assert_eq!(
            held.turn(1.0, Pad::START | Pad::LEFT),
            (Pad::START | Pad::LEFT, false)
        );
        assert!(
            held.closed(1.0)
                .is_some_and(|closed| closed > 0.4 && closed < 0.5)
        );
        assert_eq!(
            held.turn(RESET_S, Pad::START | Pad::LEFT),
            (Pad::LEFT, true)
        );
        assert_eq!(held.turn(9.0, Pad::START), (Pad::NONE, false), "swallowed");
        assert_eq!(held.closed(9.0), None);
        assert_eq!(held.turn(9.5, Pad::NONE), (Pad::NONE, false));
        assert_eq!(
            held.turn(10.0, Pad::START),
            (Pad::START, false),
            "the game's again"
        );
    }
}
