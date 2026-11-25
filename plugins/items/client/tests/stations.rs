//! A client and the world's word on who holds a cabinet's stations: one
//! told it holds a station it never asked for, as after an answer that came
//! past its wait or the plugin switched off and on, lets it go, or nobody
//! could play it.

use client::{Client, Input, Outbound, Recipe};
use items_world::{LOOK, RISE, STATIONS, wire};
use prost::Message;
use protocol::{client_message, server_message};

const ME: u32 = 7;
const CABINET: u64 = 0xcab;

fn server(message: server_message::Message) -> Vec<u8> {
    protocol::encode(&protocol::ServerMessage {
        message: Some(message),
    })
}

/// A client online as session [`ME`] in a world where items is on.
fn online() -> Client {
    let mut client = Client::new(Recipe::new(1)).unwrap();
    client.plug(items_client::plugin());
    client.link_opened();
    let recipe = client.recipe();
    client.receive(&server(server_message::Message::Welcome(
        protocol::Welcome {
            session: ME,
            recipe: Some(protocol::Recipe {
                seed: worldgen::format_seed(recipe.seed),
                generator_version: recipe.generator_version,
                params_json: serde_json::to_string(&recipe.params).unwrap(),
            }),
            plugins: vec![protocol::Plugin {
                name: items_client::NAME.into(),
                version: items_client::VERSION,
            }],
            ..Default::default()
        },
    )));
    turns(&mut client, 10);
    client.drain_outbound();
    client
}

fn turns(client: &mut Client, count: u32) {
    let mut input = Input::default();
    for _ in 0..count {
        client.update(1.0 / 60.0, &mut input);
    }
}

/// The ops of items this client sent: their kinds and payloads.
fn sent(client: &mut Client) -> Vec<(String, Vec<u8>)> {
    client
        .drain_outbound()
        .into_iter()
        .filter_map(|out| match out {
            Outbound::Frame(frame) => {
                match protocol::ClientMessage::decode(&frame[..]).ok()?.message? {
                    client_message::Message::Envelope(envelope)
                        if envelope.plugin == items_client::NAME =>
                    {
                        Some((envelope.kind, envelope.payload))
                    }
                    _ => None,
                }
            }
            Outbound::Close => None,
        })
        .collect()
}

/// The world says which plugins are on: items, or none.
fn items_on(on: bool) -> Vec<u8> {
    let items = protocol::Plugin {
        name: items_client::NAME.into(),
        version: items_client::VERSION,
    };
    server(server_message::Message::Plugins(protocol::Plugins {
        plugins: on.then_some(items).into_iter().collect(),
    }))
}

/// The world says who holds [`CABINET`]'s stations.
fn stations(holders: Vec<u32>) -> Vec<u8> {
    server(server_message::Message::Envelope(protocol::Envelope {
        plugin: items_client::NAME.into(),
        kind: STATIONS.into(),
        payload: wire::Stations {
            cabinet: CABINET,
            holders,
        }
        .encode_to_vec(),
        ..Default::default()
    }))
}

#[test]
fn a_station_held_here_and_never_asked_for_is_let_go() {
    let mut client = online();
    // Another's station is theirs to keep.
    client.receive(&stations(vec![3, 0]));
    turns(&mut client, 2);
    assert!(sent(&mut client).iter().all(|(kind, _)| kind != RISE));
    client.receive(&stations(vec![3, ME]));
    turns(&mut client, 2);
    let rise = wire::Rise { cabinet: CABINET }.encode_to_vec();
    assert!(
        sent(&mut client).contains(&(RISE.to_string(), rise)),
        "it rises from the station it does not play"
    );
}

#[test]
fn switched_off_and_on_it_asks_again_who_holds_the_stations() {
    let mut client = online();
    client.receive(&items_on(false));
    turns(&mut client, 2);
    sent(&mut client);
    client.receive(&items_on(true));
    turns(&mut client, 2);
    assert!(
        sent(&mut client).iter().any(|(kind, _)| kind == LOOK),
        "what it held before is the world's to say again"
    );
}
