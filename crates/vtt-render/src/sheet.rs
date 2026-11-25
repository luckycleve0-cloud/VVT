//! Панель листа персонажа SCP: здоровье, характеристики, навыки, броски.
//!
//! Это «немедленный режим»: панель рисует себя и возвращает список действий,
//! которые главный цикл превращает в `Intent`. Никакого состояния игры здесь
//! нет — только `ScpCharacter` (копия) и состояние прокрутки.

use macroquad::prelude::*;

use vtt_core::geom::Vec2;
use vtt_core::ids::EntityId;
use vtt_core::scp::{ScpAttr, ScpCharacter, ScpSkill, SkillCategory};

use crate::draw::{rgba, text, text_width, Assets};

/// Ширина панели.
pub const PANEL_WIDTH: f32 = 430.0;

/// Действие, выбранное игроком на панели.
#[derive(Debug, Clone, PartialEq)]
pub enum SheetAction {
    /// Изменить характеристику на дельту.
    UpdateAttribute(EntityId, ScpAttr, i32),
    /// Изменить вложенные очки навыка на дельту.
    SetSkill(EntityId, ScpSkill, i32),
    /// Урон по здоровью (`true` — психическое).
    Damage(EntityId, i32, bool),
    /// Лечение.
    Heal(EntityId, i32, bool),
    /// Бросок навыка.
    RollCheck {
        character: EntityId,
        skill: ScpSkill,
        dc: i32,
    },
    /// Бросок характеристики.
    RollAttribute {
        character: EntityId,
        attr: ScpAttr,
        dc: i32,
    },
    /// Бросок психики.
    RollSanity(EntityId),
    /// Начислить опыт.
    GrantXp(EntityId, i32),
    /// Потратить опыт на характеристику.
    LevelUp(EntityId, ScpAttr),
    /// Поставить токен персонажа на сцену.
    SpawnToken(EntityId),
    /// Закрыть панель.
    Close,
}

/// Состояние панели между кадрами.
#[derive(Debug, Clone)]
pub struct SheetState {
    pub scroll: f32,
    pub dc: i32,
}

impl Default for SheetState {
    fn default() -> Self {
        Self {
            scroll: 0.0,
            dc: 15,
        }
    }
}

fn category_color(category: SkillCategory) -> Color {
    match category {
        SkillCategory::Agility => rgba(120, 200, 140, 255),
        SkillCategory::Perception => rgba(110, 180, 240, 255),
        SkillCategory::Endurance => rgba(220, 180, 90, 255),
        SkillCategory::Charisma => rgba(230, 150, 200, 255),
        SkillCategory::Tech => rgba(150, 200, 220, 255),
        SkillCategory::Intellect => rgba(190, 170, 240, 255),
    }
}

/// Нарисовать кнопку; вернуть `true`, если по ней кликнули.
#[allow(clippy::too_many_arguments)]
fn button(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    enabled: bool,
) -> bool {
    let hovered = mouse.x >= x && mouse.x <= x + w && mouse.y >= y && mouse.y <= y + h;
    let bg = if !enabled {
        rgba(28, 30, 34, 255)
    } else if hovered {
        rgba(74, 86, 108, 255)
    } else {
        rgba(40, 46, 58, 255)
    };
    draw_rectangle(x, y, w, h, bg);
    draw_rectangle_lines(
        x,
        y,
        w,
        h,
        1.0,
        if hovered && enabled {
            rgba(150, 165, 190, 255)
        } else {
            rgba(80, 90, 108, 255)
        },
    );
    let size = 15.0;
    let width = text_width(assets, label, size);
    text(
        assets,
        label,
        x + (w - width) * 0.5,
        y + h * 0.5 + 5.0,
        size,
        if enabled {
            rgba(230, 234, 242, 255)
        } else {
            rgba(120, 126, 136, 255)
        },
    );
    clicked && hovered && enabled
}

/// Полоска здоровья с подписью.
#[allow(clippy::too_many_arguments)]
fn health_bar(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    x: f32,
    y: f32,
    label: &str,
    current: i32,
    max: i32,
    mental: bool,
    character: EntityId,
    actions: &mut Vec<SheetAction>,
) -> f32 {
    let ratio = if max > 0 {
        (current as f32 / max as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    text(
        assets,
        &format!("{label} {current}/{max}"),
        x,
        y + 14.0,
        17.0,
        rgba(220, 225, 235, 255),
    );
    let color = if mental {
        rgba(120, 140, 230, 255)
    } else {
        rgba(200, 70, 70, 255)
    };
    draw_rectangle(x, y + 20.0, 250.0, 12.0, rgba(26, 28, 32, 255));
    draw_rectangle(x, y + 20.0, 250.0 * ratio, 12.0, color);

    let mut bx = x + 262.0;
    for (label, delta) in [("-5", -5), ("-1", -1), ("+1", 1), ("+5", 5)] {
        if button(assets, mouse, clicked, bx, y + 2.0, 34.0, 24.0, label, true) {
            if delta < 0 {
                actions.push(SheetAction::Damage(character, -delta, mental));
            } else {
                actions.push(SheetAction::Heal(character, delta, mental));
            }
        }
        bx += 38.0;
    }
    y + 40.0
}

/// Нарисовать панель. Возвращает действия, выбранные в этом кадре.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    wheel: f32,
    character: &ScpCharacter,
    has_token: bool,
    state: &mut SheetState,
    right_offset: f32,
) -> Vec<SheetAction> {
    let mut actions = Vec::new();
    let px = viewport.x - PANEL_WIDTH - right_offset;
    let top = 0.0;
    let bottom = viewport.y;

    draw_rectangle(px, top, PANEL_WIDTH, bottom, rgba(16, 18, 22, 255));
    draw_line(px, 0.0, px, bottom, 2.0, rgba(70, 80, 96, 255));

    let x = px + 14.0;
    let id = character.id;

    // Шапка.
    text(
        assets,
        &character.name,
        x,
        30.0,
        24.0,
        rgba(240, 243, 250, 255),
    );
    text(
        assets,
        &format!(
            "{} · {} · допуск {}",
            character.class.name_ru(),
            character.organization,
            character.access.numeral()
        ),
        x,
        52.0,
        16.0,
        rgba(170, 178, 194, 255),
    );
    text(
        assets,
        &format!(
            "XP {} · иниц. {} · движение {} · 1+СИЛ/5 d6",
            character.xp,
            character.initiative(),
            character.movement_squares()
        ),
        x,
        72.0,
        15.0,
        rgba(150, 158, 174, 255),
    );

    // Здоровье.
    let y = health_bar(
        assets,
        mouse,
        clicked,
        x,
        88.0,
        "ФИЗ",
        character.physical.current,
        character.physical.max,
        false,
        id,
        &mut actions,
    );
    let y = health_bar(
        assets,
        mouse,
        clicked,
        x,
        y + 4.0,
        "ПСИХ",
        character.mental.current,
        character.mental.max,
        true,
        id,
        &mut actions,
    );

    // Характеристики: две колонки по пять.
    text(
        assets,
        "Характеристики (↑ — потратить опыт)",
        x,
        y + 24.0,
        15.0,
        rgba(150, 158, 174, 255),
    );
    let attrs_top = y + 34.0;
    for (index, attr) in ScpAttr::ALL.iter().enumerate() {
        let column = index / 5;
        let row = index % 5;
        let cx = x + column as f32 * 204.0;
        let cy = attrs_top + row as f32 * 28.0;
        let value = character.attributes.get(*attr);
        let cost = character.level_up_cost(*attr);

        text(
            assets,
            attr.short_ru(),
            cx,
            cy + 18.0,
            15.0,
            rgba(190, 198, 212, 255),
        );
        text(
            assets,
            &format!("{value:02}"),
            cx + 40.0,
            cy + 18.0,
            16.0,
            rgba(230, 235, 245, 255),
        );
        if button(
            assets,
            mouse,
            clicked,
            cx + 64.0,
            cy,
            22.0,
            22.0,
            "−",
            value > 1,
        ) {
            actions.push(SheetAction::UpdateAttribute(id, *attr, -1));
        }
        if button(
            assets,
            mouse,
            clicked,
            cx + 88.0,
            cy,
            22.0,
            22.0,
            "+",
            value < 30,
        ) {
            actions.push(SheetAction::UpdateAttribute(id, *attr, 1));
        }
        let can_level = character.xp >= cost;
        if button(
            assets,
            mouse,
            clicked,
            cx + 112.0,
            cy,
            24.0,
            22.0,
            "↑",
            can_level,
        ) {
            actions.push(SheetAction::LevelUp(id, *attr));
        }
        let _ = cost;
    }

    // Сложность проверок.
    let mut dc_y = attrs_top + 5.0 * 28.0 + 6.0;
    text(
        assets,
        &format!("СЛ проверок: {}", state.dc),
        x,
        dc_y + 16.0,
        16.0,
        rgba(220, 210, 150, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        x + 170.0,
        dc_y,
        26.0,
        22.0,
        "−",
        state.dc > 5,
    ) {
        state.dc = (state.dc - 1).max(5);
    }
    if button(
        assets,
        mouse,
        clicked,
        x + 198.0,
        dc_y,
        26.0,
        22.0,
        "+",
        state.dc < 30,
    ) {
        state.dc = (state.dc + 1).min(30);
    }
    text(
        assets,
        "клик по навыку — бросок",
        x + 236.0,
        dc_y + 16.0,
        14.0,
        rgba(140, 148, 164, 255),
    );
    dc_y += 30.0;

    // Навыки: прокручиваемый список.
    draw_rectangle(
        x,
        dc_y,
        PANEL_WIDTH - 28.0,
        bottom - dc_y - 84.0,
        rgba(20, 22, 27, 255),
    );
    let list_top = dc_y + 4.0;
    let list_bottom = bottom - 84.0;
    let inside = mouse.x >= px + 8.0
        && mouse.x <= px + PANEL_WIDTH - 8.0
        && mouse.y >= list_top
        && mouse.y <= list_bottom;
    let row_height = 24.0;
    let total = row_height * ScpSkill::ALL.len() as f32;
    let max_scroll = (total - (list_bottom - list_top)).max(0.0);
    if inside && wheel.abs() > 0.0 {
        state.scroll = (state.scroll + wheel * 42.0).clamp(0.0, max_scroll);
    }
    state.scroll = state.scroll.clamp(0.0, max_scroll);

    let mut sy = list_top - state.scroll;
    for skill in ScpSkill::ALL {
        let row_hit = sy >= list_top - row_height && sy <= list_bottom;
        if row_hit {
            let value = character.skill_value(skill);
            let base = character.skills.get(&skill).copied().unwrap_or(0);
            let hovered_row = inside
                && mouse.y >= sy.max(list_top)
                && mouse.y < (sy + row_height).min(list_bottom);
            if hovered_row {
                draw_rectangle(x, sy, PANEL_WIDTH - 28.0, row_height, rgba(34, 38, 46, 255));
            }
            draw_circle(
                x + 6.0,
                sy + row_height * 0.5,
                3.5,
                category_color(skill.category()),
            );
            let name_limit = 150.0;
            let mut label = skill.name_ru().to_string();
            while text_width(assets, &label, 15.0) > name_limit && label.len() > 4 {
                label.pop();
            }
            text(
                assets,
                &label,
                x + 16.0,
                sy + 17.0,
                15.0,
                rgba(220, 226, 236, 255),
            );
            text(
                assets,
                &format!("{value:02}"),
                x + 186.0,
                sy + 17.0,
                15.0,
                rgba(240, 240, 200, 255),
            );
            if button(
                assets,
                mouse,
                clicked,
                x + 210.0,
                sy + 1.0,
                22.0,
                22.0,
                "−",
                base > 0,
            ) {
                actions.push(SheetAction::SetSkill(id, skill, -1));
            }
            if button(
                assets,
                mouse,
                clicked,
                x + 234.0,
                sy + 1.0,
                22.0,
                22.0,
                "+",
                base < 20,
            ) {
                actions.push(SheetAction::SetSkill(id, skill, 1));
            }
            if button(
                assets,
                mouse,
                clicked,
                x + 262.0,
                sy + 1.0,
                60.0,
                22.0,
                "d20",
                true,
            ) {
                actions.push(SheetAction::RollCheck {
                    character: id,
                    skill,
                    dc: state.dc,
                });
            }
        }
        sy += row_height;
    }

    // Нижние кнопки.
    let by = bottom - 62.0;
    if button(
        assets,
        mouse,
        clicked,
        x,
        by,
        120.0,
        30.0,
        "Психоз d10",
        true,
    ) {
        actions.push(SheetAction::RollSanity(id));
    }
    if button(
        assets,
        mouse,
        clicked,
        x + 126.0,
        by,
        96.0,
        30.0,
        "+50 XP",
        true,
    ) {
        actions.push(SheetAction::GrantXp(id, 50));
    }
    if button(
        assets,
        mouse,
        clicked,
        x + 228.0,
        by,
        120.0,
        30.0,
        if has_token {
            "Токен на карте"
        } else {
            "Поставить токен"
        },
        !has_token,
    ) {
        actions.push(SheetAction::SpawnToken(id));
    }
    if button(
        assets,
        mouse,
        clicked,
        x + PANEL_WIDTH - 48.0,
        by,
        30.0,
        30.0,
        "×",
        true,
    ) {
        actions.push(SheetAction::Close);
    }
    if !has_token {
        text(
            assets,
            "ЛКМ по токену — выбрать; Tab — скрыть лист",
            x,
            bottom - 14.0,
            13.0,
            rgba(130, 138, 154, 255),
        );
    }

    actions
}

/// Панель «нет персонажа»: подсказка, как создать.
pub fn draw_empty(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    right_offset: f32,
) -> Option<SheetAction> {
    let px = viewport.x - PANEL_WIDTH - right_offset;
    draw_rectangle(px, 0.0, PANEL_WIDTH, viewport.y, rgba(16, 18, 22, 255));
    draw_line(px, 0.0, px, viewport.y, 2.0, rgba(70, 80, 96, 255));
    text(
        assets,
        "Лист персонажа",
        px + 14.0,
        30.0,
        22.0,
        rgba(230, 234, 242, 255),
    );
    text(
        assets,
        "Токен без листа. Выберите токен персонажа",
        px + 14.0,
        60.0,
        15.0,
        rgba(160, 168, 184, 255),
    );
    text(
        assets,
        "(у него был создан персонаж) — откроется лист.",
        px + 14.0,
        80.0,
        15.0,
        rgba(160, 168, 184, 255),
    );
    let mut y = 110.0;
    if button(
        assets,
        mouse,
        clicked,
        px + 14.0,
        y,
        200.0,
        30.0,
        "Новый агент (Солдат)",
        true,
    ) {
        return Some(SheetAction::Close);
    }
    y += 40.0;
    text(
        assets,
        "В демо-кампании персонажи уже есть.",
        px + 14.0,
        y + 10.0,
        14.0,
        rgba(130, 138, 154, 255),
    );
    None
}
