//! What the cartridges' worker said since a frame last read it, bounded
//! however long the page goes without one: the newest screen of each game,
//! the last batches each played, every answer to an open, in order, and
//! why the worker died, once it did. Plain logic, generic over a screen, so
//! its tests run natively.

use std::collections::{HashMap, VecDeque};

/// The most batches of a game kept between two frames. The worker ticks
/// once a frame, a batch a tick, so a quarter of a second of frames, as far
/// ahead as the game's sound lays them; older ones would sound late, and
/// are let go.
pub const KEPT_BATCHES: usize = 15;

/// The worker's answer to an open: the size of the game's screen, or why it
/// does not run.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Answer {
    Opened {
        number: u32,
        width: u32,
        height: u32,
        /// The pads its file plays, when it was read from one.
        plays: Option<usize>,
    },
    Failed {
        number: u32,
        message: String,
    },
    /// The game stopped for good, its screen saying why.
    Stopped {
        number: u32,
    },
}

/// One thing the worker says, its screens of type `S`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Told<S> {
    Answer(Answer),
    Screen {
        number: u32,
        screen: S,
    },
    /// What a game played since the last tick, as the chip's processor
    /// takes it: the emulator's batch, its words never read here.
    Sound {
        number: u32,
        batch: Vec<u32>,
    },
    /// A screen read, what an agent reads.
    Read {
        number: u32,
        read: Read,
    },
    /// It broke, and runs nothing more.
    Dead(String),
}

/// A game's screen as it wrote it: its palette, and a byte a pixel, each a
/// colour of it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Read {
    pub palette: Vec<[u8; 4]>,
    pub pixels: Vec<u8>,
}

/// What the worker said, kept until a frame takes it.
#[derive(Debug)]
pub struct Heard<S> {
    pub answers: Vec<Answer>,
    pub screens: HashMap<u32, S>,
    /// By game, what it played, a batch a tick, in order: the last
    /// [`KEPT_BATCHES`].
    pub played: HashMap<u32, VecDeque<Vec<u32>>>,
    /// The newest screen read of each game.
    pub reads: HashMap<u32, Read>,
    pub dead: Option<String>,
}

impl<S> Default for Heard<S> {
    fn default() -> Self {
        Self {
            answers: Vec::new(),
            screens: HashMap::new(),
            played: HashMap::new(),
            reads: HashMap::new(),
            dead: None,
        }
    }
}

impl<S> Heard<S> {
    /// Keeps what the worker said: a screen in place of the one before it.
    pub fn hear(&mut self, told: Told<S>) {
        match told {
            Told::Answer(answer) => self.answers.push(answer),
            Told::Screen { number, screen } => {
                self.screens.insert(number, screen);
            }
            Told::Sound { number, batch } => {
                let kept = self.played.entry(number).or_default();
                if kept.len() == KEPT_BATCHES {
                    kept.pop_front();
                }
                kept.push_back(batch);
            }
            Told::Read { number, read } => {
                self.reads.insert(number, read);
            }
            Told::Dead(why) => {
                self.dead.get_or_insert(why);
            }
        }
    }

    /// Everything heard so far, leaving nothing.
    pub fn take(&mut self) -> Self {
        core::mem::take(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_without_frames_keeps_one_screen_a_game_and_every_answer() {
        let mut heard = Heard::default();
        heard.hear(Told::Answer(Answer::Opened {
            number: 1,
            width: 160,
            height: 120,
            plays: Some(1),
        }));
        for tick in 0..10_000 {
            for number in [1, 2] {
                heard.hear(Told::Screen {
                    number,
                    screen: tick,
                });
            }
        }
        heard.hear(Told::Answer(Answer::Failed {
            number: 2,
            message: "gone".into(),
        }));
        let taken = heard.take();
        assert_eq!(taken.screens.len(), 2);
        assert_eq!(taken.screens[&1], 9_999);
        assert_eq!(taken.screens[&2], 9_999);
        assert!(matches!(
            taken.answers.as_slice(),
            [
                Answer::Opened { number: 1, .. },
                Answer::Failed { number: 2, .. }
            ]
        ));
        assert!(heard.screens.is_empty() && heard.answers.is_empty());
    }

    #[test]
    fn each_games_batches_are_kept_in_order_the_last_few() {
        let mut heard = Heard::<()>::default();
        for tick in 1..=1_000 {
            heard.hear(Told::Sound {
                number: 1,
                batch: vec![60, tick],
            });
        }
        heard.hear(Told::Sound {
            number: 2,
            batch: vec![60, 0],
        });
        let taken = heard.take();
        let kept: Vec<u32> = taken.played[&1].iter().map(|batch| batch[1]).collect();
        assert_eq!(
            kept,
            (986..=1_000).collect::<Vec<_>>(),
            "the last, in order"
        );
        assert_eq!(kept.len(), KEPT_BATCHES);
        assert_eq!(taken.played[&2].len(), 1);
        assert!(heard.take().played.is_empty(), "taken once");
    }

    #[test]
    fn a_worker_dies_once_for_the_first_reason() {
        let mut heard = Heard::<()>::default();
        heard.hear(Told::Dead("first".into()));
        heard.hear(Told::Dead("second".into()));
        assert_eq!(heard.take().dead.as_deref(), Some("first"));
    }
}
