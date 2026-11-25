//! A room a test holds for an owner: a store in memory, what it told and
//! to whom, the code it refused with, whoever a test puts in it, and the
//! ground of the test world when a test hands one.

use std::collections::BTreeMap;

use worldgen::Generator;

use crate::{Measure, Moment, Room, Who};

/// A room that keeps what it is told and what it is given to keep, as the
/// server does once an op has been applied.
pub(crate) struct Stage {
    pub measure: Measure,
    pub ground: Option<Generator>,
    pub now: Moment,
    pub here: Vec<Who>,
    pub store: BTreeMap<Vec<u8>, Vec<u8>>,
    pub told: Vec<(String, Vec<u8>, Vec<u32>)>,
    pub refused: Option<String>,
    /// Who holds each thing the owner linked.
    pub linked: BTreeMap<u64, Vec<u32>>,
}

impl Stage {
    /// A room of the world a generator makes, with nobody in it and nothing
    /// kept; its ground handed when `ground`.
    pub fn of(generator: Generator, ground: bool) -> Stage {
        Stage {
            measure: Measure {
                sphere: generator.sphere(),
                moon: generator.moon(),
            },
            ground: ground.then_some(generator),
            now: Moment(0),
            here: Vec::new(),
            store: BTreeMap::new(),
            told: Vec::new(),
            refused: None,
            linked: BTreeMap::new(),
        }
    }
}

impl Room for Stage {
    fn now(&self) -> Moment {
        self.now
    }

    fn measure(&self) -> Measure {
        self.measure
    }

    fn sessions(&self) -> &[Who] {
        &self.here
    }

    fn tell(&mut self, kind: &str, payload: Vec<u8>, to: &[u32]) {
        self.told.push((kind.to_owned(), payload, to.to_vec()));
    }

    fn ground(&self) -> Option<&Generator> {
        self.ground.as_ref()
    }

    fn get(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        self.store.get(key).cloned()
    }

    fn scan(&mut self, prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let rows = self.store.iter().filter(|(key, _)| key.starts_with(prefix));
        rows.map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    fn keep(&mut self, key: &[u8], value: Vec<u8>) {
        self.store.insert(key.to_vec(), value);
    }

    fn forget(&mut self, key: &[u8]) {
        self.store.remove(key);
    }

    fn refuse(&mut self, code: &str) {
        self.refused = Some(code.to_owned());
    }

    fn link(&mut self, thing: u64, sessions: &[u32]) {
        if sessions.is_empty() {
            self.linked.remove(&thing);
        } else {
            self.linked.insert(thing, sessions.to_vec());
        }
    }
}
