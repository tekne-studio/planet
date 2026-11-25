//! The runner on wasmi, and the engine it asks for: the imports of module
//! `engine`, of the version `arcade::ENGINE` names. Each game runs on an
//! engine and a store of its own ([`Game`]); what it plays is handed to the
//! runner's `hear` as each call returns, before the runner reads it, the
//! words as the game gave them.

use std::collections::HashMap;
use std::fmt;

use sha2::{Digest, Sha256};
use wasmi::{Caller, Engine, Linker, Memory, Module, Store, TypedFunc};

use crate::Kept;
use crate::game::{Game, Names, Trapped};
use crate::limits;

/// How many bytes the colours of a screen shown are: 256 of four bytes.
const COLOURS_BYTES: usize = 256 * 4;

/// Why the runner can serve no more: it trapped, or broke the calls it
/// makes. A fault of the runner, never of a game; the emulator is let go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Broken(pub String);

impl fmt::Display for Broken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the emulator's runner broke: {}", self.0)
    }
}

impl std::error::Error for Broken {}

impl From<wasmi::Error> for Broken {
    fn from(error: wasmi::Error) -> Broken {
        Broken(error.to_string())
    }
}

/// What a cartridge's cover says of it, when its file has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub title: String,
    pub maker: String,
    pub year: u16,
}

/// A cartridge's cover, as a shell shows it before anyone plays it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Covered {
    pub said: Option<Said>,
    /// The picture in colour, four bytes a pixel.
    pub rgba: Vec<u8>,
}

/// Bytes of a watched cart's memory, and a byte each, 1 where the last
/// updates changed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub bytes: Vec<u8>,
    pub changes: Vec<u8>,
}

/// A screen as the game wrote it, what an agent reads: its palette, and a
/// byte a pixel, each a colour of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    pub palette: Vec<[u8; 4]>,
    pub pixels: Vec<u8>,
}

/// A line of text in the console's font: a byte a pixel, a row after
/// another, 1 where the font inks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub width: u32,
    pub height: u32,
    pub ink: Vec<u8>,
}

/// The runner's `hear`: the seven words of a call of `env.tone`.
type Hear = TypedFunc<(i32, i32, i32, i32, i32, i32, i32), ()>;

/// What the runner's store holds: the games, by handle, and what the
/// engine's calls need of the runner.
#[derive(Default)]
struct Held {
    games: HashMap<i32, Game>,
    next: i32,
    why_not: String,
    names: Names,
    /// The runner's memory and `hear`, once it is instantiated.
    runner: Option<(Memory, Hear)>,
}

impl Held {
    fn runner(&self) -> Result<(Memory, Hear), wasmi::Error> {
        self.runner
            .ok_or_else(|| wasmi::Error::new("the engine called before the runner was hosted"))
    }

    fn game(&mut self, handle: i32) -> Result<&mut Game, wasmi::Error> {
        self.games
            .get_mut(&handle)
            .ok_or_else(|| wasmi::Error::new(format!("no game by the handle {handle}")))
    }
}

/// What the runner pointed at outside its own memory.
fn outside(at: i32, len: i32) -> wasmi::Error {
    wasmi::Error::new(format!(
        "the runner pointed outside its memory: {len} bytes from {at}"
    ))
}

/// `len` bytes of the runner's memory from `at`, copied out.
fn runner_bytes(caller: &Caller<'_, Held>, at: i32, len: i32) -> Result<Vec<u8>, wasmi::Error> {
    let (memory, _) = caller.data().runner()?;
    let data = memory.data(caller);
    let range = limits::range(at, len, data.len()).ok_or_else(|| outside(at, len))?;
    Ok(data[range].to_vec())
}

/// Writes `bytes` into the runner's memory at `at`.
fn write_runner(caller: &mut Caller<'_, Held>, at: i32, bytes: &[u8]) -> Result<(), wasmi::Error> {
    let (memory, _) = caller.data().runner()?;
    let len = bytes.len() as i32;
    let data = memory.data_mut(caller);
    let range = limits::range(at, len, data.len()).ok_or_else(|| outside(at, len))?;
    data[range].copy_from_slice(bytes);
    Ok(())
}

/// The engine the runner asks for, the imports of module `engine`.
fn engine_imports(linker: &mut Linker<Held>) -> Result<(), wasmi::Error> {
    linker.func_wrap(
        "engine",
        "load",
        |mut caller: Caller<'_, Held>, at: i32, len: i32| -> Result<i32, wasmi::Error> {
            let prepared = runner_bytes(&caller, at, len)?;
            let held = caller.data_mut();
            match Game::load(&prepared, &held.names) {
                Ok(game) => {
                    let handle = held.next;
                    held.next += 1;
                    held.games.insert(handle, game);
                    Ok(handle)
                }
                Err(why) => {
                    held.why_not = why;
                    Ok(-1)
                }
            }
        },
    )?;
    linker.func_wrap(
        "engine",
        "why_not",
        |mut caller: Caller<'_, Held>, at: i32, max: i32| -> Result<i32, wasmi::Error> {
            let said = caller.data().why_not.as_bytes();
            let said = said[..said.len().min(max as u32 as usize)].to_vec();
            write_runner(&mut caller, at, &said)?;
            Ok(said.len() as i32)
        },
    )?;
    linker.func_wrap(
        "engine",
        "call",
        |mut caller: Caller<'_, Held>,
         handle: i32,
         function: i32,
         arg: i32,
         out: i32|
         -> Result<i32, wasmi::Error> {
            let held = caller.data_mut();
            let (_, hear) = held.runner()?;
            let game = held.game(handle)?;
            let called = game.call(function as u32, arg as u32);
            // What it played, trapped or not, as the runner would have heard
            // it during the call.
            for [channel, hz, ms, wave, to_hz, loud, to_loud] in game.heard() {
                hear.call(&mut caller, (channel, hz, ms, wave, to_hz, loud, to_loud))?;
            }
            match called {
                Ok(got) => {
                    write_runner(&mut caller, out, &got.to_le_bytes())?;
                    Ok(0)
                }
                Err(Trapped) => Ok(1),
            }
        },
    )?;
    linker.func_wrap(
        "engine",
        "read",
        |mut caller: Caller<'_, Held>,
         handle: i32,
         from: i32,
         at: i32,
         len: i32|
         -> Result<i32, wasmi::Error> {
            let (memory, _) = caller.data().runner()?;
            let (into, held) = memory.data_and_store_mut(&mut caller);
            let into = limits::range(at, len, into.len())
                .map(|range| &mut into[range])
                .ok_or_else(|| outside(at, len))?;
            match held.game(handle)?.bytes(from, len) {
                Some(bytes) => {
                    into.copy_from_slice(bytes);
                    Ok(1)
                }
                None => Ok(0),
            }
        },
    )?;
    linker.func_wrap(
        "engine",
        "meter",
        |mut caller: Caller<'_, Held>, handle: i32| -> Result<i32, wasmi::Error> {
            let game = caller.data_mut().game(handle)?;
            game.meter()
                .ok_or_else(|| wasmi::Error::new("a meter not of i32"))
        },
    )?;
    linker.func_wrap(
        "engine",
        "set_meter",
        |mut caller: Caller<'_, Held>, handle: i32, units: i32| -> Result<(), wasmi::Error> {
            let game = caller.data_mut().game(handle)?;
            game.set_meter(units).map_err(wasmi::Error::new)
        },
    )?;
    linker.func_wrap(
        "engine",
        "drop",
        |mut caller: Caller<'_, Held>, handle: i32| {
            caller.data_mut().games.remove(&handle);
        },
    )?;
    Ok(())
}

/// The runner's exports, as the host calls them.
struct Exports {
    room: TypedFunc<i32, i32>,
    file: TypedFunc<i32, i32>,
    name: TypedFunc<i32, i32>,
    out: TypedFunc<(), i32>,
    check: TypedFunc<(), i32>,
    prepare: TypedFunc<(), i32>,
    open: TypedFunc<(i32, i32, i32, f64), ()>,
    open_prepared: TypedFunc<(i32, i32, i32, f64), ()>,
    still: TypedFunc<(i32, f64), ()>,
    fail: TypedFunc<(i32, f64), ()>,
    close: TypedFunc<i32, ()>,
    press: TypedFunc<(i32, i32), ()>,
    pace: TypedFunc<(i32, f64, f64), ()>,
    script: TypedFunc<i32, ()>,
    scripted: TypedFunc<i32, i32>,
    step: TypedFunc<(i32, i32), i32>,
    tick: TypedFunc<f64, i32>,
    idle: TypedFunc<(), i32>,
    stopped: TypedFunc<i32, i32>,
    updates: TypedFunc<i32, f64>,
    shown: TypedFunc<i32, i32>,
    read: TypedFunc<i32, i32>,
    sound: TypedFunc<i32, i32>,
    stop: TypedFunc<i32, i32>,
    frames: TypedFunc<i32, i32>,
    ran: TypedFunc<(i32, i32), i32>,
    size: TypedFunc<i32, i32>,
    window: TypedFunc<(i32, i32, i32), i32>,
    blocks: TypedFunc<i32, i32>,
    cover: TypedFunc<(), i32>,
    text: TypedFunc<(), i32>,
}

/// The emulator on wasmi, its runner instantiated: carts by a name the
/// shell gives each, time on one clock in milliseconds.
pub struct Emulator {
    store: Store<Held>,
    memory: Memory,
    runner: Exports,
    /// The version of the contract it runs games of.
    pub contract: u32,
    /// The updates a second a cart runs at its clock's pace of 1.
    pub ticks: u32,
    /// The most updates whose tones a cart keeps until [`Emulator::sound`] takes them.
    pub kept: u32,
    /// The words a tone is: `env.tone`'s arguments.
    pub words: u32,
    /// The keys a keyboard plays the buttons by, by the names a page's
    /// keyboard events give them, each to its button's bit.
    pub keys: Vec<(String, u32)>,
    /// A screen's pixels across and down.
    pub width: u32,
    pub height: u32,
    /// The meter's units one update may spend.
    pub budget: u32,
    /// The bytes of memory a byte of [`Emulator::blocks`] stands for.
    pub block: u32,
}

impl Emulator {
    /// Hosts `runner`, the bytes of `runner.wasm` (`arcade::RUNNER`); a
    /// runner that asks for another version of the engine is refused.
    pub fn hosted(runner: &[u8]) -> Result<Emulator, Broken> {
        let engine = Engine::default();
        let module = Module::new(&engine, runner)?;
        let mut store = Store::new(&engine, Held::default());
        let mut linker = <Linker<Held>>::new(&engine);
        engine_imports(&mut linker)?;
        let instance = linker.instantiate_and_start(&mut store, &module)?;
        let constant = |store: &mut Store<Held>, name: &str| -> Result<u32, Broken> {
            let ask = instance.get_typed_func::<(), i32>(&*store, name)?;
            Ok(ask.call(store, ())? as u32)
        };
        let asks = constant(&mut store, "engine")?;
        if asks != arcade::ENGINE {
            return Err(Broken(format!(
                "it asks for engine {asks}, and this host gives {}",
                arcade::ENGINE
            )));
        }
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| Broken("no memory".into()))?;
        let hear = instance.get_typed_func(&store, "hear")?;
        store.data_mut().runner = Some((memory, hear));
        let runner = Exports {
            room: instance.get_typed_func(&store, "room")?,
            file: instance.get_typed_func(&store, "file")?,
            name: instance.get_typed_func(&store, "name")?,
            out: instance.get_typed_func(&store, "out")?,
            check: instance.get_typed_func(&store, "check")?,
            prepare: instance.get_typed_func(&store, "prepare")?,
            open: instance.get_typed_func(&store, "open")?,
            open_prepared: instance.get_typed_func(&store, "open_prepared")?,
            still: instance.get_typed_func(&store, "still")?,
            fail: instance.get_typed_func(&store, "fail")?,
            close: instance.get_typed_func(&store, "close")?,
            press: instance.get_typed_func(&store, "press")?,
            pace: instance.get_typed_func(&store, "pace")?,
            script: instance.get_typed_func(&store, "script")?,
            scripted: instance.get_typed_func(&store, "scripted")?,
            step: instance.get_typed_func(&store, "step")?,
            tick: instance.get_typed_func(&store, "tick")?,
            idle: instance.get_typed_func(&store, "idle")?,
            stopped: instance.get_typed_func(&store, "stopped")?,
            updates: instance.get_typed_func(&store, "updates")?,
            shown: instance.get_typed_func(&store, "shown")?,
            read: instance.get_typed_func(&store, "read")?,
            sound: instance.get_typed_func(&store, "sound")?,
            stop: instance.get_typed_func(&store, "stop")?,
            frames: instance.get_typed_func(&store, "frames")?,
            ran: instance.get_typed_func(&store, "ran")?,
            size: instance.get_typed_func(&store, "size")?,
            window: instance.get_typed_func(&store, "window")?,
            blocks: instance.get_typed_func(&store, "blocks")?,
            cover: instance.get_typed_func(&store, "cover")?,
            text: instance.get_typed_func(&store, "text")?,
        };
        let mut emulator = Emulator {
            contract: constant(&mut store, "contract")?,
            ticks: constant(&mut store, "ticks")?,
            kept: constant(&mut store, "kept")?,
            words: constant(&mut store, "words")?,
            width: constant(&mut store, "width")?,
            height: constant(&mut store, "height")?,
            budget: constant(&mut store, "budget")?,
            block: constant(&mut store, "block")?,
            keys: Vec::new(),
            store,
            memory,
            runner,
        };
        let names = instance.get_typed_func::<(), i32>(&emulator.store, "names")?;
        let keys = instance.get_typed_func::<(), i32>(&emulator.store, "keys")?;
        let len = names.call(&mut emulator.store, ())?;
        let lines = emulator.text_out(len)?.unwrap_or_default();
        let lines: Vec<&str> = lines.split('\n').collect();
        let [meter, memory, module, tone] = lines[lines.len().saturating_sub(4)..] else {
            return Err(Broken("names without the game's parts".into()));
        };
        emulator.store.data_mut().names = Names {
            functions: lines[..lines.len() - 4]
                .iter()
                .map(|&name| name.into())
                .collect(),
            meter: meter.into(),
            memory: memory.into(),
            tone: [module.into(), tone.into()],
        };
        let len = keys.call(&mut emulator.store, ())?;
        emulator.keys = emulator
            .text_out(len)?
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let (key, bit) = line.split_once(' ')?;
                Some((key.to_owned(), bit.parse().ok()?))
            })
            .collect();
        Ok(emulator)
    }

    /// Writes `bytes` where the runner's next call reads them, at the room
    /// `room` (the runner's `room` or `name`) gives.
    fn put(&mut self, room: TypedFunc<i32, i32>, bytes: &[u8]) -> Result<(), Broken> {
        let len = i32::try_from(bytes.len()).map_err(|_| Broken("bytes past 2 GiB".into()))?;
        let at = room.call(&mut self.store, len)?;
        let data = self.memory.data_mut(&mut self.store);
        let range = limits::range(at, len, data.len())
            .ok_or_else(|| Broken(outside(at, len).to_string()))?;
        data[range].copy_from_slice(bytes);
        Ok(())
    }

    fn room(&mut self, bytes: &[u8]) -> Result<(), Broken> {
        self.put(self.runner.room, bytes)
    }

    /// Writes a cartridge's `file` for the runner's next call that reads
    /// one. A file past the most a file may have gets no room, a null
    /// pointer, and is never copied: the runner refuses it by its size.
    fn file(&mut self, file: &[u8]) -> Result<(), Broken> {
        // Past 4 GiB, any length past the most is refused the same.
        let len = u32::try_from(file.len()).unwrap_or(u32::MAX) as i32;
        let at = self.runner.file.call(&mut self.store, len)?;
        if at == 0 {
            return Ok(());
        }
        let data = self.memory.data_mut(&mut self.store);
        let range = limits::range(at, len, data.len())
            .ok_or_else(|| Broken(outside(at, len).to_string()))?;
        data[range].copy_from_slice(file);
        Ok(())
    }

    fn name(&mut self, name: &str) -> Result<(), Broken> {
        self.put(self.runner.name, name.as_bytes())
    }

    /// The bytes the call that gave `len` gave, copied out; `None` for -1.
    fn out(&mut self, len: i32) -> Result<Option<Vec<u8>>, Broken> {
        if len < 0 {
            return Ok(None);
        }
        let at = self.runner.out.call(&mut self.store, ())?;
        let data = self.memory.data(&self.store);
        let range = limits::range(at, len, data.len())
            .ok_or_else(|| Broken(outside(at, len).to_string()))?;
        Ok(Some(data[range].to_vec()))
    }

    fn text_out(&mut self, len: i32) -> Result<Option<String>, Broken> {
        Ok(self
            .out(len)?
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
    }

    fn numbers(&mut self, len: i32) -> Result<Option<Vec<u32>>, Broken> {
        Ok(self.out(len)?.map(|bytes| {
            bytes
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
                .collect()
        }))
    }

    /// The bytes a screen shown is: its colours, then a byte a pixel.
    fn shown_bytes(&self) -> usize {
        COLOURS_BYTES + (self.width * self.height) as usize
    }

    /// A screen shown in colour, four bytes a pixel: each pixel's byte
    /// looked up in the 256 colours before it, the whole of a host's
    /// drawing.
    fn drawn(&self, shown: &[u8]) -> Result<Vec<u8>, Broken> {
        if shown.len() != self.shown_bytes() {
            return Err(Broken(format!(
                "a screen of {} bytes, not {}",
                shown.len(),
                self.shown_bytes()
            )));
        }
        let (colours, pixels) = shown.split_at(COLOURS_BYTES);
        Ok(pixels
            .iter()
            .flat_map(|&index| {
                let at = usize::from(index) * 4;
                [
                    colours[at],
                    colours[at + 1],
                    colours[at + 2],
                    colours[at + 3],
                ]
            })
            .collect())
    }

    /// Why `file` breaks the contract, as a host judges it before keeping
    /// it: the code of why not, or `None` when it keeps it.
    pub fn check(&mut self, file: &[u8]) -> Result<Option<String>, Broken> {
        self.file(file)?;
        let len = self.runner.check.call(&mut self.store, ())?;
        Ok(self.text_out(len)?.filter(|code| !code.is_empty()))
    }

    /// `file` prepared, for a host that keeps prepared bytes by the file's
    /// hash; `None` when it breaks the contract.
    pub fn prepare(&mut self, file: &[u8]) -> Result<Option<Vec<u8>>, Broken> {
        self.file(file)?;
        let len = self.runner.prepare.call(&mut self.store, ())?;
        self.out(len)
    }

    /// Opens `file` as `id`, started with `seed`, its clock running from
    /// `now`: running, or stopped with its screen saying why. A cart
    /// `watched` is seen from inside.
    pub fn open(
        &mut self,
        id: u32,
        file: &[u8],
        seed: u32,
        now: f64,
        watched: bool,
    ) -> Result<(), Broken> {
        self.file(file)?;
        let args = (id as i32, seed as i32, i32::from(watched), now);
        Ok(self.runner.open.call(&mut self.store, args)?)
    }

    /// [`Emulator::open`] for bytes [`Emulator::prepare`] gave. Bytes of
    /// another shape open stopped, as a load that failed. Bytes of its
    /// shape that preparing never gave may run unmetered, an update that
    /// never ends: a host keeps prepared bytes only where no one but the
    /// user who runs it can write ([`Kept`]).
    pub fn open_prepared(
        &mut self,
        id: u32,
        prepared: &[u8],
        seed: u32,
        now: f64,
        watched: bool,
    ) -> Result<(), Broken> {
        self.room(prepared)?;
        let args = (id as i32, seed as i32, i32::from(watched), now);
        Ok(self.runner.open_prepared.call(&mut self.store, args)?)
    }

    /// [`Emulator::open`] through `kept`: the prepared bytes of `file`,
    /// kept under the hash of its bytes, read back when they are kept and
    /// whole, else prepared and kept. A file that breaks the contract opens
    /// stopped, saying why, and nothing is kept. Keeping never stands in
    /// the way: a write that fails leaves the next open to prepare again.
    pub fn open_kept(
        &mut self,
        kept: &Kept,
        id: u32,
        file: &[u8],
        seed: u32,
        now: f64,
        watched: bool,
    ) -> Result<(), Broken> {
        let name: String = Sha256::digest(file)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if let Some(prepared) = kept.get(&name) {
            return self.open_prepared(id, &prepared, seed, now, watched);
        }
        match self.prepare(file)? {
            Some(prepared) => {
                let _ = kept.put(&name, &prepared);
                self.open_prepared(id, &prepared, seed, now, watched)
            }
            None => self.open(id, file, seed, now, watched),
        }
    }

    /// Shows the cover of `file`, whose hash is `name`, as `id`, still: it
    /// never runs.
    pub fn still(&mut self, id: u32, file: &[u8], name: &str, now: f64) -> Result<(), Broken> {
        self.file(file)?;
        self.name(name)?;
        Ok(self.runner.still.call(&mut self.store, (id as i32, now))?)
    }

    /// Shows `id` as a cart whose file never came.
    pub fn fail(&mut self, id: u32, now: f64) -> Result<(), Broken> {
        Ok(self.runner.fail.call(&mut self.store, (id as i32, now))?)
    }

    /// Takes `id` off the shelf, its game let go.
    pub fn close(&mut self, id: u32) -> Result<(), Broken> {
        Ok(self.runner.close.call(&mut self.store, id as i32)?)
    }

    /// Holds the contract's `buttons` down on `id` from its next update on.
    pub fn press(&mut self, id: u32, buttons: u32) -> Result<(), Broken> {
        let args = (id as i32, buttons as i32);
        Ok(self.runner.press.call(&mut self.store, args)?)
    }

    /// Runs `id`'s clock at `pace` times the contract's from `now`; 0 holds it.
    pub fn pace(&mut self, id: u32, pace: f64, now: f64) -> Result<(), Broken> {
        let args = (id as i32, pace, now);
        Ok(self.runner.pace.call(&mut self.store, args)?)
    }

    /// Plays `buttons` on `id`, a byte an update, after any script it plays.
    pub fn script(&mut self, id: u32, buttons: &[u8]) -> Result<(), Broken> {
        self.room(buttons)?;
        Ok(self.runner.script.call(&mut self.store, id as i32)?)
    }

    /// How many updates of `id`'s script are left; `None` when it plays none.
    pub fn scripted(&mut self, id: u32) -> Result<Option<u32>, Broken> {
        let left = self.runner.scripted.call(&mut self.store, id as i32)?;
        Ok(u32::try_from(left).ok())
    }

    /// Runs `id`'s next `count` updates now, outside its clock, at most
    /// [`Emulator::kept`] a call: how many ran.
    pub fn step(&mut self, id: u32, count: u32) -> Result<u32, Broken> {
        let args = (id as i32, count as i32);
        Ok(self.runner.step.call(&mut self.store, args)? as u32)
    }

    /// Runs every cart's updates due by `now`: the carts whose screens changed.
    pub fn tick(&mut self, now: f64) -> Result<Vec<u32>, Broken> {
        let len = self.runner.tick.call(&mut self.store, now)?;
        Ok(self.numbers(len)?.unwrap_or_default())
    }

    /// Whether a tick would do nothing: no cart runs by its clock and every
    /// screen was said.
    pub fn idle(&mut self) -> Result<bool, Broken> {
        Ok(self.runner.idle.call(&mut self.store, ())? == 1)
    }

    /// Whether `id` stopped for good.
    pub fn stopped(&mut self, id: u32) -> Result<bool, Broken> {
        Ok(self.runner.stopped.call(&mut self.store, id as i32)? == 1)
    }

    /// How many updates `id` ran; `None` when no cart is there.
    pub fn updates(&mut self, id: u32) -> Result<Option<u64>, Broken> {
        let updates = self.runner.updates.call(&mut self.store, id as i32)?;
        Ok((updates >= 0.0).then_some(updates as u64))
    }

    /// What `id`'s screen is drawn from: 256 colours, then the pixels.
    pub fn shown(&mut self, id: u32) -> Result<Option<Vec<u8>>, Broken> {
        let len = self.runner.shown.call(&mut self.store, id as i32)?;
        self.out(len)
    }

    /// `id`'s screen in colour, four bytes a pixel.
    pub fn rgba(&mut self, id: u32) -> Result<Option<Vec<u8>>, Broken> {
        self.shown(id)?.map(|shown| self.drawn(&shown)).transpose()
    }

    /// `id`'s screen as the game wrote it, what an agent reads.
    pub fn read(&mut self, id: u32) -> Result<Option<Screen>, Broken> {
        let len = self.runner.read.call(&mut self.store, id as i32)?;
        let Some(mut bytes) = self.out(len)? else {
            return Ok(None);
        };
        // Four bytes a colour, then a byte a pixel.
        let pixels = (self.width * self.height) as usize;
        let at = bytes
            .len()
            .checked_sub(pixels)
            .filter(|at| at % 4 == 0)
            .ok_or_else(|| Broken(format!("a screen read of {} bytes", bytes.len())))?;
        let pixels = bytes.split_off(at);
        let palette = bytes
            .chunks_exact(4)
            .map(|colour| [colour[0], colour[1], colour[2], colour[3]])
            .collect();
        Ok(Some(Screen { palette, pixels }))
    }

    /// What `id` played since last asked, as the chip's processor takes it;
    /// empty when it ran no update.
    pub fn sound(&mut self, id: u32) -> Result<Vec<u32>, Broken> {
        let len = self.runner.sound.call(&mut self.store, id as i32)?;
        Ok(self.numbers(len)?.unwrap_or_default())
    }

    /// Each update's tones in a batch [`Emulator::sound`] gave: a list an
    /// update, in order, [`Emulator::words`] words a tone.
    pub fn tones<'a>(&self, batch: &'a [u32]) -> Vec<&'a [u32]> {
        let words = self.words as usize;
        let mut updates = Vec::new();
        let mut at = 1;
        while at < batch.len() {
            let count = batch[at] as usize;
            let end = (at + 1 + count * words).min(batch.len());
            updates.push(&batch[at + 1..end]);
            at = end;
        }
        updates
    }

    /// Why `id` stopped, as its screen says it; `None` while it runs.
    pub fn stop(&mut self, id: u32) -> Result<Option<String>, Broken> {
        let len = self.runner.stop.call(&mut self.store, id as i32)?;
        Ok(self.text_out(len)?.filter(|said| !said.is_empty()))
    }

    /// The frames hash of a watched `id`.
    pub fn frames(&mut self, id: u32) -> Result<Option<String>, Broken> {
        let len = self.runner.frames.call(&mut self.store, id as i32)?;
        self.text_out(len)
    }

    /// Every update a watched `id` ran from update `from`: its buttons and
    /// the units it spent, a pair of words each.
    pub fn ran(&mut self, id: u32, from: u32) -> Result<Option<Vec<u32>>, Broken> {
        let len = self
            .runner
            .ran
            .call(&mut self.store, (id as i32, from as i32))?;
        self.numbers(len)
    }

    /// The bytes of memory a watched `id` holds.
    pub fn size(&mut self, id: u32) -> Result<Option<u32>, Broken> {
        let size = self.runner.size.call(&mut self.store, id as i32)?;
        Ok(u32::try_from(size).ok())
    }

    /// `len` bytes of a watched `id`'s memory from `at`, cut where it ends.
    pub fn window(&mut self, id: u32, at: u32, len: u32) -> Result<Option<Window>, Broken> {
        let args = (id as i32, at as i32, len as i32);
        let len = self.runner.window.call(&mut self.store, args)?;
        Ok(self.out(len)?.map(|mut bytes| {
            let changes = bytes.split_off(bytes.len() / 2);
            Window { bytes, changes }
        }))
    }

    /// A watched `id`'s memory's map: a byte a block, 1 where the last
    /// updates changed it.
    pub fn blocks(&mut self, id: u32) -> Result<Option<Vec<u8>>, Broken> {
        let len = self.runner.blocks.call(&mut self.store, id as i32)?;
        self.out(len)
    }

    /// What a shell shows of `file`, whose hash is `name`, before it runs:
    /// read from the file, never by running it.
    pub fn cover(&mut self, file: &[u8], name: &str) -> Result<Covered, Broken> {
        self.file(file)?;
        self.name(name)?;
        let len = self.runner.cover.call(&mut self.store, ())?;
        let out = self.out(len)?.unwrap_or_default();
        let (shown, said) = out
            .split_at_checked(self.shown_bytes())
            .ok_or_else(|| Broken("a cover too short".into()))?;
        let rgba = self.drawn(shown)?;
        let mut said = said.iter().copied();
        let said = (said.len() > 0).then(|| {
            let year = u16::from_le_bytes([said.next().unwrap_or(0), said.next().unwrap_or(0)]);
            let mut text = || {
                let len = said.next().unwrap_or(0) as usize;
                let bytes: Vec<u8> = said.by_ref().take(len).collect();
                String::from_utf8_lossy(&bytes).into_owned()
            };
            let title = text();
            Said {
                title,
                maker: text(),
                year,
            }
        });
        Ok(Covered { said, rgba })
    }

    /// `line` in the console's font. A letter the font lacks keeps its
    /// place, blank.
    pub fn text(&mut self, line: &str) -> Result<Text, Broken> {
        self.room(line.as_bytes())?;
        let len = self.runner.text.call(&mut self.store, ())?;
        let out = self.out(len)?.unwrap_or_default();
        let (size, ink) = out
            .split_at_checked(4)
            .ok_or_else(|| Broken("a line of text without its size".into()))?;
        let width = u32::from(u16::from_le_bytes([size[0], size[1]]));
        let height = u32::from(u16::from_le_bytes([size[2], size[3]]));
        if ink.len() != (width * height) as usize {
            return Err(Broken(format!(
                "a line of {width} by {height} in {} bytes",
                ink.len()
            )));
        }
        Ok(Text {
            width,
            height,
            ink: ink.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runner that asks the engine for what its `go` names, with the
    /// pointers and lengths given, in a memory of one page.
    fn runner(body: &str) -> String {
        format!(
            r#"(module
                (import "engine" "load" (func $load (param i32 i32) (result i32)))
                (import "engine" "why_not" (func $why_not (param i32 i32) (result i32)))
                (import "engine" "call" (func $call (param i32 i32 i32 i32) (result i32)))
                (import "engine" "read" (func $read (param i32 i32 i32 i32) (result i32)))
                (memory (export "memory") 1)
                (func (export "hear") (param i32 i32 i32 i32 i32 i32 i32))
                (func (export "engine") (result i32) i32.const {engine})
                (func (export "go") (result i32) {body}))"#,
            engine = arcade::ENGINE
        )
    }

    /// Runs the runner's `go` on the host's engine: what it gave, or the
    /// error the engine stopped it with.
    fn go(body: &str) -> Result<i32, String> {
        let wasm = wat::parse_str(runner(body)).expect("a test runner parses");
        let engine = Engine::default();
        let module = Module::new(&engine, &wasm[..]).unwrap();
        let mut store = Store::new(&engine, Held::default());
        let mut linker = <Linker<Held>>::new(&engine);
        engine_imports(&mut linker).unwrap();
        let instance = linker.instantiate_and_start(&mut store, &module).unwrap();
        let memory = instance.get_memory(&store, "memory").unwrap();
        let hear = instance.get_typed_func(&store, "hear").unwrap();
        store.data_mut().runner = Some((memory, hear));
        let go = instance.get_typed_func::<(), i32>(&store, "go").unwrap();
        go.call(&mut store, ()).map_err(|error| error.to_string())
    }

    #[test]
    fn a_runner_pointing_outside_its_memory_is_stopped_never_followed() {
        let page = 65_536;
        for body in [
            format!("i32.const {} i32.const 2 call $load", page - 1),
            "i32.const -1 i32.const 1 call $load".into(),
            "i32.const 0 i32.const -1 call $load".into(),
            // Words for why, after a load that fails, past the memory's end.
            format!(
                "i32.const 0 i32.const 8 call $load drop \
                 i32.const {} i32.const 8 call $why_not",
                page - 4
            ),
        ] {
            let stopped = go(&body).expect_err(&body);
            assert!(stopped.contains("outside its memory"), "{body}: {stopped}");
        }
    }

    #[test]
    fn a_load_that_fails_says_why_within_the_room_given() {
        // Eight bytes that are no WebAssembly: no game, and words for why.
        let body = "i32.const 0 i32.const 8 call $load drop \
                    i32.const 0 i32.const 4 call $why_not";
        assert_eq!(go(body), Ok(4));
    }

    #[test]
    fn a_handle_never_given_is_the_runner_broken() {
        let body = "i32.const 7 i32.const 0 i32.const 0 i32.const 0 call $call";
        let stopped = go(body).expect_err("no game 7");
        assert!(stopped.contains("no game by the handle 7"), "{stopped}");
    }

    #[test]
    fn a_runner_that_asks_for_another_engine_is_refused() {
        let other = runner("i32.const 0").replace(
            &format!("i32.const {}", arcade::ENGINE),
            &format!("i32.const {}", arcade::ENGINE + 1),
        );
        let wasm = wat::parse_str(other).unwrap();
        let Err(Broken(why)) = Emulator::hosted(&wasm) else {
            panic!("hosted");
        };
        assert!(why.contains("asks for engine"), "{why}");
    }

    #[test]
    fn the_pinned_runner_is_hosted_and_writes_in_its_font() {
        let mut emulator = Emulator::hosted(arcade::RUNNER).unwrap();
        assert_eq!(
            (emulator.width, emulator.height),
            (arcade::WIDTH, arcade::HEIGHT)
        );
        let line = emulator.text("HI").unwrap();
        // Two letters of three, a pixel between them, five high.
        assert_eq!((line.width, line.height), (7, 5));
        // H's left column is inked down its height; the gap never is.
        assert!((0..5).all(|row| line.ink[row * 7] == 1));
        assert!((0..5).all(|row| line.ink[row * 7 + 3] == 0));
    }
}
