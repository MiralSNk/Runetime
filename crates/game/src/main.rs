//! Window, input and drawing for the arena.
//!
//! WASD moves the player, 1 and 2 cast the built-in spells of scenarios A and
//! B, R restarts the arena.

use game::arena::{Arena, ENEMY, ENTITY_RADIUS, MAX_HP, PLAYER, PROJECTILE_RADIUS, SIZE};
use game::{SCENARIO_A, SCENARIO_B};
use macroquad::prelude::*;

const PLAYER_SPEED: f32 = 200.0;
/// Longest simulated step, so a slow frame cannot tunnel through walls.
const MAX_DT: f32 = 1.0 / 30.0;

fn window_conf() -> Conf {
    Conf {
        window_title: loc::get("window.title")
            .expect("loc has window.title")
            .to_owned(),
        window_width: SIZE.x as i32,
        window_height: SIZE.y as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let spells = [SCENARIO_A, SCENARIO_B].map(|src| {
        lang::compile(src)
            .expect("built-in spell compiles")
            .remove(0)
    });
    let mut arena = Arena::default();
    loop {
        let dt = get_frame_time().min(MAX_DT);
        let mut dir = Vec2::ZERO;
        for (key, d) in [
            (KeyCode::W, vec2(0.0, -1.0)),
            (KeyCode::S, vec2(0.0, 1.0)),
            (KeyCode::A, vec2(-1.0, 0.0)),
            (KeyCode::D, vec2(1.0, 0.0)),
        ] {
            if is_key_down(key) {
                dir += d;
            }
        }
        arena.move_player(dir.normalize_or_zero() * PLAYER_SPEED * dt);
        if arena.entities[PLAYER].alive() {
            // ponytail: the outcome (trace, errors) is dropped until the
            // journal arrives in step 7.
            if is_key_pressed(KeyCode::Key1) {
                arena.cast(&spells[0]);
            }
            if is_key_pressed(KeyCode::Key2) {
                arena.cast(&spells[1]);
            }
        }
        if is_key_pressed(KeyCode::R) {
            arena = Arena::default();
        }
        arena.update(dt);
        draw(&arena);
        next_frame().await;
    }
}

fn draw(arena: &Arena) {
    clear_background(Color::from_rgba(24, 22, 30, 255));
    for w in &arena.walls {
        draw_rectangle(w.x, w.y, w.w, w.h, GRAY);
    }
    for (i, e) in arena.entities.iter().enumerate() {
        let color = match (e.alive(), i) {
            (false, _) => DARKGRAY,
            (true, PLAYER) => SKYBLUE,
            (true, ENEMY) => RED,
            (true, _) => WHITE,
        };
        draw_circle(e.pos.x, e.pos.y, ENTITY_RADIUS, color);
        // HP bar above the entity, no text until loc covers the HUD.
        let width = 2.0 * ENTITY_RADIUS;
        let (x, y) = (e.pos.x - ENTITY_RADIUS, e.pos.y - ENTITY_RADIUS - 8.0);
        draw_rectangle(x, y, width, 4.0, MAROON);
        let filled = width * e.hp.clamp(0, MAX_HP) as f32 / MAX_HP as f32;
        draw_rectangle(x, y, filled, 4.0, GREEN);
    }
    for p in &arena.projectiles {
        draw_circle(p.pos.x, p.pos.y, PROJECTILE_RADIUS, ORANGE);
    }
}
