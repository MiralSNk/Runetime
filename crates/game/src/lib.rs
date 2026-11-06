//! Runetime game: arena logic and built-in spells. The window and drawing
//! live in the binary (`main.rs`).

pub mod arena;
pub mod grimoire;
pub mod text;

/// Built-in spell for scenario A of `docs/V0_1.md`: the working example from
/// `docs/SPEC.md`. The grimoire starts with it.
pub const SCENARIO_A: &str = "spell Fireball(target):
    let power <- 12
    let cell <- alloc(power)
    let dmg <- read(cell)
    emit Flame(at: target, power: dmg)
    free(cell)
end
";

/// Built-in spell for scenario B of `docs/V0_1.md`: the use-after-free
/// example from `docs/SPEC.md`.
pub const SCENARIO_B: &str = "spell Fireball(target):
    let power <- 12
    let cell <- alloc(power)
    free(cell)
    emit Flame(at: target, power: read(cell))
end
";
