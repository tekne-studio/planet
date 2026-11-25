//! A game the host holds for the runner, on an engine and a store of its
//! own: wasmi lets nothing go from a store until the store goes, so a game
//! let go takes its store with it. Across two stores a game cannot call the
//! runner, so what it plays is kept in its own store as `env.tone`'s words,
//! the first [`TONES`] calls, and handed to the runner's `hear` when the
//! call returns ([`Game::heard`]); the runner judges them.

use wasmi::{Caller, Engine, Func, Global, Linker, Memory, Module, Store, StoreLimits, Val};

use crate::limits::{self, TONES};

/// The words of a call of `env.tone`.
pub(crate) type Tone = [i32; 7];

/// A game's call trapped: the runner stops it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Trapped;

/// The names the runner gives a prepared game's parts by.
#[derive(Default)]
pub(crate) struct Names {
    /// The functions it calls, by the index it calls each by.
    pub functions: Vec<String>,
    pub meter: String,
    pub memory: String,
    /// The module and name of the import a game plays its tones through.
    pub tone: [String; 2],
}

/// What a game's store holds: what it may grow to, and what it played
/// during the call that runs.
struct Played {
    limits: StoreLimits,
    tones: Vec<Tone>,
}

/// A game on its engine.
pub(crate) struct Game {
    store: Store<Played>,
    functions: Vec<Option<Func>>,
    memory: Memory,
    meter: Global,
}

impl Game {
    /// Compiles and instantiates a prepared file, its parts found by the
    /// runner's names: the engine's words for why not, when it would not.
    pub fn load(prepared: &[u8], names: &Names) -> Result<Game, String> {
        let engine = Engine::new(&limits::engine());
        let module = Module::new(&engine, prepared).map_err(|error| error.to_string())?;
        let played = Played {
            limits: limits::store(),
            tones: Vec::new(),
        };
        let mut store = Store::new(&engine, played);
        store.limiter(|played| &mut played.limits);
        let mut linker = <Linker<Played>>::new(&engine);
        let [module_name, name] = &names.tone;
        linker
            .func_wrap(
                module_name,
                name,
                |mut caller: Caller<'_, Played>,
                 channel: i32,
                 hz: i32,
                 ms: i32,
                 wave: i32,
                 to_hz: i32,
                 loud: i32,
                 to_loud: i32| {
                    let tones = &mut caller.data_mut().tones;
                    if tones.len() < TONES {
                        tones.push([channel, hz, ms, wave, to_hz, loud, to_loud]);
                    }
                },
            )
            .map_err(|error| error.to_string())?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|error| error.to_string())?;
        let functions = names
            .functions
            .iter()
            .map(|name| instance.get_func(&store, name))
            .collect();
        let missing = |name: &str| format!("no {name} after preparing");
        let memory = instance
            .get_memory(&store, &names.memory)
            .ok_or_else(|| missing(&names.memory))?;
        let meter = instance
            .get_global(&store, &names.meter)
            .ok_or_else(|| missing(&names.meter))?;
        Ok(Game {
            store,
            functions,
            memory,
            meter,
        })
    }

    /// Calls the function at `index` with `arg`, when it takes one: its
    /// result, 0 for none or for a function the game lacks.
    pub fn call(&mut self, index: u32, arg: u32) -> Result<u32, Trapped> {
        let Some(Some(function)) = self.functions.get(index as usize) else {
            return Ok(0);
        };
        let ty = function.ty(&self.store);
        let args = [Val::I32(arg as i32)];
        let mut results = [Val::I32(0)];
        let results = &mut results[..ty.results().len().min(1)];
        function
            .call(&mut self.store, &args[..ty.params().len().min(1)], results)
            .map_err(|_| Trapped)?;
        Ok(match results.first() {
            Some(Val::I32(got)) => *got as u32,
            _ => 0,
        })
    }

    /// What it played since last asked, at most [`TONES`] calls of it.
    pub fn heard(&mut self) -> Vec<Tone> {
        std::mem::take(&mut self.store.data_mut().tones)
    }

    /// `len` bytes of its memory from `at`; `None` when any lies outside.
    pub fn bytes(&self, at: i32, len: i32) -> Option<&[u8]> {
        let memory = self.memory.data(&self.store);
        memory.get(limits::range(at, len, memory.len())?)
    }

    pub fn meter(&self) -> Option<i32> {
        match self.meter.get(&self.store) {
            Val::I32(units) => Some(units),
            _ => None,
        }
    }

    pub fn set_meter(&mut self, units: i32) -> Result<(), String> {
        self.meter
            .set(&mut self.store, Val::I32(units))
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::{DEPTH, MEMORY, TABLE};

    /// The names a test game's parts go by, as the runner would give them.
    fn names() -> Names {
        Names {
            functions: vec!["run".into()],
            meter: "meter".into(),
            memory: "memory".into(),
            tone: ["env".into(), "tone".into()],
        }
    }

    /// A game whose memory starts at `pages`, whose table holds `table`
    /// elements, and whose `run` is `body`.
    fn game(pages: usize, table: usize, body: &str) -> Result<Game, String> {
        let text = format!(
            r#"(module
                (import "env" "tone" (func $tone (param i32 i32 i32 i32 i32 i32 i32)))
                (memory (export "memory") {pages})
                (table {table} funcref)
                (global (export "meter") (mut i32) (i32.const 0))
                (func $run (export "run") (param $n i32) (result i32) {body}))"#
        );
        Game::load(&wat::parse_str(text).expect("a test game parses"), &names())
    }

    const PAGES: usize = MEMORY / 65_536;

    #[test]
    fn memory_that_starts_past_the_limit_is_not_loaded() {
        assert!(game(PAGES, 0, "i32.const 0").is_ok());
        let refused = game(PAGES + 1, 0, "i32.const 0").err().expect("refused");
        assert!(!refused.is_empty());
    }

    #[test]
    fn memory_grown_past_the_limit_is_told_no() {
        let mut game = game(PAGES - 1, 0, "local.get $n memory.grow").unwrap();
        assert_eq!(game.call(0, 1), Ok(PAGES as u32 - 1), "to the limit");
        assert_eq!(game.call(0, 1), Ok(u32::MAX), "past it: -1");
    }

    #[test]
    fn a_table_past_the_limit_is_not_loaded() {
        assert!(game(1, TABLE, "i32.const 0").is_ok());
        assert!(game(1, TABLE + 1, "i32.const 0").is_err());
    }

    #[test]
    fn recursion_past_the_depth_traps_and_the_host_lives() {
        // Recurses `n` deep.
        let body = "local.get $n i32.eqz if (result i32) i32.const 0 else \
                    local.get $n i32.const 1 i32.sub call $run end";
        let mut game = game(1, 0, body).unwrap();
        assert_eq!(game.call(0, arcade::CALLS), Ok(0), "the contract's deepest");
        assert_eq!(game.call(0, DEPTH as u32), Err(Trapped));
        assert_eq!(game.call(0, 3), Ok(0), "and it runs on");
    }

    #[test]
    fn tones_past_the_first_calls_are_not_heard() {
        let body = "(loop $again \
                      local.get $n i32.const 0 i32.const 0 i32.const 0 \
                      i32.const 0 i32.const 0 i32.const 0 call $tone \
                      local.get $n i32.const 1 i32.sub local.tee $n br_if $again) \
                    i32.const 0";
        let mut game = game(1, 0, body).unwrap();
        game.call(0, TONES as u32 + 5).unwrap();
        let heard = game.heard();
        // Each call's channel is the count left, from the most down.
        let channels: Vec<i32> = heard.iter().map(|tone| tone[0]).collect();
        let made = TONES as i32 + 5;
        assert_eq!(
            channels,
            (made - TONES as i32 + 1..=made).rev().collect::<Vec<_>>()
        );
        assert!(game.heard().is_empty(), "taken");
    }

    #[test]
    fn bytes_outside_its_memory_are_none() {
        let game = game(1, 0, "i32.const 0").unwrap();
        assert_eq!(game.bytes(65_535, 1).map(<[u8]>::len), Some(1));
        assert_eq!(game.bytes(65_535, 2), None);
        assert_eq!(game.bytes(-1, 1), None);
    }
}
