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
//! rise. One whose link dropped sits again, once back, at the station they
//! held if it is still free, and runs at once what they missed.

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
    held: BTreeMap<PartId, Holders>,
    /// The session the world was last asked who holds them for.
    session: Option<u32>,
    /// A station asked for and not given yet.
    asked: Option<Asked>,
    /// The station held, and the game played from it.
    playing: Option<Playing>,
    /// The clock at the last turn, seconds: what is relayed between turns is
    /// heard then.
    now_s: f64,
}

struct Asked {
    cabinet: PartId,
    station: usize,
    since_s: f64,
    /// How the game here ran, kept from a station held before.
    ran: Runs,
}

struct Playing {
    cabinet: PartId,
    versus: Versus,
}

impl Stations {
    /// Whether this machine holds or asked for a station of `cabinet`.
    pub(crate) fn sits_at(&self, cabinet: PartId) -> bool {
        self.station_at(cabinet).is_some()
    }

    /// The station of `cabinet` this machine holds or asked for.
    fn station_at(&self, cabinet: PartId) -> Option<usize> {
        let playing = self
            .playing
            .as_ref()
            .filter(|playing| playing.cabinet == cabinet);
        let asked = self.asked.as_ref().filter(|asked| asked.cabinet == cabinet);
        playing
            .map(|playing| playing.versus.station())
            .or(asked.map(|asked| asked.station))
    }

    /// The first station of `cabinet` free among the `plays` its game reads.
    fn free(&self, cabinet: PartId, plays: usize) -> Option<usize> {
        let holders = self.held.get(&cabinet).copied().unwrap_or_default();
        (0..plays.min(PADS)).find(|&station| holders[station] == 0)
    }

    /// Who holds `cabinet`'s stations now.
    fn said(&mut self, cabinet: PartId, holders: Holders) {
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

/// The pads `cabinet`'s game reads, when it reads more than one: a game of
/// stations.
fn stations_of(host: &Host<'_>, cabinet: PartId) -> Option<usize> {
    host.game_plays(cabinet).filter(|&plays| plays > 1)
}

impl Items {
    /// Whether `cabinet` may be played from here: a game of one pad always,
    /// one of stations while this machine sits there or a station is free.
    pub(crate) fn station_open(&self, host: &Host<'_>, cabinet: PartId) -> bool {
        match stations_of(host, cabinet) {
            None => true,
            Some(plays) => {
                self.stations.sits_at(cabinet) || self.stations.free(cabinet, plays).is_some()
            }
        }
    }

    /// The station `cabinet` is played from, asked for when its game is one
    /// of stations this machine does not sit at: player 1's for a game of
    /// one pad. `None` when none is free.
    pub(crate) fn sit(&mut self, host: &mut Host<'_>, cabinet: PartId) -> Option<usize> {
        let Some(plays) = stations_of(host, cabinet) else {
            return Some(0);
        };
        if let Some(station) = self.stations.station_at(cabinet) {
            return Some(station);
        }
        let station = self.stations.free(cabinet, plays)?;
        self.rise(host);
        self.ask(host, cabinet, station, Runs::default());
        Some(station)
    }

    fn ask(&mut self, host: &mut Host<'_>, cabinet: PartId, station: usize, ran: Runs) {
        let sit = wire::Sit {
            cabinet: cabinet.0,
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
            host.send(RISE, wire::Rise { cabinet: cabinet.0 }.encode_to_vec());
        }
    }

    /// The turn of the station held: the world asked who holds the stations
    /// once a session begins, one dropped sits again, and the game played
    /// from the station is given every update agreed.
    pub(crate) fn stations_turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        self.stations.now_s = turn.now_s;
        let session = host.session();
        if session != self.stations.session {
            self.stations.session = session;
            self.stations.held.clear();
            if session.is_some() {
                host.send(LOOK, wire::Look {}.encode_to_vec());
            }
            // Back with another session: sat again where it was, the game
            // here as it ran.
            if let (Some(_), Some(playing)) = (session, self.stations.playing.take()) {
                let station = playing.versus.station();
                let ran = playing.versus.history().clone();
                self.ask(host, playing.cabinet, station, ran);
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
        let cabinet = playing.cabinet;
        let played = self.played == Some(cabinet);
        // Its game stopped waiting, its cabinet went, or it gave up: risen.
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
            host.relay(cabinet.0, payload);
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
        let cabinet = PartId(said.cabinet);
        let mut holders = Holders::default();
        for (held, &session) in holders.iter_mut().zip(&said.holders) {
            *held = session;
        }
        self.stations.said(cabinet, holders);
        let Some(asked) = self
            .stations
            .asked
            .take_if(|asked| asked.cabinet == cabinet)
        else {
            return;
        };
        let (Some(me), Some(plays)) = (host.session(), stations_of(host, cabinet)) else {
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
            None if self.played == Some(cabinet) => self.leave(host),
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
            .filter(|playing| playing.cabinet.0 == relayed.thing)
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
        let left: Vec<(PartId, Holders)> = self
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
