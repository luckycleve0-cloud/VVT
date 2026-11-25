//! Левая панель со списком персонажей кампании (как «Characters» в Roll20).

use macroquad::prelude::*;

use vtt_core::entities::Token;
use vtt_core::geom::Vec2;
use vtt_core::ids::EntityId;
use vtt_core::scp::ScpCharacter;

use crate::draw::{rgba, text, text_width, Assets};

/// Ширина панели.
pub const WIDTH: f32 = 260.0;

/// Действие панели.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RosterAction {
    /// Открыть лист персонажа.
    Open(EntityId),
    /// Открыть мастер создания.
    New,
    /// Скрыть панель.
    Close,
    /// Поставить токен персонажа в центр сцены.
    Place(EntityId),
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn contains(self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.w
            && point.y >= self.y
            && point.y <= self.y + self.h
    }
}

fn small_button(assets: &Assets, mouse: Vec2, clicked: bool, rect: Rect, label: &str) -> bool {
    let hovered = rect.contains(mouse);
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if hovered {
            rgba(70, 82, 104, 255)
        } else {
            rgba(40, 46, 58, 255)
        },
    );
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, rgba(80, 90, 108, 255));
    let width = text_width(assets, label, 15.0);
    text(
        assets,
        label,
        rect.x + (rect.w - width) * 0.5,
        rect.y + rect.h * 0.5 + 5.0,
        15.0,
        rgba(230, 234, 242, 255),
    );
    clicked && hovered
}

/// Нарисовать панель; вернуть действия этого кадра.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    characters: &[ScpCharacter],
    tokens: &[Token],
    selected: Option<EntityId>,
    top: f32,
) -> Vec<RosterAction> {
    let mut actions = Vec::new();
    draw_rectangle(0.0, top, WIDTH, viewport.y - top, rgba(16, 18, 22, 255));
    draw_line(WIDTH, top, WIDTH, viewport.y, 2.0, rgba(70, 80, 96, 255));

    text(
        assets,
        "Персонажи",
        WIDTH - 180.0,
        top + 32.0,
        22.0,
        rgba(238, 242, 250, 255),
    );

    if small_button(
        assets,
        mouse,
        clicked,
        Rect {
            x: 14.0,
            y: top + 46.0,
            w: 150.0,
            h: 30.0,
        },
        "+ Персонаж",
    ) {
        actions.push(RosterAction::New);
    }
    if small_button(
        assets,
        mouse,
        clicked,
        Rect {
            x: WIDTH - 46.0,
            y: top + 46.0,
            w: 32.0,
            h: 30.0,
        },
        "×",
    ) {
        actions.push(RosterAction::Close);
    }

    text(
        assets,
        "Characters",
        WIDTH - 180.0,
        top + 104.0,
        16.0,
        rgba(150, 158, 174, 255),
    );

    let mut y = top + 116.0;
    if characters.is_empty() {
        text(
            assets,
            "Пока никого. Нажмите «+ Персонаж».",
            14.0,
            y + 18.0,
            14.0,
            rgba(140, 148, 164, 255),
        );
    }

    for character in characters {
        let row = Rect {
            x: 8.0,
            y,
            w: WIDTH - 16.0,
            h: 54.0,
        };
        let hovered = row.contains(mouse);
        if hovered {
            draw_rectangle(row.x, row.y, row.w, row.h, rgba(34, 38, 46, 255));
        }
        if selected == Some(character.id) {
            draw_rectangle_lines(row.x, row.y, row.w, row.h, 1.5, rgba(255, 210, 120, 200));
        }

        // Аватар: инициалы в круге.
        let avatar = Vec2::new(row.x + 30.0, row.y + 27.0);
        draw_circle(avatar.x, avatar.y, 18.0, rgba(60, 70, 88, 255));
        let initials: String = character
            .name
            .split_whitespace()
            .take(2)
            .filter_map(|part| part.chars().next())
            .collect();
        let width = text_width(assets, &initials, 15.0);
        text(
            assets,
            &initials,
            avatar.x - width * 0.5,
            avatar.y + 5.0,
            15.0,
            rgba(235, 238, 245, 255),
        );

        let mut name = character.name.clone();
        while text_width(assets, &name, 16.0) > 130.0 && name.len() > 4 {
            name.pop();
        }
        text(
            assets,
            &name,
            row.x + 56.0,
            row.y + 22.0,
            16.0,
            rgba(232, 236, 244, 255),
        );
        text(
            assets,
            &format!(
                "{} · допуск {}",
                character.class.name_ru(),
                character.access.numeral()
            ),
            row.x + 56.0,
            row.y + 42.0,
            13.0,
            rgba(150, 158, 174, 255),
        );

        let has_token = tokens
            .iter()
            .any(|token| token.character == Some(character.id));
        if !has_token
            && small_button(
                assets,
                mouse,
                clicked,
                Rect {
                    x: row.x + row.w - 34.0,
                    y: row.y + 14.0,
                    w: 26.0,
                    h: 26.0,
                },
                "→",
            )
        {
            actions.push(RosterAction::Place(character.id));
        }

        if clicked && hovered {
            actions.push(RosterAction::Open(character.id));
        }
        y += 58.0;
    }

    actions
}
