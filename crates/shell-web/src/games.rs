//! The games the cartridges' worker knows, as the page keeps them: each by
//! the number the worker knows it by, with its picture, its screen's size
//! and its pace. Which games the worker ticks and whose batches go to a
//! stream is decided here, plain logic, so its tests run natively.

use std::collections::HashMap;

use scene::PictureId;

/// How a game the worker knows goes on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pace {
    /// A cover, a stop screen, or a game that stopped: never ticked, and
    /// what it said once stands.
    Still,
    /// Ticked a frame drawn, heard through its stream.
    Runs,
    /// Its player gone: ticked no more, its stream held. A batch it
    /// played before the hold may come after it, and is heard all the
    /// same: its stream lays it where it fell, and plays it once it runs
    /// on.
    Held,
}

/// A game the worker knows.
pub struct Game {
    pub picture: PictureId,
    /// The size of its screen, once the worker said it.
    pub size: Option<(u32, u32)>,
    /// Whether its texture is made: the engine heard its size a frame ago,
    /// and the frame between made it.
    pub made: bool,
    pub pace: Pace,
    /// Whether the worker said a screen of it.
    pub said: bool,
}

/// Every game the worker knows, by its number and by its picture.
#[derive(Default)]
pub struct Games {
    games: HashMap<u32, Game>,
    numbers: HashMap<PictureId, u32>,
    next: u32,
}

impl Games {
    /// The number the next game is asked under.
    pub fn next(&mut self) -> u32 {
        self.next = self.next.wrapping_add(1);
        self.next
    }

    /// Keeps the game asked under `number` into `picture`.
    pub fn insert(&mut self, number: u32, picture: PictureId, pace: Pace) {
        let game = Game {
            picture,
            size: None,
            made: false,
            pace,
            said: false,
        };
        self.games.insert(number, game);
        self.numbers.insert(picture, number);
    }

    /// The number of the game in `picture`.
    pub fn number(&self, picture: PictureId) -> Option<u32> {
        self.numbers.get(&picture).copied()
    }

    pub fn get(&self, number: u32) -> Option<&Game> {
        self.games.get(&number)
    }

    pub fn get_mut(&mut self, number: u32) -> Option<&mut Game> {
        self.games.get_mut(&number)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Game> {
        self.games.values_mut()
    }

    /// Sets the pace of the game in `picture`, running or held: its
    /// number, to tell the worker. Nothing for a game the worker does not
    /// know or that stands still.
    pub fn pace(&mut self, picture: PictureId, pace: Pace) -> Option<u32> {
        let number = self.number(picture)?;
        let game = self.games.get_mut(&number)?;
        if game.pace == Pace::Still {
            return None;
        }
        game.pace = pace;
        Some(number)
    }

    /// Forgets the game in `picture`: its number, to tell the worker.
    pub fn close(&mut self, picture: PictureId) -> Option<u32> {
        let number = self.numbers.remove(&picture)?;
        self.games.remove(&number);
        Some(number)
    }

    /// Forgets the game `number`, which failed.
    pub fn failed(&mut self, number: u32) -> Option<Game> {
        let game = self.games.remove(&number)?;
        self.numbers.remove(&game.picture);
        Some(game)
    }

    /// Forgets every game: the worker died.
    pub fn drain(&mut self) -> impl Iterator<Item = Game> + '_ {
        self.numbers.clear();
        self.games.drain().map(|(_, game)| game)
    }

    /// The picture whose stream takes what the game `number` played: one
    /// that runs, or holds.
    pub fn heard(&self, number: u32) -> Option<PictureId> {
        self.games
            .get(&number)
            .filter(|game| game.pace != Pace::Still)
            .map(|game| game.picture)
    }

    /// Whether the worker is told a frame was drawn: a game runs, or a
    /// screen is still to be said. Covers and stop screens, once said, ask
    /// no tick: an idle cabinet costs the worker nothing.
    pub fn tick(&self) -> bool {
        self.games
            .values()
            .any(|game| game.pace == Pace::Runs || !game.said)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heard::{Heard, Told};

    /// What a game's stream was given: its batches, and where it held and
    /// ran on.
    #[derive(Clone, PartialEq, Eq, Debug)]
    enum Fed {
        Batch(u32),
        Hold,
        Resume,
    }

    /// A page drawing `frames` frames of the game in picture 1, as the
    /// shell's frame takes them: first what the engine asked, the hold at
    /// frame `held.0` and the run on at `held.1`, then what the worker
    /// said since the last frame, the batch of the update the last tick
    /// ran among it. The worker runs one update a tick, numbered, and
    /// none while it holds. What the stream was given.
    fn stream(frames: u32, held: Option<(u32, u32)>) -> Vec<Fed> {
        let picture = PictureId(1);
        let mut games = Games::default();
        let number = games.next();
        games.insert(number, picture, Pace::Runs);
        let mut heard = Heard::<()>::default();
        let (mut update, mut ticks, mut fed) = (0, false, Vec::new());
        for frame in 0..frames {
            // The worker ran the tick the last frame asked for, and said
            // what the update played.
            if ticks {
                update += 1;
                heard.hear(Told::Sound {
                    number,
                    batch: vec![update],
                });
            }
            if let Some((hold, resume)) = held {
                if frame == hold && games.pace(picture, Pace::Held).is_some() {
                    fed.push(Fed::Hold);
                }
                if frame == resume && games.pace(picture, Pace::Runs).is_some() {
                    fed.push(Fed::Resume);
                }
            }
            for (number, played) in heard.take().played {
                if games.heard(number).is_some() {
                    fed.extend(played.into_iter().map(|batch| Fed::Batch(batch[0])));
                }
            }
            ticks = games
                .get(number)
                .is_some_and(|game| game.pace == Pace::Runs);
        }
        fed
    }

    #[test]
    fn a_game_held_between_an_update_and_its_batch_gives_its_stream_every_batch_as_never_held() {
        let never = stream(60, None);
        let held = stream(60 + 20, Some((30, 50)));
        let batches = |fed: &[Fed]| -> Vec<Fed> {
            fed.iter()
                .filter(|fed| matches!(fed, Fed::Batch(_)))
                .cloned()
                .collect()
        };
        assert_eq!(
            batches(&held),
            batches(&never),
            "every update heard, in order"
        );
        // The batch of the update ticked the frame before the hold comes
        // after it, and is the stream's to lay; none comes while it holds.
        let hold = held.iter().position(|fed| *fed == Fed::Hold).unwrap();
        let resume = held.iter().position(|fed| *fed == Fed::Resume).unwrap();
        assert_eq!(held[hold + 1], Fed::Batch(30));
        assert_eq!(resume, hold + 2, "{held:?}");
    }

    #[test]
    fn only_a_game_that_runs_is_ticked_and_a_still_one_is_never_heard() {
        let mut games = Games::default();
        let cover = games.next();
        games.insert(cover, PictureId(1), Pace::Still);
        assert!(games.tick(), "its screen is still to be said");
        games.get_mut(cover).unwrap().said = true;
        assert!(!games.tick());
        assert_eq!(games.heard(cover), None);
        assert_eq!(
            games.pace(PictureId(1), Pace::Runs),
            None,
            "a cover never runs"
        );
        let game = games.next();
        games.insert(game, PictureId(2), Pace::Runs);
        games.get_mut(game).unwrap().said = true;
        assert!(games.tick());
        assert_eq!(games.pace(PictureId(2), Pace::Held), Some(game));
        assert!(!games.tick(), "held, it is not ticked");
        assert_eq!(games.heard(game), Some(PictureId(2)));
        games.get_mut(game).unwrap().pace = Pace::Still;
        assert_eq!(games.heard(game), None, "stopped, it is heard no more");
        assert_eq!(games.close(PictureId(2)), Some(game));
        assert!(games.get(game).is_none() && games.number(PictureId(2)).is_none());
    }
}
