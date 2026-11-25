//! Two machines play a game of two pads at one cabinet through a world the
//! Go server serves, each a whole client with every plugin, its game run by
//! the console as the desktop shell runs it: the frames each draws agree,
//! update by update. One whose window loses the keys keeps its station, one
//! drops and comes back, and a third finds no station.
//!
//! It needs a world served, alone so every session is its admin, and a
//! game of two pads built, so `cargo test` alone skips it: run it ignored,
//! with `PLANET_VERSUS_ORIGIN` the world's `host:port` and
//! `PLANET_VERSUS_CART` the game's file.

use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use arcade_host::Emulator;
use client::{Client, Input, Outbound, Recipe, Running};
use scene::PictureId;

/// Where the world is served, `host:port`.
const ORIGIN: &str = "PLANET_VERSUS_ORIGIN";
/// The game's file.
const CART: &str = "PLANET_VERSUS_CART";
/// A cart's address, as a cabinet's cartridge names one: the shell here
/// opens the game's file whatever it names.
const NAMED: &str = "asset://2222222222222222222222222222222222222222222222222222222222222222.wasm";
const TURN_S: f64 = 1.0 / 60.0;

#[test]
#[ignore = "needs a world served and a game of two pads: bun run versus"]
fn two_machines_play_one_game_through_a_world_and_draw_the_same_frames() {
    let origin = std::env::var(ORIGIN).expect(ORIGIN);
    let cart = std::fs::read(std::env::var(CART).expect(CART)).expect("the game's file");
    found(&origin);
    let mut a = Machine::enter(&origin, &cart);
    let mut b = Machine::enter(&origin, &cart);
    turns(&mut [&mut a, &mut b], 60, |_| []);
    // A places a cabinet and plays it; B, beside it, plays it too.
    a.client.command_json(&format!(
        r#"{{"type":"items.games","games":[{{"name":"versus","cart":"{NAMED}"}}]}}"#
    ));
    a.client
        .command_json(r#"{"type":"items.take","item":"cabinet","ahead":true}"#);
    turns(&mut [&mut a, &mut b], 60, |_| []);
    a.client.command_json(r#"{"type":"items.edit","on":false}"#);
    turns(&mut [&mut a, &mut b], 2, |_| []);
    a.client.command_json(r#"{"type":"act"}"#);
    turns(&mut [&mut a, &mut b], 90, |_| []);
    b.client.command_json(r#"{"type":"act"}"#);
    // Each presses buttons of their own, a pattern each.
    let pressed = |turn: u64| -> [&'static [&'static str]; 2] {
        [
            match turn / 9 % 3 {
                0 => &["left"],
                1 => &["right"],
                _ => &[],
            },
            match turn / 13 % 3 {
                0 => &["up"],
                1 => &["down", "jump"],
                _ => &[],
            },
        ]
    };
    turns(&mut [&mut a, &mut b], 720, pressed);
    let agreed = agree(&a, &b);
    let (fed_a, fed_b) = (a.frames.len(), b.frames.len());
    assert!(
        agreed >= 600,
        "{agreed} updates agreed, {fed_a} and {fed_b} fed, {:?} and {:?} open",
        a.opened,
        b.opened
    );

    // A third, beside them, finds no station: nothing opens for them.
    let mut c = Machine::enter(&origin, &cart);
    turns(&mut [&mut a, &mut b, &mut c], 60, pressed);
    c.client.command_json(r#"{"type":"items.edit","on":false}"#);
    c.client.command_json(r#"{"type":"act"}"#);
    turns(&mut [&mut a, &mut b, &mut c], 60, pressed);
    assert!(c.opened.is_empty(), "a third plays nothing");
    drop(c);

    // B's window loses the keys: its station kept, its pad released, the
    // game runs on for both. The keys back, B presses again, nothing asked.
    b.client.away(true);
    turns(&mut [&mut a, &mut b], 120, pressed);
    let away = agree(&a, &b);
    assert!(away > agreed + 100, "away, B ran on: {agreed} then {away}");
    b.client.away(false);
    turns(&mut [&mut a, &mut b], 120, pressed);
    let back = agree(&a, &b);
    assert!(back > away + 100, "back, B plays: {away} then {back}");
    let agreed = back;

    // B's link drops; A plays on alone. B comes back and plays again from
    // the station it held, its game run at once through what it missed.
    b.drop_link();
    turns(&mut [&mut a, &mut b], 120, pressed);
    b.link(&origin);
    // Back, it hears the world again before its cabinet is offered.
    turns(&mut [&mut a, &mut b], 120, pressed);
    b.client.command_json(r#"{"type":"act"}"#);
    turns(&mut [&mut a, &mut b], 300, pressed);
    let again = agree(&a, &b);
    assert!(
        again > agreed + 300,
        "B came back: {agreed} then {again} agreed"
    );
    eprintln!("{agreed} updates agreed, then {again} once B came back");
}

/// Runs `count` turns of every machine, each pressing what `pressed` says
/// at its station, in real time: what the world relays comes as it would.
fn turns<const N: usize>(
    machines: &mut [&mut Machine],
    count: u64,
    pressed: impl Fn(u64) -> [&'static [&'static str]; N],
) {
    for _ in 0..count {
        let started = Instant::now();
        for (index, machine) in machines.iter_mut().enumerate() {
            let held = pressed(machine.turn).get(index).copied().unwrap_or(&[]);
            machine.turn(held);
        }
        if let Some(rest) = Duration::from_secs_f64(TURN_S).checked_sub(started.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// The updates two machines both ran, each frame the same on both.
fn agree(a: &Machine, b: &Machine) -> u64 {
    let mut agreed = 0;
    for (updates, hash) in &a.frames {
        if let Some(theirs) = b.frames.get(updates) {
            assert_eq!(hash, theirs, "the frames differ by update {updates}");
            agreed = agreed.max(*updates);
        }
    }
    agreed
}

/// One machine: a client, its link to the world, and the console that runs
/// its games, watched so each update's frames are hashed.
struct Machine {
    client: Client,
    socket: Option<Socket>,
    key: String,
    emulator: Emulator,
    cart: Vec<u8>,
    /// The games open, by the picture each is drawn in.
    opened: BTreeMap<PictureId, u32>,
    next: u32,
    /// The frames hash of the game played by each update count it ran to.
    frames: BTreeMap<u64, String>,
    turn: u64,
    pressed: &'static [&'static str],
}

impl Machine {
    fn enter(origin: &str, cart: &[u8]) -> Machine {
        let key = entered(origin);
        let mut client = Client::new(Recipe::new(1)).unwrap();
        for plugin in plugins_client::all() {
            client.plug(plugin);
        }
        client.set_view([1600, 900]);
        let mut machine = Machine {
            client,
            socket: None,
            key,
            emulator: Emulator::hosted(arcade::RUNNER).unwrap(),
            cart: cart.to_vec(),
            opened: BTreeMap::new(),
            next: 1,
            frames: BTreeMap::new(),
            turn: 0,
            pressed: &[],
        };
        machine.link(origin);
        machine
    }

    fn link(&mut self, origin: &str) {
        let path = match self.key.as_str() {
            "" => "/api/socket".to_string(),
            key => format!("/api/socket?key={key}"),
        };
        self.socket = Some(Socket::open(origin, &path));
        self.client.link_opened();
    }

    fn drop_link(&mut self) {
        self.socket = None;
        self.client.link_closed();
    }

    fn turn(&mut self, pressed: &'static [&'static str]) {
        self.turn += 1;
        if let Some(socket) = &mut self.socket {
            for frame in socket.frames() {
                self.client.receive(&frame);
            }
        }
        if pressed != self.pressed {
            let names: Vec<String> = pressed.iter().map(|name| format!("\"{name}\"")).collect();
            self.client.command_json(&format!(
                r#"{{"type":"press","controls":[{}]}}"#,
                names.join(",")
            ));
            self.pressed = pressed;
        }
        self.client.update(TURN_S, &mut Input::default());
        self.client.drain_events();
        for out in self.client.drain_outbound() {
            if let (Outbound::Frame(frame), Some(socket)) = (out, &mut self.socket) {
                socket.send(&frame);
            }
        }
        for running in self.client.drain_running() {
            self.run(running);
        }
    }

    /// Runs what the client asks of its games, as the desktop shell does.
    fn run(&mut self, running: Running) {
        let now = self.turn as f64 * TURN_S * 1000.0;
        match running {
            Running::Open {
                picture, seed, fed, ..
            } => {
                let id = self.fresh();
                self.emulator.open(id, &self.cart, seed, now, true).unwrap();
                if fed {
                    self.emulator.pace(id, 0.0, now).unwrap();
                }
                self.opened.insert(picture, id);
                self.said(picture);
            }
            Running::Cover { picture, name, .. } => {
                let id = self.fresh();
                self.emulator.still(id, &self.cart, &name, now).unwrap();
                self.said(picture);
            }
            Running::Feed {
                picture, buttons, ..
            } => {
                let Some(&id) = self.opened.get(&picture) else {
                    return;
                };
                self.emulator.script(id, &buttons).unwrap();
                while self.emulator.scripted(id).unwrap().unwrap_or(0) > 0 {
                    let left = self.emulator.scripted(id).unwrap().unwrap_or(0);
                    if self.emulator.step(id, left).unwrap() == 0 {
                        break;
                    }
                }
                let updates = self.emulator.updates(id).unwrap().unwrap_or(0);
                let hash = self.emulator.frames(id).unwrap().unwrap_or_default();
                self.frames.insert(updates, hash);
            }
            Running::Close(picture) => {
                if let Some(id) = self.opened.remove(&picture) {
                    self.emulator.close(id).unwrap();
                }
            }
            _ => {}
        }
    }

    /// A name for a game on the console, never one given before.
    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next
    }

    /// Tells the client a game's screen and the pads its file plays.
    fn said(&mut self, picture: PictureId) {
        if let Some(plays) = self.emulator.plays(&self.cart).unwrap() {
            self.client.running_plays(picture, plays);
        }
        self.client
            .running_opened(picture, Ok((arcade::WIDTH, arcade::HEIGHT)));
    }
}

/// Founds the world served, as its admin, with the plugin of cabinets on.
fn found(origin: &str) {
    let key = entered(origin);
    let recipe = r#"{"seed":"0000000000000001","generator_version":3,"params":{}}"#;
    let (status, said) = http(origin, "POST", "/api/world", &key, recipe);
    assert!(status < 300 || status == 409, "founding: {status} {said}");
    let on = r#"{"name":"items","on":true}"#;
    let (status, said) = http(origin, "POST", "/api/plugins", &key, on);
    assert!(status < 300, "items on: {status} {said}");
}

/// A key to the world served, a session's: none in a world that runs
/// alone, where every session has the level it gives.
fn entered(origin: &str) -> String {
    let (_, entered) = http(origin, "POST", "/api/enter", "", "{}");
    between(&entered, "\"key\":\"", "\"")
        .unwrap_or_default()
        .to_string()
}

/// A request over HTTP/1.1 with `key`, if any, as its bearer: its answer's
/// status and body.
fn http(origin: &str, method: &str, path: &str, key: &str, body: &str) -> (u32, String) {
    let bearer = match key {
        "" => String::new(),
        key => format!("Authorization: Bearer {key}\r\n"),
    };
    let mut stream = TcpStream::connect(origin).expect("the world is served");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {origin}\r\n{bearer}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    let status = answer
        .get(9..12)
        .and_then(|status| status.parse().ok())
        .unwrap_or(0);
    let body = answer
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string());
    (status, body.unwrap_or_default())
}

fn between<'a>(text: &'a str, from: &str, to: &str) -> Option<&'a str> {
    let start = text.find(from)? + from.len();
    let end = text[start..].find(to)? + start;
    Some(&text[start..end])
}

/// A WebSocket client, as much of one as a world's link asks: binary
/// frames, masked out, whole in.
struct Socket {
    stream: TcpStream,
    read: Vec<u8>,
}

impl Socket {
    fn open(origin: &str, path: &str) -> Socket {
        let mut stream = TcpStream::connect(origin).expect("the world is served");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )
        .unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        assert!(
            head.starts_with(b"HTTP/1.1 101"),
            "{}",
            String::from_utf8_lossy(&head)
        );
        stream.set_nonblocking(true).unwrap();
        stream.set_nodelay(true).unwrap();
        Socket {
            stream,
            read: Vec::new(),
        }
    }

    fn send(&mut self, payload: &[u8]) {
        let mut frame = vec![0x82];
        match payload.len() {
            len @ 0..=125 => frame.push(0x80 | len as u8),
            len @ 126..=0xffff => {
                frame.push(0x80 | 126);
                frame.extend((len as u16).to_be_bytes());
            }
            len => {
                frame.push(0x80 | 127);
                frame.extend((len as u64).to_be_bytes());
            }
        }
        let mask = [0x5a, 0xa5, 0x3c, 0xc3];
        frame.extend(mask);
        frame.extend(
            payload
                .iter()
                .zip(mask.iter().cycle())
                .map(|(byte, mask)| byte ^ mask),
        );
        self.stream.set_nonblocking(false).unwrap();
        self.stream.write_all(&frame).unwrap();
        self.stream.set_nonblocking(true).unwrap();
    }

    /// The binary frames come whole since the last call.
    fn frames(&mut self) -> Vec<Vec<u8>> {
        let mut chunk = [0u8; 65536];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => self.read.extend_from_slice(&chunk[..read]),
                Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                Err(error) => panic!("{error}"),
            }
        }
        let mut frames = Vec::new();
        while let Some((opcode, payload, used)) = whole(&self.read) {
            self.read.drain(..used);
            match opcode {
                0x2 => frames.push(payload),
                0x9 => {
                    // A pong, empty and masked as every frame out is.
                    let _ = self.stream.write_all(&[0x8a, 0x80, 0, 0, 0, 0]);
                }
                _ => {}
            }
        }
        frames
    }
}

/// The first frame whole in `bytes`: its opcode, payload and length.
fn whole(bytes: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    let (&first, &second) = (bytes.first()?, bytes.get(1)?);
    let (len, at) = match second & 0x7f {
        126 => (
            u16::from_be_bytes(bytes.get(2..4)?.try_into().ok()?) as usize,
            4,
        ),
        127 => (
            u64::from_be_bytes(bytes.get(2..10)?.try_into().ok()?) as usize,
            10,
        ),
        len => (len as usize, 2),
    };
    let payload = bytes.get(at..at + len)?.to_vec();
    Some((first & 0x0f, payload, at + len))
}
