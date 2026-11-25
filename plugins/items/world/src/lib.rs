//! The items plugin's world half. Items is how a hand places parts, and a
//! hand is in the client: the parts and the ops that change them are the
//! core's. What this half holds is who stands at a cabinet's controls: a
//! station each player, one a pad of its game. Who holds which is the
//! world's, said to everyone, and the holders of one cabinet are linked so
//! they relay their buttons to each other.
//!
//! What both halves say once lives here: the plugin's name and version, its
//! ops and its events, its wire.

use std::collections::BTreeMap;

use prost::Message;
use world::{Level, Op, Room, Who};

/// The plugin's wire, generated from the schema in its `wire/` by
/// `bun run proto`.
pub mod wire {
    #![allow(clippy::all, clippy::pedantic)]
    include!("gen/planet/items/v1/planet.items.v1.rs");
}

/// What the world's statement calls the plugin.
pub const NAME: &str = "items";
/// The version of its wire and of its seam, and of what a game of several
/// pads reads in the buttons its players agree: a client that reads them
/// otherwise, a held Start that resets, speaks another version and pairs
/// with no one of this one.
pub const VERSION: u32 = 3;

/// The ops a session asks: to take a station, to let it go, and to hear who
/// holds every station there is.
pub const SIT: &str = "sit";
pub const RISE: &str = "rise";
pub const LOOK: &str = "look";
/// The event everyone hears when who holds a cabinet's stations changes.
pub const STATIONS: &str = "stations";

/// Why a sit is refused: past the pads a game reads, held by another.
pub const REFUSED_STATION: &str = "station";
pub const REFUSED_TAKEN: &str = "taken";
/// A payload that is no op of this plugin.
pub const REFUSED_PAYLOAD: &str = "payload";

/// The stations a cabinet has: one a pad the console reads. A game reads as
/// many of them as it plays, and a client offers no more; one past it holds
/// a pad the game never reads.
pub const STATIONS_MOST: usize = arcade::PADS;

/// The plugin: who holds each station, by cabinet.
#[derive(Default)]
pub struct Items {
    /// A session at each station, by station, of every cabinet someone holds.
    held: BTreeMap<u64, [u32; STATIONS_MOST]>,
}

/// The plugin as the server's module holds it.
pub fn plugin() -> Box<dyn world::Plugin> {
    Box::new(Items::default())
}

impl Items {
    /// The cabinet and station `session` holds, if any.
    fn station_of(&self, session: u32) -> Option<(u64, usize)> {
        self.held.iter().find_map(|(&cabinet, holders)| {
            holders
                .iter()
                .position(|&held| held == session)
                .map(|station| (cabinet, station))
        })
    }

    /// Lets `session` go of what it holds; the cabinet it held, if any.
    fn free(&mut self, session: u32) -> Option<u64> {
        let (cabinet, station) = self.station_of(session)?;
        let holders = self.held.get_mut(&cabinet)?;
        holders[station] = 0;
        if holders.iter().all(|&held| held == 0) {
            self.held.remove(&cabinet);
        }
        Some(cabinet)
    }

    /// Says who holds `cabinet`'s stations to everyone, and links them.
    fn said(&self, cabinet: u64, room: &mut dyn Room) {
        let holders = self.held.get(&cabinet).copied().unwrap_or_default();
        let everyone: Vec<u32> = room.sessions().iter().map(|who| who.session).collect();
        room.tell(STATIONS, stations(cabinet, &holders), &everyone);
        let linked: Vec<u32> = holders.into_iter().filter(|&held| held != 0).collect();
        room.link(cabinet, &linked);
    }
}

/// The event that says who holds `cabinet`'s stations.
fn stations(cabinet: u64, holders: &[u32]) -> Vec<u8> {
    wire::Stations {
        cabinet,
        holders: holders.to_vec(),
    }
    .encode_to_vec()
}

impl world::Plugin for Items {
    fn name(&self) -> &'static str {
        NAME
    }

    fn version(&self) -> u32 {
        VERSION
    }

    /// Anyone in the world may play, a visitor included.
    fn ops(&self) -> Vec<Op> {
        [SIT, RISE, LOOK]
            .into_iter()
            .map(|kind| Op {
                kind,
                level: Level::Anonymous,
            })
            .collect()
    }

    fn op(&mut self, kind: &str, payload: &[u8], who: &Who, room: &mut dyn Room) {
        match kind {
            SIT => {
                let Ok(sit) = wire::Sit::decode(payload) else {
                    return room.refuse(REFUSED_PAYLOAD);
                };
                let station = usize::try_from(sit.station).unwrap_or(usize::MAX);
                if station >= STATIONS_MOST {
                    return room.refuse(REFUSED_STATION);
                }
                let held = self.held.get(&sit.cabinet).map(|holders| holders[station]);
                // Asked again, by one whose answer went astray or whose
                // client forgot it: said again.
                if held == Some(who.session) {
                    return self.said(sit.cabinet, room);
                }
                if held.is_some_and(|held| held != 0) {
                    return room.refuse(REFUSED_TAKEN);
                }
                // One station a session: the one it held elsewhere is let go.
                if let Some(left) = self.free(who.session) {
                    self.said(left, room);
                }
                self.held.entry(sit.cabinet).or_default()[station] = who.session;
                self.said(sit.cabinet, room);
            }
            RISE => {
                let Ok(rise) = wire::Rise::decode(payload) else {
                    return room.refuse(REFUSED_PAYLOAD);
                };
                if self
                    .station_of(who.session)
                    .is_some_and(|(cabinet, _)| cabinet == rise.cabinet)
                {
                    self.free(who.session);
                    self.said(rise.cabinet, room);
                }
            }
            LOOK => {
                for (&cabinet, holders) in &self.held {
                    room.tell(STATIONS, stations(cabinet, holders), &[who.session]);
                }
            }
            _ => {}
        }
    }

    /// A session that left lets go of its station. Everyone hears it left,
    /// and lets go of it there too; the server lets go of its link.
    fn gone(&mut self, session: u32) {
        self.free(session);
    }
}
