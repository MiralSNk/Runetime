//! Arena logic by the "Arena v0.1" section of `docs/SPEC.md`: a player, one
//! enemy, walls and projectiles. No drawing here, so it is testable headless.

use lang::bytecode::Chunk;
use macroquad::math::{Rect, Vec2, vec2};
use vm::{Cast, ErrorKind, Outcome, Point};

/// Entity index of the player, who is always the caster.
pub const PLAYER: usize = 0;
/// Entity index of the enemy, passed as the spell parameter.
pub const ENEMY: usize = 1;
/// Starting and maximum HP of every entity.
pub const MAX_HP: i64 = 10;
/// Fuel given to every cast.
pub const CAST_FUEL: u64 = 100;
/// Radius of an entity, in pixels.
pub const ENTITY_RADIUS: f32 = 12.0;
/// Radius of a projectile, in pixels.
pub const PROJECTILE_RADIUS: f32 = 4.0;
/// Arena size, in pixels.
pub const SIZE: Vec2 = vec2(800.0, 600.0);

const PROJECTILE_SPEED: f32 = 300.0;
const WALL: f32 = 20.0;
/// `R005` hits every entity this close to the caster.
const R005_RADIUS: f32 = 80.0;

/// A player or an enemy.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    /// Center, in pixels.
    pub pos: Vec2,
    /// Current HP; dead at 0 or below.
    pub hp: i64,
}

impl Entity {
    /// Whether the entity is alive.
    pub fn alive(&self) -> bool {
        self.hp > 0
    }

    /// Deals `power` damage; negative power heals up to [`MAX_HP`].
    fn hit(&mut self, power: i64) {
        self.hp = self.hp.saturating_sub(power).min(MAX_HP);
    }
}

/// A flying `Flame`.
#[derive(Debug, Clone, PartialEq)]
pub struct Projectile {
    /// Center, in pixels.
    pub pos: Vec2,
    /// Unit direction of flight.
    dir: Vec2,
    /// Distance left to the target point.
    left: f32,
    /// Damage on hit; negative heals.
    pub power: i64,
    /// Entity that cast it; a projectile never hits its caster.
    caster: usize,
}

/// The battle scene.
#[derive(Debug, Clone, PartialEq)]
pub struct Arena {
    /// Indexed by [`PLAYER`] and [`ENEMY`].
    pub entities: [Entity; 2],
    /// Wall rectangles.
    pub walls: Vec<Rect>,
    /// Projectiles in flight.
    pub projectiles: Vec<Projectile>,
}

impl Default for Arena {
    /// The v0.1 layout: border walls, a pillar above the line between the
    /// player (left) and the enemy (right).
    fn default() -> Self {
        let full = |hp| Entity {
            pos: hp,
            hp: MAX_HP,
        };
        Arena {
            entities: [full(vec2(150.0, 300.0)), full(vec2(650.0, 300.0))],
            walls: vec![
                Rect::new(0.0, 0.0, SIZE.x, WALL),
                Rect::new(0.0, SIZE.y - WALL, SIZE.x, WALL),
                Rect::new(0.0, 0.0, WALL, SIZE.y),
                Rect::new(SIZE.x - WALL, 0.0, WALL, SIZE.y),
                Rect::new(380.0, 100.0, 40.0, 150.0),
            ],
            projectiles: Vec::new(),
        }
    }
}

/// Whether a circle overlaps a rectangle.
fn overlaps(center: Vec2, radius: f32, rect: Rect) -> bool {
    let closest = center.clamp(rect.point(), rect.point() + rect.size());
    center.distance_squared(closest) < radius * radius
}

impl Arena {
    /// Moves the player by `delta` unless that would put it into a wall.
    /// The dead do not move.
    pub fn move_player(&mut self, delta: Vec2) {
        let player = &self.entities[PLAYER];
        let to = player.pos + delta;
        if player.alive() && !self.walls.iter().any(|&w| overlaps(to, ENTITY_RADIUS, w)) {
            self.entities[PLAYER].pos = to;
        }
    }

    /// Casts `chunk` from the player at the enemy and applies the outcome:
    /// effects first, then the game consequence of the error, if any.
    pub fn cast(&mut self, chunk: &Chunk) -> Outcome {
        let snapshot = self.entities.each_ref().map(|e| Point {
            x: e.pos.x.round() as i32,
            y: e.pos.y.round() as i32,
        });
        let cast = Cast {
            entities: &snapshot,
            caster: PLAYER as u32,
            args: &[ENEMY as u32],
            fuel: CAST_FUEL,
        };
        let outcome = vm::run(chunk, &cast);
        let kind = outcome.error.map(|e| e.kind);
        let inverted = kind == Some(ErrorKind::IntegerOverflow);
        for effect in &outcome.effects {
            let power = if inverted {
                effect.power.saturating_neg()
            } else {
                effect.power
            };
            self.flame(PLAYER, effect.at as usize, power);
        }
        match kind {
            Some(ErrorKind::FrameOverflow) => self.entities[PLAYER].hit(10),
            Some(ErrorKind::DoubleFree(_)) => self.entities[PLAYER].hit(5),
            Some(ErrorKind::DivisionByZero) => {
                let center = self.entities[PLAYER].pos;
                for e in &mut self.entities {
                    if e.pos.distance(center) <= R005_RADIUS {
                        e.hit(5);
                    }
                }
            }
            _ => {}
        }
        outcome
    }

    /// Spawns a projectile from `caster` towards where `at` stands now, or
    /// hits the caster at once when `at` is the caster.
    fn flame(&mut self, caster: usize, at: usize, power: i64) {
        let from = self.entities[caster].pos;
        match self.entities.get(at) {
            Some(target) if at != caster => self.projectiles.push(Projectile {
                pos: from,
                dir: (target.pos - from).normalize_or_zero(),
                left: from.distance(target.pos),
                power,
                caster,
            }),
            _ => self.entities[caster].hit(power),
        }
    }

    /// Advances projectiles by `dt` seconds: walls stop them, living
    /// entities other than the caster take their power, and they vanish at
    /// the target point.
    pub fn update(&mut self, dt: f32) {
        let mut projectiles = std::mem::take(&mut self.projectiles);
        projectiles.retain_mut(|p| {
            let step = (PROJECTILE_SPEED * dt).min(p.left);
            p.pos += p.dir * step;
            p.left -= step;
            if self
                .walls
                .iter()
                .any(|&w| overlaps(p.pos, PROJECTILE_RADIUS, w))
            {
                return false;
            }
            let reach = ENTITY_RADIUS + PROJECTILE_RADIUS;
            let hit = self
                .entities
                .iter_mut()
                .enumerate()
                .find(|(i, e)| *i != p.caster && e.alive() && e.pos.distance(p.pos) < reach);
            if let Some((_, e)) = hit {
                e.hit(p.power);
                return false;
            }
            p.left > 0.0
        });
        self.projectiles = projectiles;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SCENARIO_A, SCENARIO_B};

    fn chunk(src: &str) -> Chunk {
        lang::compile(src).expect("test spell compiles").remove(0)
    }

    fn settle(arena: &mut Arena) {
        for _ in 0..600 {
            arena.update(1.0 / 60.0);
        }
        assert!(arena.projectiles.is_empty());
    }

    #[test]
    fn scenario_a_fireball_kills_the_enemy() {
        let mut arena = Arena::default();
        let outcome = arena.cast(&chunk(SCENARIO_A));
        assert_eq!((outcome.error, outcome.fault), (None, None));
        settle(&mut arena);
        assert!(!arena.entities[ENEMY].alive());
        assert_eq!(arena.entities[PLAYER].hp, MAX_HP);
    }

    #[test]
    fn scenario_b_use_after_free_kills_the_caster() {
        let mut arena = Arena::default();
        let outcome = arena.cast(&chunk(SCENARIO_B));
        assert_eq!(outcome.fault.map(|f| f.kind.code()), Some("R003"));
        assert!(!arena.entities[PLAYER].alive());
        settle(&mut arena);
        assert_eq!(arena.entities[ENEMY].hp, MAX_HP);
    }

    #[test]
    fn wall_stops_the_projectile() {
        let mut arena = Arena::default();
        // From here the line to the enemy crosses the pillar.
        arena.entities[PLAYER].pos = vec2(150.0, 175.0);
        arena.cast(&chunk(SCENARIO_A));
        settle(&mut arena);
        assert_eq!(arena.entities[ENEMY].hp, MAX_HP);
    }

    #[test]
    fn error_consequences() {
        let double_free = "spell S(t):\n    let c <- alloc(1)\n    free(c)\n    free(c)\nend";
        let mut arena = Arena::default();
        arena.cast(&chunk(double_free));
        assert_eq!(arena.entities[PLAYER].hp, MAX_HP - 5);

        let div_zero = "spell S(t):\n    let z <- 0\n    let x <- 1 / z\nend";
        let mut arena = Arena::default();
        arena.cast(&chunk(div_zero));
        // The enemy is far away, so only the caster is in the R005 radius.
        assert_eq!(arena.entities[PLAYER].hp, MAX_HP - 5);
        assert_eq!(arena.entities[ENEMY].hp, MAX_HP);

        let overflow = "spell S(t):\n    emit Flame(at: t, power: 3)\n    let x <- 9223372036854775807 + 1\nend";
        let mut arena = Arena::default();
        arena.cast(&chunk(overflow));
        assert_eq!(arena.projectiles[0].power, -3);
    }
}
