//! The server's module: the systems of the core that run in the world, the
//! cells, the parts and the life of a world, and every world half of a
//! version, hosted by `world::Host`, built as one WASM file the Go server
//! runs through wazero.
//!
//! The server asks by [`serve`]: one call in, every reply out, as bytes in
//! the schema of `proto/planet/module/v1`. In the WASM build `abi` gives that
//! one function the names the server knows and asks the server for what an
//! owner keeps; natively a test calls it as it is, over a store held here.

use std::sync::Mutex;

use world::{Host, Measure};
use worldgen::{Field, Generator, Params, Recipe};

#[cfg(target_family = "wasm")]
mod abi;
#[cfg(target_family = "wasm")]
use abi::asked;
#[cfg(not(target_family = "wasm"))]
mod alone;
#[cfg(not(target_family = "wasm"))]
use alone::asked;

static HOST: Mutex<Option<Host>> = Mutex::new(None);

/// Serves one call of the server and gives back what was said, each reply
/// encoded. The host is made by the first call.
pub fn serve(call: &[u8]) -> Vec<Vec<u8>> {
    let said = HOST
        .lock()
        .expect("the server calls one call at a time")
        .get_or_insert_with(|| {
            let fixed = vec![world::cells::system(), world::parts::system()];
            // The life of a world is on until a world's admin turns it off.
            let life = world::Installed {
                plugin: world::life::system(),
                on: true,
            };
            let switched = core::iter::once(life).chain(plugins_world::all()).collect();
            Host::new(fixed, switched, measure_of, ground_of)
        })
        .serve(call, &mut asked);
    #[cfg(not(target_family = "wasm"))]
    alone::keep(&said);
    said
}

/// How a recipe says the size of its bodies. The wire's recipe carries no
/// size of its own (OPEN), so every world a server hosts is measured at the
/// generator's default, whatever else its recipe says.
fn measure_of(_recipe: &protocol::Recipe) -> Option<Measure> {
    Some(Measure {
        sphere: Recipe::new(0).sphere()?,
        moon: worldgen::moon(),
    })
}

/// The ground a recipe makes, for the systems that read it: where a volume
/// starts and ends. A world shaped by a field is made with the file the
/// server found for it. `None` for a recipe this generator cannot read, and
/// for a field that is not the one the recipe names.
fn ground_of(recipe: &protocol::Recipe, field: &[u8]) -> Option<Generator> {
    let params = match recipe.params_json.trim() {
        "" => Params::default(),
        json => serde_json::from_str(json).ok()?,
    };
    let seed = worldgen::parse_seed(&recipe.seed).ok()?;
    let recipe = Recipe {
        generator_version: recipe.generator_version,
        params,
        ..Recipe::new(seed)
    };
    match Generator::required_field(&recipe) {
        None => Generator::new(recipe).ok(),
        Some(_) => Generator::with_field(recipe, Field::parse(field).ok()?).ok(),
    }
}
