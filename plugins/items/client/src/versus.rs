//! A game of several pads played at one cabinet, each player on their own
//! machine. Every machine runs the game from the same file and seed, and
//! runs an update only once it holds every pad's buttons for it: the same
//! buttons make the same frames, so nothing else is ever sent, never a
//! frame, never a state.
//!
//! Each holder of a station writes its own pad's buttons [`DELAY`] updates
//! ahead of the update due by its clock, and relays them. The world relays
//! what each holder of the cabinet sends to every holder, the sender too,
//! in the one order it takes them in, stamped by its clock, so each machine
//! reads the same bytes in the same order: the first byte written for a
//! pad's update is its buttons, and a later one for it is dropped. What a
//! holder writes counts once it comes back from the world, on its own
//! machine as on the others.
//!
//! A holder gone quiet holds the others [`WAIT_S`] at most: then they write
//! its pad released, nothing pressed, and run on. Such bytes count only when
//! the world's stamps say its holder wrote nothing for as long, so a peer
//! never releases the pad of a holder who plays; one who writes again is
//! heard, wherever its bytes fall, and claims its pad back past them. A machine that fell behind, hidden or
//! slow, runs what it missed at once and goes on.
//!
//! Alone at the cabinet, a player runs the game with no delay and relays
//! nothing, and the game holds while they are away. One who sits beside
//! them is sent how the game ran so far, its buttons as runs ([`Runs`]),
//! runs it to the same update, and the two go on together: a game is
//! joined where it is, never started again. A game of two does not hold for
//! one who stepped away or whose window lost the keys: their machine writes
//! their pad released, and the other plays on.
//!
//! A Start held long enough resets the game ([`crate::reset`]): each machine
//! finds the reset in the buttons agreed, at the same update, and the game
//! here starts over from there. The history keeps every update as pressed,
//! so one who sits beside the game finds the same resets in it.
//!
//! Nothing a peer sends is trusted to be well made: bytes past a pad's
//! bits, a pad its sender does not hold, a gap, the past, a flood of the
//! future and a Start never asked for are each dropped, and what a machine
//! keeps is bounded by [`AHEAD_MOST`], [`BEHIND_MOST`] and the console's
//! longest game.

use std::collections::VecDeque;

use client::{GAME_LONGEST, PADS, Pad, RELAY_BYTES, UPDATES_PER_S};
use items_world::wire::{self, versus};
use prost::Message;

use crate::reset::Starts;

/// How many updates ahead of the one due a holder writes its buttons: what
/// it presses now is pressed in the game this many updates later, 100 ms.
/// A world on the same Wi-Fi answers well within it, and a phone on a
/// mobile link, a hundred milliseconds there and back, about within it: a
/// slower answer shows the game that much later, at its pace all the same.
pub(crate) const DELAY: u64 = 6;

/// How long a machine waits for a holder who wrote nothing new before it
/// writes that holder's pad released and runs on, seconds: far past a
/// link's jitter, short enough that the game does not seem to stop.
pub(crate) const WAIT_S: f64 = 0.5;

/// [`WAIT_S`] on the world's clock, milliseconds.
const WAIT_MS: u64 = (WAIT_S * 1000.0) as u64;

/// How long a pad's bytes may wait to come back before they are sent again,
/// seconds: the world drops what a session sends past its rate.
const RESEND_S: f64 = 1.0;

/// How long one who sat beside a game waits to be sent how it ran, seconds:
/// past it, nobody sends it, and they give up their station.
pub(crate) const START_WAITS_S: f64 = 10.0;

/// How far past the update every pad has bytes for a pad's bytes may reach:
/// past it they are dropped, so a flood of the future holds no memory.
/// Wider than a holder ever writes ahead.
const AHEAD_MOST: u64 = UPDATES_PER_S as u64;

/// How far the update due by a machine's clock may run past the update
/// every pad has bytes for: a holder whose bytes come late shows the game
/// later, at its pace, until they are this late, and then holds it.
const LAG_MOST: u64 = 2 * DELAY;

/// How far behind the update every pad has bytes for a machine may fall
/// before it gives up its station, a minute: one whose page was hidden
/// longer comes back to a game gone on without it, and rises rather than
/// keep and run that much at once.
const BEHIND_MOST: u64 = 60 * UPDATES_PER_S as u64;

/// How far behind the update due a game runs what it missed at once, in
/// place of at its pace: more than a link holds in flight.
pub(crate) const BEHIND_AT_ONCE: u64 = 2 * DELAY;

/// The most updates the clock owes a game played alone at once: one held a
/// moment runs a few at once after, and no more.
const OWED_MOST: f64 = 4.0;

/// The runs a part of a Start carries: two numbers of at most five bytes
/// each, within a relay's bytes with room to spare.
const RUNS_A_PART: usize = (RELAY_BYTES - 64) / 10;

/// The parts of a Start relayed in one turn: the rest go in the turns
/// after, within the world's rate.
const PARTS_A_TURN: usize = 4;

/// A game's buttons, an update's word each, as runs of one word: how a game
/// ran, kept to hand to one who sits beside it.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub(crate) struct Runs {
    runs: Vec<(u32, u64)>,
    len: u64,
}

impl Runs {
    /// The updates it holds.
    pub(crate) fn len(&self) -> u64 {
        self.len
    }

    fn extend(&mut self, word: u32, count: u64) {
        if count == 0 {
            return;
        }
        match self.runs.last_mut() {
            Some((last, held)) if *last == word => *held += count,
            _ => self.runs.push((word, count)),
        }
        self.len += count;
    }

    /// The words of the updates from `from` to `to`, found from the end: a
    /// turn reads the last few, at a cost that does not grow with the game.
    pub(crate) fn words(&self, from: u64, to: u64) -> Vec<u32> {
        let to = to.min(self.len);
        let mut words = Vec::with_capacity(to.saturating_sub(from) as usize);
        if from >= to {
            return words;
        }
        // Back from the end to the run `from` falls in.
        let (mut first, mut at) = (self.runs.len(), self.len);
        while first > 0 && at > from {
            first -= 1;
            at -= self.runs[first].1;
        }
        for &(word, count) in &self.runs[first..] {
            let (start, end) = (at.max(from), (at + count).min(to));
            if start < end {
                words.extend(core::iter::repeat_n(word, (end - start) as usize));
            }
            at += count;
            if at >= to {
                break;
            }
        }
        words
    }

    /// How many of their first updates `self` and `other` agree on.
    pub(crate) fn agrees(&self, other: &Runs) -> u64 {
        let (mut mine, mut theirs) = (self.runs.iter().copied(), other.runs.iter().copied());
        let (mut a, mut b) = (mine.next(), theirs.next());
        let mut agreed = 0;
        while let (Some((word, count)), Some((other, others))) = (a, b) {
            if word != other {
                break;
            }
            let both = count.min(others);
            agreed += both;
            a = if count > both {
                Some((word, count - both))
            } else {
                mine.next()
            };
            b = if others > both {
                Some((other, others - both))
            } else {
                theirs.next()
            };
        }
        agreed
    }

    /// Itself as the parts of a Start for the session `to`, each within a
    /// relay's bytes; a run longer than a count holds is split.
    fn parts(&self, to: u32) -> Vec<Vec<u8>> {
        let most = u64::from(u32::MAX);
        let runs: Vec<(u32, u32)> = self
            .runs
            .iter()
            .flat_map(|&(word, count)| {
                let rest = (count % most) as u32;
                core::iter::repeat_n((word, u32::MAX), (count / most) as usize)
                    .chain((rest > 0).then_some((word, rest)))
            })
            .collect();
        let chunks: Vec<&[(u32, u32)]> = match runs.is_empty() {
            true => vec![&[]],
            false => runs.chunks(RUNS_A_PART).collect(),
        };
        let parts = chunks.len() as u32;
        (0..)
            .zip(chunks)
            .map(|(part, runs)| {
                encode(versus::Message::Start(wire::Start {
                    to,
                    at: self.len,
                    part,
                    parts,
                    buttons: runs.iter().map(|&(word, _)| word).collect(),
                    counts: runs.iter().map(|&(_, count)| count).collect(),
                }))
            })
            .collect()
    }
}

/// How a game ran: every update agreed, as its players pressed it, and
/// those since the game last started over, as it is given them.
#[derive(Clone, Default, Debug)]
struct Ran {
    history: Runs,
    game: Runs,
    /// Each pad's Start along the history.
    starts: Starts,
    /// The update of the history the game last started over at.
    start: u64,
}

impl Ran {
    /// The game as it ran through `history`, its resets found again.
    fn new(history: &Runs) -> Ran {
        let mut ran = Ran::default();
        for &(word, count) in &history.runs {
            ran.give(word, count);
        }
        ran
    }

    /// `count` updates of `word` agreed, given to the game: whether a Start
    /// held reset it among them, the game started over there.
    fn give(&mut self, word: u32, count: u64) -> bool {
        self.history.extend(word, count);
        let given = self.starts.given(word);
        let Some(at) = self.starts.run(word, count) else {
            self.game.extend(given, count);
            return false;
        };
        let rest = count - at;
        self.start = self.history.len() - rest;
        self.game = Runs::default();
        self.game.extend(self.starts.given(word), rest);
        true
    }
}

fn encode(message: versus::Message) -> Vec<u8> {
    wire::Versus {
        message: Some(message),
    }
    .encode_to_vec()
}

/// Who holds each station of a cabinet: a session, or 0 for none.
pub(crate) type Holders = [u32; PADS];

/// One machine's part in a game played at one cabinet: the station it
/// holds, how the game ran, and what it agreed with the others' machines.
#[derive(Debug)]
pub(crate) struct Versus {
    me: u32,
    mine: usize,
    /// The pads the game reads.
    plays: usize,
    holders: Holders,
    mode: Mode,
    ran: Ran,
    /// How many of the game's first updates still hold since a Start
    /// replaced the history or a reset started it over: a game here that
    /// ran more starts over. `None` while all it ran is the game's.
    kept: Option<u64>,
    /// The update due by this machine's clock, and part of the next.
    clock: f64,
    /// When the last turn was, seconds.
    last_s: Option<f64>,
    /// By pad, when its holder last wrote a byte new to its pad: on the
    /// world's clock, milliseconds, and on this machine's, seconds. `None`
    /// since it took the station.
    heard_ms: [Option<u64>; PADS],
    heard_s: [Option<f64>; PADS],
    /// The parts of a Start still to relay.
    out: VecDeque<Vec<u8>>,
    /// Given up: it holds its station no longer.
    lost: bool,
}

#[derive(Debug)]
enum Mode {
    /// The one holder of the game's pads: no delay, nothing relayed.
    Alone,
    /// Sat beside a game: waits to be sent how it ran.
    Waiting {
        since_s: Option<f64>,
        start: Option<Gathering>,
    },
    /// Played with others.
    Together(Lockstep),
}

/// A Start as its parts come.
#[derive(Debug)]
struct Gathering {
    from: u32,
    at: u64,
    parts: u32,
    next: u32,
    runs: Runs,
}

/// The buttons agreed and being agreed, from the update the game is given
/// next on.
#[derive(Debug)]
struct Lockstep {
    /// The words of the updates from the history's end on, each pad's byte
    /// in its place once one is written.
    ahead: VecDeque<u32>,
    /// By pad, the next update it has no byte for.
    front: [u64; PADS],
    /// By pad this machine writes, the next update it sent no byte for.
    sent: [u64; PADS],
    /// By pad, its front as last seen and since when it stands there.
    stands: [(u64, f64); PADS],
    /// By pad, since when the game here waits on it and its holder wrote
    /// nothing new.
    waited_s: [Option<f64>; PADS],
    /// By pad, whether its holder went quiet: this machine writes it
    /// released until it writes again.
    absent: [bool; PADS],
    /// Whether another wrote this machine's pad: it claims it back.
    filled: bool,
}

impl Lockstep {
    fn new(at: u64) -> Lockstep {
        Lockstep {
            ahead: VecDeque::new(),
            front: [at; PADS],
            sent: [at; PADS],
            stands: [(at, f64::NEG_INFINITY); PADS],
            waited_s: [None; PADS],
            absent: [false; PADS],
            filled: false,
        }
    }

    /// The update every pad in play has bytes for.
    fn agreed(&self, holders: Holders, plays: usize) -> u64 {
        (0..plays)
            .filter(|&pad| holders[pad] != 0)
            .map(|pad| self.front[pad])
            .min()
            .unwrap_or(0)
    }

    /// `pad`'s bytes from where this machine last sent up to `until`, each
    /// `bits`: from where they stand again when what was sent past it did
    /// not come back a while.
    fn write(&mut self, pad: usize, until: u64, bits: u8, now_s: f64) -> Option<wire::Pad> {
        let front = self.front[pad];
        if self.stands[pad].0 != front {
            self.stands[pad] = (front, now_s);
        }
        let stuck = now_s - self.stands[pad].1 >= RESEND_S;
        if self.sent[pad] < front || (self.sent[pad] > front && stuck) {
            self.sent[pad] = front;
            self.stands[pad].1 = now_s;
        }
        let from = self.sent[pad];
        if from >= until {
            return None;
        }
        self.sent[pad] = until;
        Some(wire::Pad {
            pad: pad as u32,
            from,
            buttons: vec![bits; (until - from) as usize],
        })
    }
}

impl Versus {
    /// The session `me` sat at station `mine` of a game that reads `plays`
    /// pads, held as `holders` say, where the game here ran `ran`. Alone,
    /// it plays on from there at once; beside another, it waits to be sent
    /// how the game ran.
    pub(crate) fn new(me: u32, mine: usize, plays: usize, holders: Holders, ran: Runs) -> Versus {
        let ran = Ran::new(&ran);
        let mut versus = Versus {
            me,
            mine,
            plays: plays.min(PADS),
            holders,
            mode: Mode::Alone,
            clock: ran.history.len() as f64,
            ran,
            kept: None,
            last_s: None,
            heard_ms: [None; PADS],
            heard_s: [None; PADS],
            out: VecDeque::new(),
            lost: holders.get(mine) != Some(&me),
        };
        if versus.others().next().is_some() {
            versus.mode = Mode::Waiting {
                since_s: None,
                start: None,
            };
        }
        versus
    }

    /// The pads in play held by others.
    fn others(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.plays).filter(|&pad| pad != self.mine && self.holders[pad] != 0)
    }

    /// The station this machine holds.
    pub(crate) fn station(&self) -> usize {
        self.mine
    }

    /// The updates agreed.
    pub(crate) fn fed(&self) -> u64 {
        self.ran.history.len()
    }

    /// The buttons of every update agreed, in order, as pressed.
    pub(crate) fn history(&self) -> &Runs {
        &self.ran.history
    }

    /// The buttons of the updates since the game last started over, as it
    /// is given them: what the game here runs.
    pub(crate) fn game(&self) -> &Runs {
        &self.ran.game
    }

    /// How far a Start held toward a reset has gone, by the updates agreed:
    /// every machine shows the same.
    pub(crate) fn closed(&self) -> Option<f32> {
        self.ran.starts.closed()
    }

    /// Whether a game here given `given` updates runs on: all it ran is the
    /// game's. One that does not starts over.
    pub(crate) fn runs_on(&self, given: u64) -> bool {
        given <= self.kept.unwrap_or(self.ran.game.len())
    }

    /// The game here was given all of the history up to now.
    pub(crate) fn given_all(&mut self) {
        self.kept = None;
    }

    /// Whether it gave up its station: waited too long to be sent the game,
    /// fell too far behind, or its station went to another.
    pub(crate) fn lost(&self) -> bool {
        self.lost
    }

    /// Who holds the stations now, in its place among what was relayed. One
    /// who sat beside a game this machine played alone is sent how it ran;
    /// one who rose leaves this machine alone; a station of this machine's
    /// given to another is lost.
    pub(crate) fn stations(&mut self, holders: Holders) {
        if self.lost {
            return;
        }
        let before = core::mem::replace(&mut self.holders, holders);
        for pad in (0..PADS).filter(|&pad| holders[pad] != before[pad]) {
            self.heard_ms[pad] = None;
            self.heard_s[pad] = None;
        }
        if holders[self.mine] != self.me {
            self.lost = true;
            return;
        }
        let others: Vec<usize> = self.others().collect();
        match (&self.mode, others.is_empty()) {
            // Alone now: the game goes on from what was given.
            (Mode::Waiting { .. } | Mode::Together(_), true) => {
                self.mode = Mode::Alone;
                self.out.clear();
                self.clock = self.fed() as f64;
            }
            // The one who ran the game sends it to the one who sat.
            (Mode::Alone, false) => {
                for pad in others {
                    let parts = self.ran.history.parts(holders[pad]);
                    self.out.extend(parts);
                }
                self.mode = Mode::Together(Lockstep::new(self.fed()));
                self.clock = self.fed() as f64;
            }
            _ => {}
        }
    }

    /// Bytes relayed by `session`, stamped `clock_ms` by the world, heard at
    /// `now_s`, in their place.
    pub(crate) fn relayed(&mut self, session: u32, payload: &[u8], clock_ms: u64, now_s: f64) {
        if self.lost || session == 0 || !self.holders.contains(&session) {
            return;
        }
        let Ok(wire::Versus {
            message: Some(message),
        }) = wire::Versus::decode(payload)
        else {
            return;
        };
        match message {
            versus::Message::Start(start) => self.started(session, start),
            versus::Message::Pads(pads) => self.wrote(session, pads, clock_ms, now_s),
        }
    }

    /// A part of a Start: taken only while waiting for one, for this
    /// machine, from another holder, its parts in order and adding up to a
    /// game the console can run.
    fn started(&mut self, session: u32, start: wire::Start) {
        let Mode::Waiting {
            start: gathering, ..
        } = &mut self.mode
        else {
            return;
        };
        if start.to != self.me
            || session == self.me
            || start.buttons.len() != start.counts.len()
            || start.at > GAME_LONGEST
        {
            return;
        }
        if start.part == 0 {
            *gathering = Some(Gathering {
                from: session,
                at: start.at,
                parts: start.parts,
                next: 0,
                runs: Runs::default(),
            });
        }
        let Some(gathered) = gathering.as_mut().filter(|gathered| {
            (gathered.from, gathered.at, gathered.parts, gathered.next)
                == (session, start.at, start.parts, start.part)
        }) else {
            *gathering = None;
            return;
        };
        for (&word, &count) in start.buttons.iter().zip(&start.counts) {
            gathered.runs.extend(word, u64::from(count));
        }
        gathered.next += 1;
        if gathered.runs.len() > gathered.at {
            *gathering = None;
            return;
        }
        if gathered.next < gathered.parts {
            return;
        }
        let Some(gathered) = gathering.take().filter(|g| g.runs.len() == g.at) else {
            return;
        };
        // What the game here ran holds where it started over at the same
        // update and was given the same since.
        let ran = Ran::new(&gathered.runs);
        let kept = match ran.start == self.ran.start {
            true => self.ran.game.agrees(&ran.game),
            false => 0,
        };
        self.kept = Some(self.kept.map_or(kept, |was| was.min(kept)));
        self.ran = ran;
        self.clock = gathered.at as f64;
        self.mode = Mode::Together(Lockstep::new(gathered.at));
    }

    /// Buttons of pads, the first written for an update first: a holder's
    /// own, its pad's bits alone; another's for it, released, and only once
    /// its holder wrote nothing for [`WAIT_MS`] by the world's clock.
    fn wrote(&mut self, session: u32, pads: wire::Pads, clock_ms: u64, now_s: f64) {
        let (holders, plays, me, mine) = (self.holders, self.plays, self.me, self.mine);
        let fed = self.fed();
        let Mode::Together(lockstep) = &mut self.mode else {
            return;
        };
        for pad in pads.pads {
            let Some(index) = usize::try_from(pad.pad).ok().filter(|&index| index < plays) else {
                continue;
            };
            let own = holders[index] == session;
            let quiet =
                self.heard_ms[index].is_none_or(|heard| clock_ms.saturating_sub(heard) >= WAIT_MS);
            if holders[index] == 0 || !(own || quiet) {
                continue;
            }
            // A holder who writes is not quiet, wherever its bytes fall: one
            // back from a while away writes behind the releases written for
            // it, and reaches past them only once they stop counting.
            if own {
                self.heard_ms[index] = Some(clock_ms);
                self.heard_s[index] = Some(now_s);
                lockstep.absent[index] = false;
                lockstep.waited_s[index] = None;
            }
            let front = lockstep.front[index];
            // A gap: what comes after it waits for what is missing.
            if pad.from > front {
                continue;
            }
            let room = (lockstep.agreed(holders, plays) + AHEAD_MOST).saturating_sub(front);
            let new = pad
                .buttons
                .iter()
                .skip((front - pad.from) as usize)
                .take(room as usize);
            let mut count = 0;
            for &byte in new {
                let bits = if own { Pad::from_bits(byte) } else { Pad::NONE };
                let at = (front + count - fed) as usize;
                if at >= lockstep.ahead.len() {
                    lockstep.ahead.resize(at + 1, 0);
                }
                // Each pad's place in the word is its own, written once.
                lockstep.ahead[at] |= bits.at(index);
                count += 1;
            }
            if count == 0 {
                continue;
            }
            lockstep.front[index] = front + count;
            if !own && index == mine && session != me {
                lockstep.filled = true;
            }
        }
    }

    /// A turn at `now_s`: `pad` is what this machine's player presses, or
    /// `None` while they are away from the controls, and `ready` whether the
    /// game here runs. Writes this machine's buttons, and a quiet holder's
    /// released, and gives the game every update agreed and due. What to
    /// relay, in order.
    pub(crate) fn turn(&mut self, now_s: f64, pad: Option<Pad>, ready: bool) -> Vec<Vec<u8>> {
        let dt = self.last_s.map_or(0.0, |last| (now_s - last).max(0.0));
        self.last_s = Some(now_s);
        if self.lost {
            return Vec::new();
        }
        let mut relay = Vec::new();
        match &mut self.mode {
            Mode::Waiting { since_s, .. } => {
                let since = *since_s.get_or_insert(now_s);
                self.lost = now_s - since > START_WAITS_S;
            }
            // Alone, a game holds while its player is away.
            Mode::Alone => match pad.filter(|_| ready) {
                Some(pad) => {
                    let fed = self.fed();
                    self.clock =
                        (self.clock + dt * f64::from(UPDATES_PER_S)).min(fed as f64 + OWED_MOST);
                    let due = (self.clock as u64).min(GAME_LONGEST);
                    if due > fed && self.ran.give(pad.at(self.mine), due - fed) {
                        self.kept = Some(0);
                    }
                }
                None => self.clock = self.fed() as f64,
            },
            Mode::Together(_) => relay.extend(self.lockstep(now_s, dt, pad, ready)),
        }
        let parts = self.out.len().min(PARTS_A_TURN);
        let mut sent: Vec<Vec<u8>> = self.out.drain(..parts).collect();
        sent.extend(relay);
        sent
    }

    /// A turn played with others: what to relay, the Pads it wrote.
    fn lockstep(&mut self, now_s: f64, dt: f64, pad: Option<Pad>, ready: bool) -> Option<Vec<u8>> {
        let (mine, plays, holders) = (self.mine, self.plays, self.holders);
        let fed = self.fed();
        let starting = !self.out.is_empty();
        let Mode::Together(lockstep) = &mut self.mode else {
            return None;
        };
        let agreed = lockstep.agreed(holders, plays);
        if agreed > fed + BEHIND_MOST || fed >= GAME_LONGEST {
            self.lost = true;
            return None;
        }
        if ready {
            self.clock =
                (self.clock + dt * f64::from(UPDATES_PER_S)).min((agreed + LAG_MOST) as f64);
            // Behind what the others agreed: caught up at once.
            if agreed as f64 > self.clock + BEHIND_AT_ONCE as f64 {
                self.clock = agreed as f64;
            }
        }
        self.clock = self.clock.max(fed as f64);
        let due = self.clock as u64;
        let mut pads = Vec::new();
        // A Start goes whole before any buttons, and a game that does not run
        // here writes none: the others hear it quiet.
        if ready && !starting {
            let mut until = due + DELAY;
            if core::mem::take(&mut lockstep.filled) {
                // Written released by another: claimed back past it.
                until = until.max(lockstep.front[mine] + DELAY);
            }
            pads.extend(lockstep.write(mine, until, pad.unwrap_or(Pad::NONE).bits(), now_s));
            for other in (0..plays).filter(|&pad| pad != mine && holders[pad] != 0) {
                // Waited on, and nothing new from its holder for a while:
                // written released.
                let waits = lockstep.front[other] <= due;
                let heard = self.heard_s[other].is_some_and(|heard| now_s - heard < WAIT_S);
                lockstep.waited_s[other] =
                    (waits && !heard).then(|| lockstep.waited_s[other].unwrap_or(now_s));
                if lockstep.waited_s[other].is_some_and(|since| now_s - since >= WAIT_S) {
                    lockstep.absent[other] = true;
                }
                if lockstep.absent[other] {
                    pads.extend(lockstep.write(other, due + DELAY, Pad::NONE.bits(), now_s));
                }
            }
        }
        if ready {
            for _ in fed..due.min(agreed) {
                let word = lockstep.ahead.pop_front().unwrap_or(0);
                if self.ran.give(word, 1) {
                    self.kept = Some(0);
                }
            }
        }
        (!pads.is_empty()).then(|| encode(versus::Message::Pads(wire::Pads { pads })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 7;
    const B: u32 = 9;
    const TURN_S: f64 = 1.0 / 60.0;

    /// What the world takes in its one order, to hand on to every holder.
    enum Wire {
        Relayed(u32, Vec<u8>),
        Stations(Holders),
    }

    /// One machine: its part, and whether its game runs.
    struct Machine {
        session: u32,
        versus: Versus,
        ready: bool,
    }

    /// Machines at one cabinet and the world between them: what one sends
    /// reaches the world `latency_s / 2` later, stamped by its clock then,
    /// and every machine `latency_s` later, in the order sent.
    struct Net {
        machines: Vec<Machine>,
        wire: VecDeque<(f64, Wire)>,
        /// Sat and not yet told: made a machine when the world says so.
        sitting: Vec<(u32, usize, Runs)>,
        latency_s: f64,
        now_s: f64,
        turns: u64,
    }

    impl Net {
        /// `A` alone at station 0 of a game of two.
        fn new(latency_s: f64) -> Net {
            Net {
                machines: vec![Machine {
                    session: A,
                    versus: Versus::new(A, 0, 2, [A, 0], Runs::default()),
                    ready: true,
                }],
                wire: VecDeque::new(),
                sitting: Vec::new(),
                latency_s,
                now_s: 0.0,
                turns: 0,
            }
        }

        fn machine(&mut self, session: u32) -> &mut Machine {
            self.machines
                .iter_mut()
                .find(|machine| machine.session == session)
                .unwrap()
        }

        fn versus(&self, session: u32) -> &Versus {
            &self
                .machines
                .iter()
                .find(|machine| machine.session == session)
                .unwrap()
                .versus
        }

        /// The world takes who holds the stations, `session` sat at
        /// `station` with `ran` of the game here.
        fn sit(&mut self, session: u32, station: usize, holders: Holders, ran: Runs) {
            self.sitting.push((session, station, ran));
            self.wire
                .push_back((self.now_s + self.latency_s, Wire::Stations(holders)));
        }

        fn rise(&mut self, session: u32, holders: Holders) {
            self.machines.retain(|machine| machine.session != session);
            self.wire
                .push_back((self.now_s + self.latency_s, Wire::Stations(holders)));
        }

        /// A turn of every machine, each pressing what `pressed` says.
        fn turn(&mut self, pressed: impl Fn(u32, u64) -> Pad) {
            self.now_s += TURN_S;
            self.turns += 1;
            while self.wire.front().is_some_and(|(at, _)| *at <= self.now_s) {
                let (at, wire) = self.wire.pop_front().unwrap();
                match wire {
                    Wire::Relayed(from, payload) => {
                        let stamp = ((at - self.latency_s / 2.0) * 1000.0) as u64;
                        for machine in &mut self.machines {
                            machine.versus.relayed(from, &payload, stamp, self.now_s);
                        }
                    }
                    Wire::Stations(holders) => {
                        for machine in &mut self.machines {
                            machine.versus.stations(holders);
                        }
                        for (session, station, ran) in self.sitting.drain(..) {
                            self.machines.push(Machine {
                                session,
                                versus: Versus::new(session, station, 2, holders, ran),
                                ready: true,
                            });
                        }
                    }
                }
            }
            let turn = self.turns;
            for machine in &mut self.machines {
                let pad = Some(pressed(machine.session, turn));
                for payload in machine.versus.turn(self.now_s, pad, machine.ready) {
                    let at = self.now_s + self.latency_s;
                    self.wire
                        .push_back((at, Wire::Relayed(machine.session, payload)));
                }
            }
        }

        fn turns(&mut self, count: u64, pressed: impl Fn(u32, u64) -> Pad) {
            for _ in 0..count {
                self.turn(&pressed);
            }
        }

        /// The updates every machine ran, and whether they ran the same.
        fn agree(&self) -> u64 {
            let fed = self
                .machines
                .iter()
                .map(|machine| machine.versus.fed())
                .min()
                .unwrap();
            let words = self.machines[0].versus.history().words(0, fed);
            for machine in &self.machines {
                assert_eq!(
                    machine.versus.history().words(0, fed),
                    words,
                    "machines differ"
                );
            }
            fed
        }
    }

    /// Each player presses a pattern of their own.
    fn played(session: u32, turn: u64) -> Pad {
        match session {
            A => Pad::from_bits((turn / 7 % 4) as u8),
            _ => Pad::from_bits(((turn / 5 % 3) as u8) << 2),
        }
    }

    fn pads(versus: &Versus, pad: usize) -> u8 {
        versus
            .history()
            .words(0, versus.fed())
            .iter()
            .fold(0, |all, &word| all | Pad::of_pad(word, pad).bits())
    }

    fn together(versus: &Versus) -> bool {
        matches!(versus.mode, Mode::Together(_))
    }

    fn sent(session: u32, pads: Vec<wire::Pad>) -> Wire {
        Wire::Relayed(session, encode(versus::Message::Pads(wire::Pads { pads })))
    }

    #[test]
    fn two_machines_run_the_same_buttons_each_pad_its_holders() {
        let mut net = Net::new(0.05);
        net.turns(30, played);
        assert_eq!(net.versus(A).fed(), 29, "alone, a game runs at once");
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(600, played);
        let ran = net.agree();
        assert!(ran > 560, "the game kept its pace: {ran}");
        assert!(together(net.versus(B)) && together(net.versus(A)));
        assert_eq!(pads(net.versus(A), 0), 0b11, "A's pad pressed by A");
        assert_eq!(pads(net.versus(A), 1), 0b1100, "B's pad pressed by B");
    }

    #[test]
    fn a_game_joined_long_runs_the_same_from_its_start_in_parts() {
        let mut net = Net::new(0.02);
        // Every update a run of its own: a Start of several parts.
        net.turns(2000, |_, turn| Pad::from_bits((turn % 2) as u8));
        assert!(net.versus(A).history().parts(B).len() > 1);
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(120, played);
        assert!(net.agree() > 2050);
    }

    #[test]
    fn a_holder_gone_quiet_is_released_and_claims_its_pad_back() {
        // On one Wi-Fi, past the delay, and a phone on a mobile link.
        for latency_s in [0.05, 0.1, 0.25] {
            let mut net = Net::new(latency_s);
            net.sit(B, 1, [A, B], Runs::default());
            net.turns(120, played);
            let before = net.agree();
            net.machine(B).ready = false;
            net.turns(300, played);
            let a = net.versus(A).fed();
            // Held a wait and a round trip, then on at its pace.
            assert!(
                a > before + 200,
                "A plays on, B's pad released: {before} to {a} at {latency_s} s"
            );
            net.machine(B).ready = true;
            net.turns(120, played);
            assert!(
                net.agree() > a + 90,
                "B caught up and plays again at {latency_s} s"
            );
            let back = net.versus(A).history().words(a + 60, net.versus(A).fed());
            assert!(
                back.iter().any(|&word| Pad::of_pad(word, 1) != Pad::NONE),
                "B's pad is B's again at {latency_s} s"
            );
        }
    }

    #[test]
    fn one_who_rises_leaves_the_other_alone_and_returns_where_it_left() {
        let mut net = Net::new(0.05);
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(120, played);
        net.agree();
        let left = net.versus(B).history().clone();
        net.rise(B, [A, 0]);
        net.turns(60, played);
        assert!(!together(net.versus(A)));
        net.sit(B, 1, [A, B], left.clone());
        net.turns(60, played);
        let b = net.versus(B);
        assert!(together(b));
        assert!(
            b.runs_on(left.len()),
            "what B ran before still holds: replayed from there"
        );
        net.agree();
    }

    #[test]
    fn a_peer_writes_its_own_pad_alone_and_never_past_its_bits() {
        let mut net = Net::new(0.0);
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(10, |_, _| Pad::NONE);
        let front = net.versus(A).fed() + 40;
        net.wire.push_back((
            net.now_s,
            sent(
                B,
                vec![
                    // A's pad while A plays: dropped.
                    wire::Pad {
                        pad: 0,
                        from: front,
                        buttons: vec![0xff; 10],
                    },
                    // A pad the game does not read.
                    wire::Pad {
                        pad: 5,
                        from: 0,
                        buttons: vec![0xff; 10],
                    },
                ],
            ),
        ));
        let flood = wire::Pad {
            pad: 1,
            from: 0,
            buttons: vec![0xff; RELAY_BYTES],
        };
        net.wire.push_back((net.now_s, sent(B, vec![flood])));
        // Not a holder.
        net.wire.push_back((
            net.now_s,
            sent(
                3,
                vec![wire::Pad {
                    pad: 0,
                    from: 0,
                    buttons: vec![1; 99],
                }],
            ),
        ));
        net.wire
            .push_back((net.now_s, Wire::Relayed(B, vec![0xff, 0x01, 0x07])));
        net.turns(180, |_, _| Pad::NONE);
        let a = net.versus(A);
        assert_eq!(pads(a, 0), 0, "nobody pressed A's pad");
        assert_eq!(pads(a, 1), 0x7f, "B's bytes kept to its pad's bits");
        let Mode::Together(lockstep) = &a.mode else {
            panic!()
        };
        assert!(
            lockstep.ahead.len() as u64 <= AHEAD_MOST + LAG_MOST + DELAY,
            "a flood holds no memory"
        );
        net.agree();
    }

    #[test]
    fn a_gap_a_repeat_and_the_past_change_nothing() {
        let mut net = Net::new(0.0);
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(30, played);
        let fed = net.versus(A).fed();
        for (from, bits) in [
            (0, 0x7f),
            (fed / 2, 0x7f),
            (fed + 1000, 0x7f),
            (u64::MAX - 3, 0x7f),
        ] {
            net.wire.push_back((
                net.now_s,
                sent(
                    B,
                    vec![wire::Pad {
                        pad: 1,
                        from,
                        buttons: vec![bits; 8],
                    }],
                ),
            ));
        }
        net.turns(60, played);
        assert_eq!(pads(net.versus(A), 1), 0b1100, "only what B played");
        net.agree();
    }

    #[test]
    fn a_release_of_a_holder_who_plays_is_dropped_on_every_machine() {
        let mut net = Net::new(0.05);
        let up = |session, _| match session {
            A => Pad::NONE,
            _ => Pad::UP,
        };
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(60, up);
        // A writes B's pad released from its start past where B wrote, while B plays.
        let Mode::Together(lockstep) = &net.versus(A).mode else {
            panic!()
        };
        let front = lockstep.front[1];
        let release = wire::Pad {
            pad: 1,
            from: 0,
            buttons: vec![0; front as usize + 30],
        };
        net.wire.push_back((net.now_s, sent(A, vec![release])));
        net.turns(120, up);
        let a = net.versus(A);
        let held: Vec<u32> = a.history().words(front, front + 60);
        assert!(
            held.iter().all(|&word| Pad::of_pad(word, 1) == Pad::UP),
            "B's pad stays B's"
        );
        net.agree();
    }

    #[test]
    fn a_start_not_asked_for_or_broken_is_dropped_and_its_wait_ends() {
        let mut b = Versus::new(B, 1, 2, [A, B], Runs::default());
        let mut runs = Runs::default();
        for word in 0..2000 {
            runs.extend(word % 3, 1);
        }
        let parts = runs.parts(B);
        assert!(parts.len() > 1);
        // For another, then its parts out of order.
        b.relayed(A, &runs.parts(5)[0], 0, 0.0);
        b.relayed(A, &parts[1], 0, 0.0);
        b.relayed(A, &parts[0], 0, 0.0);
        b.relayed(A, &parts[2], 0, 0.0);
        // From one who holds no station.
        b.relayed(3, &parts[0], 0, 0.0);
        let liar = encode(versus::Message::Start(wire::Start {
            to: B,
            at: 5,
            part: 0,
            parts: 1,
            buttons: vec![1],
            counts: vec![u32::MAX],
        }));
        b.relayed(A, &liar, 0, 0.0);
        assert!(!together(&b));
        b.turn(0.0, None, true);
        b.turn(START_WAITS_S + 1.0, None, true);
        assert!(b.lost(), "nobody sent the game: it gives the station up");
    }

    #[test]
    fn a_station_given_to_another_is_lost() {
        let mut a = Versus::new(A, 0, 2, [A, 0], Runs::default());
        a.stations([3, 0]);
        assert!(a.lost());
        assert!(a.turn(1.0, Some(Pad::UP), true).is_empty());
    }

    #[test]
    fn the_last_words_of_a_long_game_cost_what_a_short_one_s_do() {
        let game = |runs: u32| {
            let mut game = Runs::default();
            for word in 0..runs {
                game.extend(word % 2, 1);
            }
            game
        };
        let (short, long) = (game(10), game(1_000_000));
        let read = |runs: &Runs| {
            let start = std::time::Instant::now();
            for _ in 0..100 {
                assert_eq!(runs.words(runs.len() - 2, runs.len()), [0, 1]);
            }
            start.elapsed()
        };
        let (short, long) = (read(&short), read(&long));
        assert!(
            long < short * 20 + std::time::Duration::from_millis(5),
            "a turn of a long game costs {long:?}, of a short one {short:?}"
        );
    }

    /// Every machine's game since it last started over: the same, from
    /// the same update of the history.
    fn games_agree(net: &Net) -> u64 {
        let first = net.machines[0].versus.ran.start;
        let len = net
            .machines
            .iter()
            .map(|machine| machine.versus.game().len())
            .min()
            .unwrap();
        let words = net.machines[0].versus.game().words(0, len);
        for machine in &net.machines {
            assert_eq!(machine.versus.ran.start, first, "started over apart");
            assert_eq!(machine.versus.game().words(0, len), words, "games differ");
        }
        first
    }

    /// `holder` holds Start, the other plays its pattern.
    fn holding(holder: u32) -> impl Fn(u32, u64) -> Pad {
        move |session, turn| match session == holder {
            true => Pad::START,
            false => played(session, turn),
        }
    }

    #[test]
    fn start_held_by_either_resets_the_game_on_both_at_one_update() {
        for holder in [A, B] {
            let mut net = Net::new(0.05);
            net.sit(B, 1, [A, B], Runs::default());
            net.turns(120, played);
            net.agree();
            assert_eq!(games_agree(&net), 0);
            net.turns(150, holding(holder));
            net.turns(120, played);
            let fed = net.agree();
            let start = games_agree(&net);
            assert!(
                start > 120 && start < fed - 60,
                "started over at {start} of {fed}"
            );
            for machine in &net.machines {
                assert!(!machine.versus.runs_on(1), "the game here starts over");
                // The Start that reset it is given released.
                let pad = (holder == B) as usize;
                let first = machine.versus.game().words(0, 1)[0];
                assert!(!Pad::of_pad(first, pad).has(Pad::START), "{first:#x}");
            }
        }
    }

    #[test]
    fn start_let_go_sooner_resets_nothing_and_the_game_has_it() {
        let mut net = Net::new(0.05);
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(120, played);
        net.turns(100, holding(B));
        net.turns(60, played);
        net.agree();
        assert_eq!(games_agree(&net), 0);
        let versus = net.versus(A);
        assert_eq!(versus.game().len(), versus.fed());
        assert!(versus.runs_on(versus.fed()));
        assert!(versus.closed().is_none());
        assert_eq!(pads(versus, 1) & Pad::START.bits(), Pad::START.bits());
    }

    #[test]
    fn one_who_sits_beside_a_game_reset_finds_the_same_reset_in_it() {
        let mut net = Net::new(0.05);
        net.turns(60, played);
        net.turns(150, holding(A));
        net.turns(30, played);
        let start = net.versus(A).ran.start;
        assert!(start > 60, "alone, reset at {start}");
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(120, played);
        net.agree();
        assert_eq!(games_agree(&net), start);
    }

    #[test]
    fn one_back_after_a_reset_runs_on_what_the_game_here_ran() {
        let mut net = Net::new(0.05);
        net.sit(B, 1, [A, B], Runs::default());
        net.turns(60, played);
        net.turns(150, holding(B));
        net.turns(60, played);
        net.agree();
        let left = net.versus(B).history().clone();
        let ran = net.versus(B).game().len();
        net.rise(B, [A, 0]);
        net.turns(60, played);
        net.sit(B, 1, [A, B], left);
        net.turns(60, played);
        assert!(net.versus(B).runs_on(ran), "replayed from there");
        net.agree();
        games_agree(&net);
    }

    #[test]
    fn runs_say_their_words_and_how_far_two_agree() {
        let mut runs = Runs::default();
        for word in [1, 1, 1, 2, 2, 3] {
            runs.extend(word, 1);
        }
        assert_eq!(runs.words(2, 5), [1, 2, 2]);
        assert_eq!(runs.words(0, 9), [1, 1, 1, 2, 2, 3]);
        assert_eq!(runs.words(3, 4), [2]);
        assert!(runs.words(5, 5).is_empty());
        let mut other = runs.clone();
        other.extend(9, 1);
        assert_eq!(runs.agrees(&other), 6);
        let mut fork = Runs::default();
        for word in [1, 1, 1, 2, 7] {
            fork.extend(word, 1);
        }
        assert_eq!(runs.agrees(&fork), 4);
    }
}
