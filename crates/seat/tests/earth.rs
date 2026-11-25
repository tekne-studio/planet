//! A volume over the Earth is seated the same on both sides of the seam: by
//! the client, natively here as on a desktop, and by the server's module in
//! wazero (`server/internal/module/field_test.go`), each from its own copy
//! of the field. Both read `earth.json`: the world's seed and generator,
//! and a coast, the steepest plot by Aconcagua, the edge of a sector and the
//! sea, each seated or refused.

use seat::{Seat, Stand, survey};
use serde_json::Value;
use topology::{Sector, SurfacePoint};
use worldgen::{Field, Generator, Recipe, Source};

/// The Earth every checkout carries, as the image does.
const FIELD: &[u8] = include_bytes!("../../../assets/fields/earth.field");
const WORLD: &str = include_str!("earth.json");

#[test]
fn a_volume_over_the_earth_stands_where_the_server_seats_it() {
    let world: Value = serde_json::from_str(WORLD).expect("the world");
    let field = Field::parse(FIELD).expect("the Earth");
    let seed = worldgen::parse_seed(world["seed"].as_str().unwrap()).unwrap();
    let mut recipe = Recipe {
        generator_version: world["generator_version"].as_u64().unwrap() as u32,
        ..Recipe::new(seed)
    };
    recipe.params.source = Source::Field(field.id());
    let earth = Generator::with_field(recipe, field).expect("the field the recipe names");
    for stand in world["stands"].as_array().unwrap() {
        let place = &stand["place"];
        let sector = Sector::new(stand["sector"].as_u64().unwrap() as u8).unwrap();
        let [u, v] = ["u", "v"].map(|axis| stand[axis].as_f64().unwrap());
        let seated = survey(
            &earth,
            Seat::Sector(sector),
            SurfacePoint::new(sector, u, v),
        );
        match stand["refused"].as_str() {
            Some(why) => assert_eq!(seated.map_err(|why| why.code()), Err(why), "{place}"),
            None => assert_eq!(
                seated,
                Ok(Stand {
                    low: stand["low"].as_i64().unwrap() as i32,
                    height: stand["height"].as_u64().unwrap() as u32,
                }),
                "{place}"
            ),
        }
    }
}
