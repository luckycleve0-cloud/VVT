//! Верхняя панель с иконками: инструменты, персонажи, лист, чат, настройки.

use macroquad::prelude::*;

use vtt_core::geom::Vec2;

use crate::draw::{rgba, text_width, Assets};
use crate::interact::Tool;

/// Высота панели.
pub const HEIGHT: f32 = 48.0;
/// Ширина одной кнопки.
const BUTTON: f32 = 40.0;

/// Действие верхней панели.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarAction {
    Tool(Tool),
    Grid,
    Roster,
    NewCharacter,
    Sheet,
    TokenPanel,
    Chat,
    Settings,
}

/// Текущее состояние кнопок.
#[derive(Debug, Clone, Copy)]
pub struct BarState {
    pub tool: Tool,
    pub grid: bool,
    pub roster: bool,
    pub sheet: bool,
    pub chat: bool,
    pub token: bool,
}

/// Нарисовать панель; вернуть нажатую кнопку.
pub fn draw(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    state: BarState,
) -> Option<BarAction> {
    draw_rectangle(0.0, 0.0, viewport.x, HEIGHT, rgba(14, 16, 20, 252));
    draw_line(0.0, HEIGHT, viewport.x, HEIGHT, 1.0, rgba(70, 80, 96, 255));

    // Название.
    crate::draw::text(assets, "VTT", 14.0, 31.0, 20.0, rgba(240, 200, 90, 255));

    let tools = [
        (BarAction::Tool(Tool::Select), "выбор"),
        (BarAction::Tool(Tool::Measure), "линейка"),
        (BarAction::Tool(Tool::Fog), "туман"),
        (BarAction::Tool(Tool::Ping), "пинг"),
        (BarAction::Tool(Tool::Wall), "стена"),
        (BarAction::Tool(Tool::Door), "дверь"),
    ];
    let windows = [
        (BarAction::Roster, "персонажи"),
        (BarAction::NewCharacter, "создать"),
        (BarAction::Sheet, "лист"),
        (BarAction::TokenPanel, "токен"),
        (BarAction::Chat, "чат"),
        (BarAction::Settings, "настройки"),
        (BarAction::Grid, "сетка"),
    ];

    let total = (tools.len() + windows.len()) as f32 * BUTTON;
    let mut x = viewport.x - total - 10.0;

    for (action, _) in tools {
        let active = matches!(action, BarAction::Tool(tool) if tool == state.tool);
        icon_button(assets, mouse, clicked, x, action, active, state);
        x += BUTTON;
    }
    // Разделитель.
    draw_line(
        x + 4.0,
        10.0,
        x + 4.0,
        HEIGHT - 10.0,
        1.0,
        rgba(70, 80, 96, 255),
    );
    x += 10.0;

    for (action, _) in windows {
        let active = match action {
            BarAction::Grid => state.grid,
            BarAction::Roster => state.roster,
            BarAction::Sheet => state.sheet,
            BarAction::Chat => state.chat,
            BarAction::TokenPanel => state.token,
            _ => false,
        };
        icon_button(assets, mouse, clicked, x, action, active, state);
        x += BUTTON;
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn icon_button(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    x: f32,
    action: BarAction,
    active: bool,
    state: BarState,
) -> bool {
    let rect = Rect::new(x + 2.0, 6.0, BUTTON - 4.0, HEIGHT - 12.0);
    let hovered = mouse.x >= rect.x
        && mouse.x <= rect.x + rect.w
        && mouse.y >= rect.y
        && mouse.y <= rect.y + rect.h;
    let bg = if active {
        rgba(58, 82, 120, 255)
    } else if hovered {
        rgba(48, 54, 66, 255)
    } else {
        rgba(22, 24, 30, 255)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    if active {
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            1.5,
            rgba(150, 190, 255, 255),
        );
    }
    let color = if active {
        rgba(235, 242, 255, 255)
    } else if hovered {
        rgba(240, 243, 250, 255)
    } else {
        rgba(190, 198, 214, 255)
    };
    draw_icon(action, rect.x + rect.w * 0.5, rect.y + rect.h * 0.5, color);

    if hovered {
        let label = label_of(action);
        let width = text_width(assets, label, 13.0);
        draw_rectangle(
            rect.x + rect.w * 0.5 - width * 0.5 - 6.0,
            HEIGHT + 4.0,
            width + 12.0,
            22.0,
            rgba(24, 27, 34, 245),
        );
        crate::draw::text(
            assets,
            label,
            rect.x + rect.w * 0.5 - width * 0.5,
            HEIGHT + 19.0,
            13.0,
            rgba(232, 236, 244, 255),
        );
    }
    let _ = state;
    clicked && hovered
}

fn label_of(action: BarAction) -> &'static str {
    match action {
        BarAction::Tool(Tool::Select) => "выбор",
        BarAction::Tool(Tool::Measure) => "линейка",
        BarAction::Tool(Tool::Fog) => "туман",
        BarAction::Tool(Tool::Ping) => "пинг",
        BarAction::Tool(Tool::Wall) => "стена",
        BarAction::Tool(Tool::Door) => "дверь",
        BarAction::Grid => "сетка",
        BarAction::Roster => "персонажи (L)",
        BarAction::NewCharacter => "создать (P)",
        BarAction::Sheet => "лист (Tab)",
        BarAction::TokenPanel => "токен (Y)",
        BarAction::Chat => "чат (T)",
        BarAction::Settings => "настройки (R)",
    }
}

fn draw_icon(action: BarAction, cx: f32, cy: f32, color: Color) {
    match action {
        BarAction::Tool(Tool::Select) => {
            draw_line(cx - 6.0, cy - 9.0, cx - 6.0, cy + 8.0, 2.0, color);
            draw_line(cx - 6.0, cy - 9.0, cx + 7.0, cy + 1.0, 2.0, color);
            draw_line(cx + 7.0, cy + 1.0, cx - 1.0, cy + 2.0, 2.0, color);
            draw_line(cx - 1.0, cy + 2.0, cx + 1.0, cy + 9.0, 2.0, color);
        }
        BarAction::Tool(Tool::Measure) => {
            draw_line(cx - 9.0, cy + 7.0, cx + 9.0, cy - 7.0, 2.0, color);
            for step in [-6.0, -2.0, 2.0, 6.0] {
                draw_line(
                    cx + step,
                    cy - step * 0.78,
                    cx + step - 3.0,
                    cy - step * 0.78 - 4.0,
                    1.5,
                    color,
                );
            }
        }
        BarAction::Tool(Tool::Fog) => {
            draw_circle(cx - 5.0, cy + 1.0, 5.0, color);
            draw_circle(cx + 1.0, cy - 2.0, 6.0, color);
            draw_circle(cx + 7.0, cy + 2.0, 4.0, color);
            draw_rectangle(cx - 10.0, cy + 1.0, 20.0, 5.0, color);
        }
        BarAction::Tool(Tool::Ping) => {
            draw_circle_lines(cx, cy, 8.0, 2.0, color);
            draw_circle_lines(cx, cy, 4.0, 2.0, color);
            draw_circle(cx, cy, 1.6, color);
        }
        BarAction::Tool(Tool::Wall) => {
            draw_line(cx - 9.0, cy - 6.0, cx + 9.0, cy - 6.0, 3.0, color);
            for step in [-6.0, 0.0, 6.0] {
                draw_line(cx + step, cy - 6.0, cx + step, cy + 2.0, 1.5, color);
            }
            draw_rectangle(cx - 9.0, cy + 4.0, 18.0, 2.0, color);
        }
        BarAction::Tool(Tool::Door) => {
            draw_rectangle_lines(cx - 9.0, cy - 8.0, 7.0, 16.0, 2.0, color);
            draw_line(cx - 9.0, cy - 8.0, cx + 8.0, cy - 8.0, 2.0, color);
            draw_line(cx - 9.0, cy + 8.0, cx + 8.0, cy + 8.0, 2.0, color);
            draw_circle(cx + 8.0, cy, 2.0, color);
        }
        BarAction::Grid => {
            for step in [-6.0, 0.0, 6.0] {
                draw_line(cx + step, cy - 9.0, cx + step, cy + 9.0, 1.2, color);
                draw_line(cx - 9.0, cy + step, cx + 9.0, cy + step, 1.2, color);
            }
        }
        BarAction::Roster => {
            draw_circle(cx - 4.0, cy - 5.0, 3.5, color);
            draw_circle(cx + 5.0, cy - 3.0, 3.0, color);
            draw_arc_lines(cx - 4.0, cy + 6.0, 8.0, 8.0, 200.0, 340.0, 2.5, color);
            draw_arc_lines(cx + 6.0, cy + 7.0, 6.0, 6.0, 200.0, 340.0, 2.0, color);
        }
        BarAction::NewCharacter => {
            draw_line(cx, cy - 8.0, cx, cy + 8.0, 2.5, color);
            draw_line(cx - 8.0, cy, cx + 8.0, cy, 2.5, color);
        }
        BarAction::Sheet => {
            draw_rectangle_lines(cx - 7.0, cy - 9.0, 14.0, 18.0, 2.0, color);
            for step in [-4.0, 1.0, 6.0] {
                draw_line(cx - 4.0, cy + step, cx + 4.0, cy + step, 1.2, color);
            }
        }
        BarAction::TokenPanel => {
            draw_circle_lines(cx, cy, 8.0, 2.0, color);
            draw_circle(cx, cy, 5.0, rgba(90, 120, 160, 255));
            draw_circle(cx - 2.0, cy - 2.0, 1.6, color);
        }
        BarAction::Chat => {
            draw_rectangle_lines(cx - 9.0, cy - 7.0, 18.0, 12.0, 2.0, color);
            draw_triangle(
                vec2(cx - 3.0, cy + 5.0),
                vec2(cx + 4.0, cy + 5.0),
                vec2(cx - 1.0, cy + 10.0),
                color,
            );
        }
        BarAction::Settings => {
            draw_circle_lines(cx, cy, 4.0, 2.0, color);
            for step in 0..8 {
                let angle = step as f32 * std::f32::consts::FRAC_PI_4;
                let (sin, cos) = angle.sin_cos();
                draw_line(
                    cx + cos * 5.0,
                    cy + sin * 5.0,
                    cx + cos * 9.0,
                    cy + sin * 9.0,
                    2.0,
                    color,
                );
            }
        }
    }
}

/// Дуга из отрезков (в `macroquad` нет толстой дуги).
#[allow(clippy::too_many_arguments)]
fn draw_arc_lines(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    start: f32,
    end: f32,
    thickness: f32,
    color: Color,
) {
    let steps = 12;
    let mut previous: Option<(f32, f32)> = None;
    for index in 0..=steps {
        let t = index as f32 / steps as f32;
        let angle = (start + (end - start) * t).to_radians();
        let point = vec2(cx + angle.cos() * rx, cy + angle.sin() * ry);
        if let Some((px, py)) = previous {
            draw_line(px, py, point.x, point.y, thickness, color);
        }
        previous = Some((point.x, point.y));
    }
}
