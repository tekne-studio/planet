//! What the host lets a game and the runner take, all of it here, each
//! limit reached by a test. The numbers are the emulator's manifest's
//! (`arcade`): a runner that asks more of its engine moves them with it.
//!
//! | Limit | Reached by |
//! |---|---|
//! | A game's memory: [`MEMORY`] bytes, whatever its file declares | `game::tests::memory_*` |
//! | A game's table: [`TABLE`] elements | `game::tests::a_table_past_the_limit_*` |
//! | A game's chain of calls: [`DEPTH`] calls, [`HEIGHT`] bytes of values | `game::tests::recursion_*`; the cases of the deepest chains |
//! | A game's tones: the first [`TONES`] calls of `env.tone` a call | `game::tests::tones_*` |
//! | One instance, one memory and one table a game | `game::tests::memory_*` |
//! | A range from the runner, or from a game through the runner ([`range`]) | `host::tests::*`; the cases of pointers past a memory |
//!
//! The stack is held past what the contract lets a chain reach, so the
//! game's own count stops it first and the engine never does: a game stops
//! the same on every host. A range outside the memory it names is never
//! read or written: outside the runner's, the runner broke; outside a
//! game's, the runner hears no and stops the game.

use std::ops::Range;

use wasmi::{CompilationMode, Config, StoreLimits, StoreLimitsBuilder};

/// The bytes of a game's memory.
pub const MEMORY: usize = arcade::PAGES as usize * 65_536;
/// The elements of a game's table.
pub const TABLE: usize = arcade::TABLE as usize;
/// The calls wasmi lets a chain hold: twice the contract's deepest, and the
/// host's own call into the game and `tone` beside.
pub const DEPTH: usize = 2 * arcade::CALLS as usize + 2;
/// The bytes wasmi lets its value stack take, eight a value: eight times
/// the most values a chain holds.
pub const HEIGHT: usize = 8 * 8 * arcade::VALUES as usize;
/// The calls of `env.tone` a game's call is heard for; the runner keeps the
/// count of the whole update.
pub const TONES: usize = arcade::TONES;

/// A game's engine: every function translated at once, so a module wasmi
/// cannot run stops when it is loaded, never in the middle of a game; and
/// a chain of calls held to [`DEPTH`] and [`HEIGHT`].
pub(crate) fn engine() -> Config {
    let mut config = Config::default();
    config
        .compilation_mode(CompilationMode::Eager)
        .set_max_recursion_depth(DEPTH)
        .set_max_stack_height(HEIGHT);
    config
}

/// What a game's store lets it hold: its memory and its table, and one of
/// each, of one instance.
pub(crate) fn store() -> StoreLimits {
    StoreLimitsBuilder::new()
        .memory_size(MEMORY)
        .table_elements(TABLE)
        .instances(1)
        .memories(1)
        .tables(1)
        .build()
}

/// `len` bytes from `at`, as a range of a memory of `size` bytes; `None`
/// when any of it lies outside. Both numbers are read as unsigned, as a
/// WebAssembly pointer is.
pub fn range(at: i32, len: i32, size: usize) -> Option<Range<usize>> {
    let at = at as u32 as usize;
    let end = at.checked_add(len as u32 as usize)?;
    (end <= size).then_some(at..end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_is_inside_its_memory_or_none() {
        assert_eq!(range(0, 16, 16), Some(0..16));
        assert_eq!(range(16, 0, 16), Some(16..16));
        assert_eq!(range(1, 16, 16), None);
        // A negative pointer or length is a large unsigned one.
        assert_eq!(range(-1, 1, 16), None);
        assert_eq!(range(0, -1, 16), None);
    }
}
