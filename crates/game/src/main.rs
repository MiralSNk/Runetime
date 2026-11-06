//! Window, input and drawing: the arena on the left, the panel on the right.
//!
//! The panel shows the grimoire (Tab) or the journal of the last cast. On the
//! arena WASD moves the player, Space casts the built spell, R restarts.

use game::arena::{
    Arena, CAST_FUEL, ENEMY, ENTITY_RADIUS, MAX_HP, PLAYER, PROJECTILE_RADIUS, SIZE,
};
use game::grimoire::Grimoire;
use game::text::{self, Line, tr};
use macroquad::prelude::*;
use macroquad::ui::{Skin, hash, root_ui, widgets};

const PLAYER_SPEED: f32 = 200.0;
/// Longest simulated step, so a slow frame cannot tunnel through walls.
const MAX_DT: f32 = 1.0 / 30.0;
const PANEL_W: f32 = 480.0;
const PAD: f32 = 12.0;
const FONT_SIZE: u16 = 15;
const LINE_H: f32 = 19.0;
const BG: Color = Color::new(0.09, 0.09, 0.12, 1.0);
const PANEL_BG: Color = Color::new(0.13, 0.12, 0.16, 1.0);
const ERROR: Color = Color::new(1.0, 0.45, 0.4, 1.0);

#[derive(PartialEq)]
enum Mode {
    Arena,
    Grimoire,
}

fn window_conf() -> Conf {
    Conf {
        window_title: tr("window.title", &[]),
        window_width: (SIZE.x + PANEL_W) as i32,
        window_height: SIZE.y as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // The default macroquad font has no Cyrillic.
    let font = load_ttf_font_from_bytes(include_bytes!("../assets/DejaVuSansMono.ttf"))
        .expect("bundled font loads");
    let skin = skin(&font);
    root_ui().push_skin(&skin);
    let char_w = measure_text("M", Some(&font), FONT_SIZE, 1.0).width;
    let wrap_at = ((PANEL_W - 2.0 * PAD) / char_w) as usize;

    let mut arena = Arena::default();
    let mut grimoire = Grimoire::default();
    let mut journal = vec![Line {
        text: tr("journal.empty", &[]),
        error: false,
    }];
    let mut scroll = 0usize;
    let mut mode = Mode::Grimoire;

    loop {
        if is_key_pressed(KeyCode::Tab) {
            mode = if mode == Mode::Arena {
                Mode::Grimoire
            } else {
                Mode::Arena
            };
        }
        let dt = get_frame_time().min(MAX_DT);
        if mode == Mode::Arena {
            arena.move_player(movement() * PLAYER_SPEED * dt);
            if is_key_pressed(KeyCode::Space) && arena.entities[PLAYER].alive() {
                journal = match &grimoire.spell {
                    Some(spell) => text::journal(&arena.cast(spell), CAST_FUEL),
                    None => grimoire
                        .report
                        .iter()
                        .map(|t| Line {
                            text: t.clone(),
                            error: true,
                        })
                        .collect(),
                };
                scroll = 0;
            }
            if is_key_pressed(KeyCode::R) {
                arena = Arena::default();
            }
        }
        arena.update(dt);

        clear_background(BG);
        draw_arena(&arena);
        draw_rectangle(SIZE.x, 0.0, PANEL_W, SIZE.y, PANEL_BG);
        let mut pen = Pen {
            font: &font,
            x: SIZE.x + PAD,
            y: PAD + LINE_H,
            wrap_at,
        };
        match mode {
            Mode::Grimoire => {
                pen.line(&tr("grimoire.title", &[]), WHITE);
                let width = PANEL_W - 2.0 * PAD;
                widgets::Editbox::new(hash!(), vec2(width, 330.0))
                    .position(vec2(pen.x, pen.y))
                    .ui(&mut root_ui(), &mut grimoire.source);
                let clicked = widgets::Button::new(tr("grimoire.build", &[]))
                    .position(vec2(pen.x, pen.y + 340.0))
                    .size(vec2(120.0, 28.0))
                    .ui(&mut root_ui());
                if clicked {
                    grimoire.build();
                }
                pen.y += 400.0;
                let color = if grimoire.failed { ERROR } else { LIGHTGRAY };
                for t in &grimoire.report {
                    pen.line(t, color);
                }
                pen.hint(&tr("hint.grimoire", &[]));
            }
            Mode::Arena => {
                pen.line(&tr("journal.title", &[]), WHITE);
                let rows = ((SIZE.y - pen.y - 2.0 * LINE_H) / LINE_H) as usize;
                let wheel = mouse_wheel().1;
                if wheel < 0.0 {
                    scroll = (scroll + 3).min(journal.len().saturating_sub(rows));
                } else if wheel > 0.0 {
                    scroll = scroll.saturating_sub(3);
                }
                for l in journal.iter().skip(scroll) {
                    if pen.y > SIZE.y - 2.0 * LINE_H {
                        break;
                    }
                    pen.line(&l.text, if l.error { ERROR } else { LIGHTGRAY });
                }
                pen.hint(&tr("hint.arena", &[]));
            }
        }
        next_frame().await;
    }
}

fn movement() -> Vec2 {
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
    dir.normalize_or_zero()
}

/// UI skin with the bundled font for the editbox and the button.
fn skin(font: &Font) -> Skin {
    let ui = root_ui();
    let style = |bg: Color| {
        ui.style_builder()
            .with_font(font)
            .expect("bundled font loads")
            .font_size(FONT_SIZE)
            .text_color(WHITE)
            .text_color_hovered(WHITE)
            .text_color_clicked(WHITE)
            .color(bg)
            .color_hovered(bg)
            .color_clicked(bg)
            .color_selected(bg)
            .color_selected_hovered(bg)
            .build()
    };
    Skin {
        editbox_style: style(BG),
        button_style: style(Color::new(0.3, 0.25, 0.45, 1.0)),
        label_style: style(PANEL_BG),
        ..ui.default_skin()
    }
}

/// Writes wrapped lines top to bottom in the panel.
struct Pen<'a> {
    font: &'a Font,
    x: f32,
    y: f32,
    wrap_at: usize,
}

impl Pen<'_> {
    fn line(&mut self, text: &str, color: Color) {
        for row in text::wrap(text, self.wrap_at) {
            self.put(&row, self.y, color);
            self.y += LINE_H;
        }
    }

    /// Writes `text` at the bottom of the panel.
    fn hint(&self, text: &str) {
        let rows = text::wrap(text, self.wrap_at);
        for (i, row) in rows.iter().rev().enumerate() {
            self.put(row, SIZE.y - PAD - i as f32 * LINE_H, GRAY);
        }
    }

    fn put(&self, text: &str, y: f32, color: Color) {
        let params = TextParams {
            font: Some(self.font),
            font_size: FONT_SIZE,
            color,
            ..Default::default()
        };
        draw_text_ex(text, self.x, y, params);
    }
}

fn draw_arena(arena: &Arena) {
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
