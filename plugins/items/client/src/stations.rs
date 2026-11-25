//! The stations of a cabinet whose game reads several pads: one a player,
//! each on their own machine. The first to sit holds pad 1, the next pad 2,
//! and one more finds no station free and is offered nothing. Who holds
//! which is the world's, said to everyone; the game each machine runs from
//! its station is [`crate::versus`]'s.
//!
//! A game of one pad has no stations: whoever plays it runs a copy of their
//! own, as everyone near may.
//!
//! A player keeps their station while their game waits for them: left, the
//! game runs on with their pad released, and they take it back by playing
//! again. Once the game stops waiting, or it gives the station up, they
//! rise. One whose link dropped, back while their game waits, plays it again
//! from the station they held if it is still free: what the game here ran
//! still holds, and what they missed runs at once.
//!
//! A cabinet is named here as the world names it: the name this client gives
//! a part is its own, and new after a link drops.

use std::collections::BTreeMap;

use client::{Host, PADS, Pad, PartId, Relayed, Turn};
use items_world::{LOOK, RISE, SIT, wire};
use prost::Message;

use crate::Items;
use crate::versus::{BEHIND_AT_ONCE, Holders, Runs, START_WAITS_S, Versus};

/// The stations this machine knows of, and the one it holds or asked for.
#[derive(Default)]
pub(crate) struct Stations {
    /// Who holds each cabinet's stations, as the world last said: a cabinet
    /// not here has every station free.
    held: BTreeMap<u64, Holders>,
    /// The session the world was last asked who holds them for.
    session: Option<u32>,
    /// A station asked for and not given yet.
    asked: Option<Asked>,
    /// The station held, and the game played from it.
    playing: Option<Playing>,
    /// The station held when the link dropped, and how the game here ran.
    dropped: Option<Asked>,
    /// The clock at the last turn, seconds: what is relayed between turns is
    /// heard then.
    now_s: f64,
}

struct Asked {
    cabinet: u64,
    station: usize,
    since_s: f64,
    /// How the game here ran, kept from a station held before.
    ran: Runs,
}

struct Playing {
    cabinet: u64,
    versus: Versus,
}

impl Stations {
    /// Whether this machine holds or asked for a station of `cabinet`.
    fn sits_at(&self, cabinet: u64) -> bool {
        self.station_at(cabinet).is_some()
    }

    /// The station of `cabinet` this machine holds or asked for.
    fn station_at(&self, cabinet: u64) -> Option<usize> {
        let playing = self
            .playing
            .as_ref()
            .filter(|playing| playing.cabinet == cabinet);
        let asked = self.asked.as_ref().filter(|asked| asked.cabinet == cabinet);
        playing
            .map(|playing| playing.versus.station())
            .or(asked.map(|asked| asked.station))
    }

    /// The first station of `cabinet` free among the `plays` its game reads:
    /// held by nobody, or by this session, whose sit the world says again.
    fn free(&self, cabinet: u64, plays: usize) -> Option<usize> {
        let holders = self.held.get(&cabinet).copied().unwrap_or_default();
        (0..plays.min(PADS))
            .find(|&station| holders[station] == 0 || Some(holders[station]) == self.session)
    }

    /// Who holds `cabinet`'s stations now.
    fn said(&mut self, cabinet: u64, holders: Holders) {
        match holders.iter().all(|&held| held == 0) {
            true => self.held.remove(&cabinet),
            false => self.held.insert(cabinet, holders),
        };
        if let Some(playing) = self
            .playing
            .as_mut()
            .filter(|playing| playing.cabinet == cabinet)
        {
            playing.versus.stations(holders);
        }
    }
}

/// Who holds the station played with no world: a session past those a
/// world gives, which count up from 1, and never 0, which is nobody. It
/// holds while no session does: one that comes ends it.
const NO_WORLD: u32 = u32::MAX;

/// The pads `cabinet`'s game reads, when it reads more than one: a game of
/// stations.
fn stations_of(host: &Host<'_>, cabinet: PartId) -> Option<usize> {
    host.game_plays(cabinet).filter(|&plays| plays > 1)
}

impl Items {
    /// Whether this machine holds or asked for a station of `cabinet`.
    pub(crate) fn sits_at(&self, host: &Host<'_>, cabinet: PartId) -> bool {
        let named = host.parts().known_as(cabinet);
        named.is_some_and(|named| self.stations.sits_at(named))
    }

    /// Whether `cabinet` may be played from here: a game of one pad always,
    /// one of stations while this machine sits there or a station is free.
    pub(crate) fn station_open(&self, host: &Host<'_>, cabinet: PartId) -> bool {
        let Some(plays) = stations_of(host, cabinet) else {
            return true;
        };
        host.parts().known_as(cabinet).is_some_and(|named| {
            self.stations.sits_at(named) || self.stations.free(named, plays).is_some()
        })
    }

    /// The station `cabinet` is played from, asked for when its game is one
    /// of stations this machine does not sit at: the one it held before its
    /// link dropped while free, else the first free; player 1's for a game
    /// of one pad. `None` when none is free.
    pub(crate) fn sit(&mut self, host: &mut Host<'_>, cabinet: PartId) -> Option<usize> {
        let Some(plays) = stations_of(host, cabinet) else {
            return Some(0);
        };
        let cabinet = host.parts().known_as(cabinet)?;
        if let Some(station) = self.stations.station_at(cabinet) {
            return Some(station);
        }
        let dropped = self
            .stations
            .dropped
            .take()
            .filter(|dropped| dropped.cabinet == cabinet);
        let holders = self
            .stations
            .held
            .get(&cabinet)
            .copied()
            .unwrap_or_default();
        let station = dropped
            .as_ref()
            .map(|dropped| dropped.station)
            .filter(|&station| holders[station] == 0)
            .or(self.stations.free(cabinet, plays))?;
        self.rise(host);
        let ran = dropped.map(|dropped| dropped.ran).unwrap_or_default();
        match host.session() {
            Some(_) => self.ask(host, cabinet, station, ran),
            // No world holds stations: its player plays at once, alone.
            None => {
                let mut holders = Holders::default();
                holders[station] = NO_WORLD;
                let versus = Versus::new(NO_WORLD, station, plays, holders, ran);
                self.stations.playing = Some(Playing { cabinet, versus });
            }
        }
        Some(station)
    }

    /// The link dropped, or the plugin rests: the station is let go, and
    /// kept in mind with how the game here ran, to play it again.
    pub(crate) fn rest_station(&mut self, host: &mut Host<'_>) {
        if let Some(playing) = &self.stations.playing {
            self.stations.dropped = Some(Asked {
                cabinet: playing.cabinet,
                station: playing.versus.station(),
                since_s: self.stations.now_s,
                ran: playing.versus.history().clone(),
            });
        }
        self.rise(host);
        // What the world said goes with it: a rise while the plugin is off is
        // refused, so its first turn back asks the world again.
        self.stations.session = None;
        self.stations.held.clear();
    }

    fn ask(&mut self, host: &mut Host<'_>, cabinet: u64, station: usize, ran: Runs) {
        let sit = wire::Sit {
            cabinet,
            station: station as u32,
        };
        host.send(SIT, sit.encode_to_vec());
        self.stations.asked = Some(Asked {
            cabinet,
            station,
            since_s: self.stations.now_s,
            ran,
        });
    }

    /// Lets the station held or asked for go.
    pub(crate) fn rise(&mut self, host: &mut Host<'_>) {
        let playing = self.stations.playing.take().map(|playing| playing.cabinet);
        let asked = self.stations.asked.take().map(|asked| asked.cabinet);
        if let Some(cabinet) = playing.or(asked) {
            host.send(RISE, wire::Rise { cabinet }.encode_to_vec());
        }
    }

    /// The turn of the station held: the world asked who holds the stations
    /// once a session begins, and the game played from the station given
    /// every update agreed.
    pub(crate) fn stations_turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        self.stations.now_s = turn.now_s;
        let session = host.session();
        if session != self.stations.session {
            // A station played with no world, or held in one gone, is no
            // station of this world's: let go, kept in mind to sit again.
            if self.stations.playing.is_some() {
                if self.played.is_some() {
                    self.leave(host);
                }
                self.rest_station(host);
            }
            self.stations.session = session;
            self.stations.held.clear();
            if session.is_some() {
                host.send(LOOK, wire::Look {}.encode_to_vec());
            }
        }
        if let Some(asked) = &self.stations.asked
            && turn.now_s - asked.since_s > START_WAITS_S
        {
            self.stations.asked = None;
            if self.played.is_some() {
                self.leave(host);
            }
        }
        let Some(playing) = self.stations.playing.as_mut() else {
            return;
        };
        let named = playing.cabinet;
        // Its cabinet went, or is out of reach: risen.
        let Some(cabinet) = host.parts().known_here(named) else {
            return self.rise(host);
        };
        let played = self.played == Some(cabinet);
        // Its game stopped waiting, or it gave up: risen.
        if playing.versus.lost()
            || !host.game_playable(cabinet)
            || !(played || host.game_waits(cabinet))
        {
            if played {
                self.leave(host);
            }
            return self.rise(host);
        }
        let given = host.game_given(cabinet);
        let pad = turn
            .controls
            .filter(|_| played)
            .or(played.then_some(Pad::NONE));
        for payload in playing.versus.turn(turn.now_s, pad, given.is_some()) {
            host.relay(named, payload);
        }
        let Some(given) = given else {
            return;
        };
        let versus = &mut playing.versus;
        if !versus.runs_on(given) {
            return host.restart_game(cabinet);
        }
        let fed = versus.fed();
        if fed > given {
            let words = versus.history().words(given, fed);
            host.feed_game(cabinet, &words, fed - given > BEHIND_AT_ONCE);
        }
        versus.given_all();
    }

    /// Who holds a cabinet's stations, said by the world.
    pub(crate) fn stations_said(&mut self, payload: &[u8], host: &mut Host<'_>) {
        let Ok(said) = wire::Stations::decode(payload) else {
            return;
        };
        let cabinet = said.cabinet;
        let mut holders = Holders::default();
        for (held, &session) in holders.iter_mut().zip(&said.holders) {
            *held = session;
        }
        self.stations.said(cabinet, holders);
        // Held here and neither asked for nor played: an answer come after
        // the wait, or a station held before the plugin was switched off and
        // on. Let go, or it is nobody's to play.
        let me = host.session();
        if me.is_some_and(|me| holders.contains(&me)) && !self.stations.sits_at(cabinet) {
            host.send(RISE, wire::Rise { cabinet }.encode_to_vec());
            return;
        }
        let Some(asked) = self
            .stations
            .asked
            .take_if(|asked| asked.cabinet == cabinet)
        else {
            return;
        };
        let here = host.parts().known_here(cabinet);
        let plays = here.and_then(|here| stations_of(host, here));
        let (Some(me), Some(plays)) = (host.session(), plays) else {
            return;
        };
        if holders[asked.station] == me {
            let versus = Versus::new(me, asked.station, plays, holders, asked.ran);
            self.stations.playing = Some(Playing { cabinet, versus });
            return;
        }
        if holders[asked.station] == 0 {
            self.stations.asked = Some(asked);
            return;
        }
        // Another took it first: the next free, or none.
        match self.stations.free(cabinet, plays) {
            Some(station) => self.ask(host, cabinet, station, asked.ran),
            None if self.played.is_some() && self.played == here => self.leave(host),
            None => {}
        }
    }

    /// Bytes relayed among the holders of a cabinet's stations.
    pub(crate) fn stations_relayed(&mut self, relayed: &Relayed<'_>) {
        let now_s = self.stations.now_s;
        if let Some(playing) = self
            .stations
            .playing
            .as_mut()
            .filter(|playing| playing.cabinet == relayed.thing)
        {
            let Relayed {
                session,
                payload,
                clock_ms,
                ..
            } = *relayed;
            playing.versus.relayed(session, payload, clock_ms, now_s);
        }
    }

    /// A session left the world: its station is free.
    pub(crate) fn stations_left(&mut self, session: u32) {
        let left: Vec<(u64, Holders)> = self
            .stations
            .held
            .iter()
            .filter(|(_, holders)| holders.contains(&session))
            .map(|(&cabinet, &holders)| {
                (
                    cabinet,
                    holders.map(|held| if held == session { 0 } else { held }),
                )
            })
            .collect();
        for (cabinet, holders) in left {
            self.stations.said(cabinet, holders);
        }
    }
}
