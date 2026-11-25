//! What lives grows only in a world with life: a client no world has
//! spoken to draws no plant, and with life on, the same view is a wood.

use std::collections::HashSet;

use client::{Client, Command, Level, Recipe};
use scene::TerrainChange;

/// The shapes painted and the plants in the patches added, as the view
/// settles. Each plant names a shape painted before its patch came, and no
/// shape is painted twice.
fn drawn(client: &mut Client) -> (usize, usize) {
    client.settled_frame();
    let (mut painted, mut plants) = (HashSet::new(), 0);
    for change in client.drain_terrain_changes() {
        match change {
            TerrainChange::Paint(shape, _) => {
                assert!(painted.insert(shape), "shape {shape} painted twice");
            }
            TerrainChange::Add(_, mesh) => {
                for plant in &mesh.foliage {
                    assert!(
                        painted.contains(&plant.shape),
                        "shape {} unpainted",
                        plant.shape
                    );
                }
                plants += mesh.foliage.len();
            }
            TerrainChange::Unpaint => painted.clear(),
            TerrainChange::Remove(_) => {}
        }
    }
    (painted.len(), plants)
}

#[test]
fn plants_grow_only_with_life_and_come_when_it_does() {
    let mut client = Client::new(Recipe::new(0x8d7e_6661_1e10_84c0)).expect("a world");
    client.command(Command::GoTo {
        place: "0-JGC32FR".into(),
    });
    assert_eq!(
        drawn(&mut client),
        (0, 0),
        "no world has spoken: nothing lives"
    );
    client.rehearse(Level::Admin);
    let (shapes, plants) = drawn(&mut client);
    assert!(
        shapes > 0 && plants > 100,
        "{shapes} shapes, {plants} plants"
    );
}

/// Life going off with the link takes every plant out of the view, and the
/// view is a wood again when life comes back.
#[test]
fn plants_go_with_life_and_come_back_with_it() {
    let mut client = Client::new(Recipe::new(0x8d7e_6661_1e10_84c0)).expect("a world");
    client.command(Command::GoTo {
        place: "0-JGC32FR".into(),
    });
    client.rehearse(Level::Admin);
    let (_, plants) = drawn(&mut client);
    assert!(plants > 100, "{plants} plants with life on");
    client.link_closed();
    client.settled_frame();
    let changes = client.drain_terrain_changes();
    assert!(
        changes
            .iter()
            .any(|change| matches!(change, TerrainChange::Unpaint)),
        "life went off and the shapes stay painted"
    );
    let left: usize = changes
        .iter()
        .map(|change| match change {
            TerrainChange::Add(_, mesh) => mesh.foliage.len(),
            _ => 0,
        })
        .sum();
    assert_eq!(left, 0, "patches built with life off carry plants");
    client.rehearse(Level::Admin);
    let (shapes, plants) = drawn(&mut client);
    assert!(
        shapes > 0 && plants > 100,
        "{shapes} shapes, {plants} plants with life back"
    );
}
