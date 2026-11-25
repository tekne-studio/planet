//! A horse ridden drives off the ground as it springs into a jump and
//! lands with a thud: each once a jump, never while it walks or stands,
//! and never a neigh.

use client::{Client, Input, Key, Level, Recipe};

fn client() -> Client {
    let mut client = Client::new(Recipe::new(1)).expect("a world");
    for plugin in plugins_client::all() {
        client.plug(plugin);
    }
    client.rehearse(Level::Admin);
    client.set_view([1600, 900]);
    client
}

/// What a jump is heard as: its springs and its landings.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
struct Heard {
    springs: usize,
    landings: usize,
}

/// Runs `seconds`, and says what of a jump was struck. A neigh fails.
fn heard(client: &mut Client, input: &mut Input, seconds: f64) -> Heard {
    let mut heard = Heard::default();
    for _ in 0..(seconds * 60.0).max(1.0) as u32 {
        client.update(1.0 / 60.0, input);
        client.drain_events();
        for strike in client.drain_strikes() {
            match strike.voice.name {
                "spring" => heard.springs += 1,
                "landing" => heard.landings += 1,
                "neigh" => panic!("a horse neighs nowhere in the world"),
                _ => {}
            }
        }
    }
    heard
}

fn jump(client: &mut Client, input: &mut Input) -> Heard {
    input.key(Key::Up, true);
    let pressed = heard(client, input, 1.0 / 60.0);
    input.key(Key::Up, false);
    let rest = heard(client, input, 2.0);
    Heard {
        springs: pressed.springs + rest.springs,
        landings: pressed.landings + rest.landings,
    }
}

const ONCE: Heard = Heard {
    springs: 1,
    landings: 1,
};

#[test]
fn a_horse_springs_and_lands_once_a_jump() {
    let mut client = client();
    let mut input = Input::default();
    client.go_to("0-JNAQ39R").unwrap();
    heard(&mut client, &mut input, 1.0);
    let horse = client
        .host("ride")
        .animals()
        .iter()
        .copied()
        .find(|near| !near.ridden)
        .expect("a horse near the spawn");
    let grid = client.generator().sphere().blocks();
    client.teleport(grid.surface_point(horse.at.to_array()));
    heard(&mut client, &mut input, 0.5);
    client.command_json(&format!(
        r#"{{"type":"act","plugin":"ride","id":{},"hold":true}}"#,
        horse.number
    ));
    assert_eq!(
        heard(&mut client, &mut input, 2.0),
        Heard::default(),
        "mounting"
    );
    assert_eq!(client.host("ride").way(), Some("horse"));

    assert_eq!(jump(&mut client, &mut input), ONCE, "from a halt");
    assert_eq!(jump(&mut client, &mut input), ONCE, "again");

    // At a walk, a while: nothing but the jump's.
    input.key(Key::Forward, true);
    heard(&mut client, &mut input, 1.0 / 60.0);
    input.key(Key::Forward, false);
    assert_eq!(
        heard(&mut client, &mut input, 2.0),
        Heard::default(),
        "walking"
    );
    assert_eq!(jump(&mut client, &mut input), ONCE, "from the walk");
}
