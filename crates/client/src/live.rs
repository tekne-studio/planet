//! Live pictures: textures the shell writes into every frame from a source
//! it plays, a moving picture's file it decodes ([`crate::pictures`]) or a
//! cartridge it runs ([`crate::carts`]). One budget holds them all, so a
//! wall of videos and a row of cabinets share what a phone can write: a few
//! at once, the tallest on the screen first, and either kind at most
//! [`ONE_KIND_AT_MOST`] of them, so the other always has a place.
//!
//! Each owner says, as it draws, which of its sources it wants live this
//! frame; at the frame's end this orders what opens and what closes, and
//! the owner turns each order into its shell's message and its texture. A
//! source unwanted for a second is closed, unless its owner holds it
//! ([`Live::hold`]): kept open with nobody wanting it, and the first to give
//! its place to one wanted when there is no room. An open the shell never
//! answers is given up; one that failed waits before it is asked again, twice as
//! long each time it fails again, and after [`GIVE_UP_AFTER`] failures in a
//! row it is given up while the world lasts, so a file that is gone is not
//! asked for for good.

use std::collections::HashMap;

use part::ContentId;

/// How many live pictures there are at once, moving pictures and games
/// together. Each costs a decoder or a slice of the runners' thread, and a
/// texture written every frame on the frame's thread: a phone's browser
/// allows few decoders, and four games at sixty ticks a second keep its
/// second core under a sixth busy at the worst (`rocks` takes 0.009 ms a
/// tick on the browser's engine, the worst a cartridge may do within its
/// budget 0.6 ms).
pub const LIVE_AT_ONCE: usize = 4;

/// How many of [`LIVE_AT_ONCE`] one kind holds at most, moving pictures or
/// games: the split between them. A wall of videos never stills every
/// cabinet in view, nor a row of cabinets every video.
pub(crate) const ONE_KIND_AT_MOST: usize = 3;

/// Frames a live picture stays open unwanted: a second at sixty. A look
/// away and back costs no reopening; a walk away frees its place soon.
pub(crate) const UNWANTED_FRAMES: u64 = 60;

/// Frames an open waits for the shell's answer before it is given up:
/// fifteen seconds at sixty. A source that never answers would otherwise
/// hold its place for good.
pub(crate) const OPEN_FRAMES: u64 = 900;

/// Frames a source that failed waits before it is asked again: ten seconds
/// at sixty the first time, twice as long each time after.
pub(crate) const RETRY_FRAMES: u64 = 600;

/// Failures in a row after which a source is given up: tried over ten
/// minutes and a half, the last wait five minutes and twenty seconds.
pub(crate) const GIVE_UP_AFTER: u32 = 7;

/// What a live picture plays.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub(crate) enum Source {
    /// A moving picture, by its original's content id.
    Moving(ContentId),
    /// A cabinet's game: the cabinet, by this client's name of the part,
    /// and the cartridge it runs. A cabinet given another cartridge plays
    /// another game.
    Game(u64, ContentId),
}

/// What the owner of a source does at the frame's end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Order {
    /// Ask the shell to play it, into a texture the owner names.
    Open(Source),
    /// Stop it, answered or not.
    Close(Source),
}

impl Source {
    fn is_game(self) -> bool {
        matches!(self, Source::Game(..))
    }
}

/// A source open: asked for, or playing.
struct Feed {
    /// The frame it was asked for in, until the shell answers.
    asked: Option<u64>,
    /// The frame it was last wanted in.
    wanted: u64,
    /// Whether its owner holds it open while nobody wants it.
    held: bool,
}

/// A source that failed: when last, and how many times running.
#[derive(Default)]
pub(crate) struct Failure {
    at: u64,
    times: u32,
}

impl Failure {
    /// Frames to wait before it is asked again; given up, for ever.
    fn wait(&self) -> u64 {
        match self.times < GIVE_UP_AFTER {
            true => RETRY_FRAMES << self.times.saturating_sub(1),
            false => u64::MAX,
        }
    }

    /// It failed once more, in `frame`.
    pub(crate) fn again(&mut self, frame: u64) {
        self.at = frame;
        self.times += 1;
    }

    /// Whether it still waits in `frame` before it is asked again: for
    /// ever once given up.
    pub(crate) fn waits(&self, frame: u64) -> bool {
        frame.saturating_sub(self.at) < self.wait()
    }

    /// Whether it failed [`GIVE_UP_AFTER`] times in a row.
    pub(crate) fn given_up(&self) -> bool {
        self.times >= GIVE_UP_AFTER
    }
}

/// The live pictures of this client, of every owner.
#[derive(Default)]
pub(crate) struct Live {
    feeds: HashMap<Source, Feed>,
    /// Wanted this frame and not open: how tall on the screen, and which.
    /// Opened at the frame's end, tallest first.
    wanting: Vec<(f64, Source)>,
    failed: HashMap<Source, Failure>,
    frame: u64,
}

impl Live {
    /// `source` is wanted live this frame, `px` tall on the screen: kept
    /// open, or opened at the frame's end while there is room, unless it
    /// failed a while ago or was given up.
    pub(crate) fn want(&mut self, source: Source, px: f64) {
        if let Some(feed) = self.feeds.get_mut(&source) {
            feed.wanted = self.frame;
            feed.held = false;
        } else if !self.failing(source) && self.wanting.iter().all(|(_, s)| *s != source) {
            self.wanting.push((px, source));
        }
    }

    /// Holds `source` open while nobody wants it, until it is wanted again
    /// or its owner lets it go: it costs its place and no more, and gives
    /// that place to a source wanted when there is no other, the one held
    /// longest first, closed by an order as one unwanted is. Nothing for a
    /// source not open.
    pub(crate) fn hold(&mut self, source: Source) {
        if let Some(feed) = self.feeds.get_mut(&source) {
            feed.held = true;
        }
    }

    /// Wants `source` again from this frame on, held no more: nothing for
    /// a source not open.
    pub(crate) fn resume(&mut self, source: Source) {
        if let Some(feed) = self.feeds.get_mut(&source) {
            feed.wanted = self.frame;
            feed.held = false;
        }
    }

    /// Whether `source` is open: asked for, or playing.
    pub(crate) fn is_open(&self, source: Source) -> bool {
        self.feeds.contains_key(&source)
    }

    /// Whether `source` is playing: open, and the shell answered.
    pub(crate) fn is_playing(&self, source: Source) -> bool {
        self.feeds
            .get(&source)
            .is_some_and(|feed| feed.asked.is_none())
    }

    /// Whether `source` failed [`GIVE_UP_AFTER`] times in a row and is no
    /// longer asked for: its owner says so where it is seen.
    pub(crate) fn given_up(&self, source: Source) -> bool {
        self.failed.get(&source).is_some_and(Failure::given_up)
    }

    fn failing(&self, source: Source) -> bool {
        self.failed
            .get(&source)
            .is_some_and(|failure| failure.waits(self.frame))
    }

    fn fail(&mut self, source: Source) {
        self.failed.entry(source).or_default().again(self.frame);
    }

    /// The shell's answer to an open of `source`: whether it plays. One
    /// that does not, or that stops playing because what played it died,
    /// is closed and waits before it is asked again. Whether the answer is
    /// one to keep: an answer to a source closed meanwhile, or a second
    /// one, is nothing.
    pub(crate) fn answered(&mut self, source: Source, plays: bool) -> bool {
        let Some(feed) = self.feeds.get_mut(&source) else {
            return false;
        };
        if plays {
            let first = feed.asked.take().is_some();
            if first {
                self.failed.remove(&source);
            }
            first
        } else {
            self.feeds.remove(&source);
            self.fail(source);
            true
        }
    }

    /// The owner let `source` go on its own, its place freed: a texture its
    /// own budget dropped.
    pub(crate) fn closed(&mut self, source: Source) {
        self.feeds.remove(&source);
    }

    /// Closes a frame: what went unwanted too long is closed, but what is
    /// held, and what the shell never answered, which waits as a failure
    /// does; what is wanted is opened, the tallest on the screen first,
    /// while there is room and its kind holds fewer than
    /// [`ONE_KIND_AT_MOST`], a source held giving its place when that is
    /// the only room.
    /// The orders, for the owners to carry out in turn. Called once every
    /// frame after every owner drew.
    pub(crate) fn end_frame(&mut self) -> Vec<Order> {
        let frame = self.frame;
        let mut orders = Vec::new();
        let mut gone: Vec<(Source, bool)> = self
            .feeds
            .iter()
            .filter_map(|(&source, feed)| {
                let late = feed
                    .asked
                    .is_some_and(|asked| frame.saturating_sub(asked) > OPEN_FRAMES);
                let unwanted = !feed.held && frame.saturating_sub(feed.wanted) > UNWANTED_FRAMES;
                (late || unwanted).then_some((source, late))
            })
            .collect();
        gone.sort_by_key(|(source, _)| *source);
        for (source, late) in gone {
            self.feeds.remove(&source);
            if late {
                self.fail(source);
            }
            orders.push(Order::Close(source));
        }
        self.wanting
            .sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(core::cmp::Ordering::Equal));
        let mut games = self.feeds.keys().filter(|source| source.is_game()).count();
        let mut moving = self.feeds.len() - games;
        for (_, source) in core::mem::take(&mut self.wanting) {
            let kind_full = match source.is_game() {
                true => games >= ONE_KIND_AT_MOST,
                false => moving >= ONE_KIND_AT_MOST,
            };
            if kind_full || games + moving >= LIVE_AT_ONCE {
                // Its kind full, only one of its kind frees a place for it.
                let Some(given) =
                    self.held_longest(|held| !kind_full || held.is_game() == source.is_game())
                else {
                    continue;
                };
                self.feeds.remove(&given);
                orders.push(Order::Close(given));
                match given.is_game() {
                    true => games -= 1,
                    false => moving -= 1,
                }
            }
            match source.is_game() {
                true => games += 1,
                false => moving += 1,
            }
            let feed = Feed {
                asked: Some(frame),
                wanted: frame,
                held: false,
            };
            self.feeds.insert(source, feed);
            orders.push(Order::Open(source));
        }
        self.frame += 1;
        orders
    }

    /// The source held the longest of those `may` takes: the one wanted
    /// last the earliest, by its name among those alike.
    fn held_longest(&self, may: impl Fn(Source) -> bool) -> Option<Source> {
        self.feeds
            .iter()
            .filter(|&(&source, feed)| feed.held && may(source))
            .min_by_key(|&(&source, feed)| (feed.wanted, source))
            .map(|(&source, _)| source)
    }

    /// Forgets every source: the world they stood in is gone, and each
    /// owner closes its own.
    pub(crate) fn clear(&mut self) {
        self.feeds.clear();
        self.wanting.clear();
        self.failed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moving(n: u8) -> Source {
        Source::Moving(ContentId([n; 32]))
    }

    fn game(n: u64) -> Source {
        Source::Game(n, ContentId([0; 32]))
    }

    fn opened(orders: &[Order]) -> Vec<Source> {
        orders
            .iter()
            .filter_map(|order| match order {
                Order::Open(source) => Some(*source),
                Order::Close(_) => None,
            })
            .collect()
    }

    #[test]
    fn a_few_are_live_at_once_the_tallest_first_and_either_kind_leaves_the_other_a_place() {
        let mut live = Live::default();
        for n in 0..4u8 {
            live.want(moving(n), 100.0 + f64::from(n));
            // Wanted twice in a frame, it is opened once.
            live.want(game(n.into()), 200.0 + f64::from(n));
            live.want(game(n.into()), 200.0 + f64::from(n));
        }
        let orders = live.end_frame();
        assert_eq!(opened(&orders), [game(3), game(2), game(1), moving(3)]);
        // Nothing more opens while they are open, answered or not.
        live.want(moving(9), 1000.0);
        assert!(live.end_frame().is_empty());

        // One kind alone takes every place it may.
        let mut games = Live::default();
        for n in 0..4 {
            games.want(game(n), 100.0 + n as f64);
        }
        assert_eq!(opened(&games.end_frame()), [game(3), game(2), game(1)]);
    }

    #[test]
    fn one_unwanted_a_second_is_closed_and_its_place_freed() {
        let mut live = Live::default();
        live.want(game(1), 100.0);
        live.end_frame();
        assert!(live.answered(game(1), true));
        assert!(!live.answered(game(1), true), "a second answer is nothing");
        for _ in 0..UNWANTED_FRAMES {
            assert!(live.end_frame().is_empty());
        }
        assert_eq!(live.end_frame(), [Order::Close(game(1))]);
        assert!(!live.is_open(game(1)));
        live.want(game(1), 100.0);
        assert_eq!(live.end_frame(), [Order::Open(game(1))]);
    }

    #[test]
    fn one_held_stays_open_unwanted_until_wanted_again() {
        let mut live = Live::default();
        live.want(game(1), 100.0);
        live.end_frame();
        assert!(live.answered(game(1), true));
        live.hold(game(1));
        for _ in 0..UNWANTED_FRAMES * 10 {
            assert!(live.end_frame().is_empty(), "held, never closed unwanted");
        }
        // Wanted again, it is held no more: unwanted, it closes as any.
        live.want(game(1), 100.0);
        for _ in 0..=UNWANTED_FRAMES {
            assert!(live.end_frame().is_empty());
        }
        assert_eq!(live.end_frame(), [Order::Close(game(1))]);
    }

    #[test]
    fn one_held_gives_its_place_to_one_wanted_the_longest_held_first() {
        let mut live = Live::default();
        for n in 0..3 {
            live.want(game(n), 100.0);
        }
        live.end_frame();
        // Held one after another: the second game a frame before the third.
        live.hold(game(1));
        live.want(game(0), 100.0);
        live.want(game(2), 100.0);
        live.end_frame();
        live.hold(game(2));
        live.want(game(0), 100.0);
        live.want(game(3), 100.0);
        assert_eq!(
            live.end_frame(),
            [Order::Close(game(1)), Order::Open(game(3))],
            "the games' places are full: one held gives its own"
        );
        // A moving picture takes the last place free, and the next one the
        // place of the game held.
        live.want(moving(1), 100.0);
        assert_eq!(live.end_frame(), [Order::Open(moving(1))]);
        live.want(moving(2), 100.0);
        assert_eq!(
            live.end_frame(),
            [Order::Close(game(2)), Order::Open(moving(2))]
        );
        // Nothing held is left: what is wanted waits as before.
        live.want(moving(3), 100.0);
        assert!(live.end_frame().is_empty());
    }

    #[test]
    fn an_open_never_answered_is_given_up_and_waits() {
        let mut live = Live::default();
        live.want(moving(1), 100.0);
        live.end_frame();
        for _ in 0..OPEN_FRAMES {
            live.want(moving(1), 100.0);
            assert!(live.end_frame().is_empty());
        }
        live.want(moving(1), 100.0);
        assert_eq!(live.end_frame(), [Order::Close(moving(1))]);
        assert!(!live.answered(moving(1), true), "a late answer is nothing");
        live.want(moving(1), 100.0);
        assert!(live.end_frame().is_empty(), "it waits as a failure does");
    }

    #[test]
    fn one_that_fails_waits_twice_as_long_each_time_and_is_given_up_at_last() {
        // Frames until it is opened again, wanted every frame.
        fn reopened(live: &mut Live) -> u64 {
            let mut frames = 0;
            loop {
                live.want(game(1), 100.0);
                if !opened(&live.end_frame()).is_empty() {
                    return frames;
                }
                frames += 1;
            }
        }
        let mut live = Live::default();
        let waited: Vec<u64> = (0..GIVE_UP_AFTER)
            .map(|_| {
                assert!(!live.given_up(game(1)));
                let frames = reopened(&mut live);
                assert!(live.answered(game(1), false));
                frames
            })
            .collect();
        assert_eq!(waited, [0, 600, 1200, 2400, 4800, 9600, 19200]);
        assert!(live.given_up(game(1)));
        for _ in 0..4 * 19200 {
            live.want(game(1), 100.0);
            assert!(live.end_frame().is_empty(), "given up, never asked again");
        }
        // Until the world is gone.
        live.clear();
        assert!(!live.given_up(game(1)));
        assert_eq!(reopened(&mut live), 0);
        assert!(live.answered(game(1), false));
        // One that played, then stopped when what played it died, waits as
        // after a first failure.
        reopened(&mut live);
        assert!(live.answered(game(1), true));
        assert!(live.answered(game(1), false));
        assert_eq!(reopened(&mut live), RETRY_FRAMES);
    }
}
