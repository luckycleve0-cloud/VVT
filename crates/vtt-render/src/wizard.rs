//! Мастер создания персонажа: шаг «лист» и шаг «токен».
//!
//! Рисует форму, следит за бюджетами очков и возвращает действия. Серверных
//! вызовов здесь нет — главный цикл превращает [`WizardAction`] в `Intent`.

use std::collections::BTreeMap;

use macroquad::prelude::*;

use vtt_core::geom::Vec2;
use vtt_core::scp::{AccessLevel, Attributes, ScpAttr, ScpClass, ScpSkill};

use crate::draw::{rgba, text, text_width, Assets, Textures};

/// Ширина/высота панели мастера.
const PANEL_WIDTH: f32 = 1000.0;
const PANEL_HEIGHT: f32 = 720.0;

/// Данные, собранные мастером.
#[derive(Debug, Clone)]
pub struct WizardData {
    pub name: String,
    pub class: ScpClass,
    pub access: AccessLevel,
    pub attributes: Attributes,
    pub skills: BTreeMap<ScpSkill, u8>,
    pub image: Option<String>,
    pub frame: vtt_core::entities::TokenFrame,
    pub color: [u8; 3],
}

/// Действие мастера.
#[derive(Debug, Clone)]
pub enum WizardAction {
    Close,
    /// Открыть проводник для выбора картинки.
    PickImage,
    /// Создать персонажа с токеном.
    Create(WizardData),
}

/// Состояние мастера.
#[derive(Debug, Clone)]
pub struct WizardState {
    pub open: bool,
    pub step: u8,
    pub name: String,
    pub class: ScpClass,
    pub access: AccessLevel,
    pub attributes: Attributes,
    pub skills: BTreeMap<ScpSkill, u8>,
    pub image: Option<String>,
    pub frame: vtt_core::entities::TokenFrame,
    pub color: [u8; 3],
    focus_name: bool,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            open: false,
            step: 0,
            name: "Новый агент".to_string(),
            class: ScpClass::Soldier,
            access: AccessLevel::First,
            attributes: Attributes::default(),
            skills: BTreeMap::new(),
            image: None,
            frame: vtt_core::entities::TokenFrame::Ring,
            color: [240, 200, 80],
            focus_name: false,
        }
    }
}

impl WizardState {
    /// Открыть мастер с чистого листа.
    pub fn open(&mut self) {
        *self = Self {
            open: true,
            ..Self::default()
        };
        self.focus_name = true;
    }

    fn attribute_spent(&self) -> u32 {
        self.attributes.total().saturating_sub(10) + self.access.cost()
    }

    fn attribute_budget(&self) -> u32 {
        70 + self.class.extra_attribute_points()
    }

    fn skill_spent(&self) -> u32 {
        self.skills.values().map(|value| u32::from(*value)).sum()
    }

    fn skill_budget(&self) -> u32 {
        84 + self.class.extra_skill_points()
    }

    fn valid(&self) -> bool {
        !self.name.trim().is_empty()
            && self.attribute_spent() <= self.attribute_budget()
            && self.skill_spent() <= self.skill_budget()
    }

    fn data(&self) -> WizardData {
        WizardData {
            name: self.name.trim().to_string(),
            class: self.class,
            access: self.access,
            attributes: self.attributes,
            skills: self.skills.clone(),
            image: self.image.clone(),
            frame: self.frame,
            color: self.color,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    fn contains(self, point: Vec2) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.w
            && point.y >= self.y
            && point.y <= self.y + self.h
    }
}

#[allow(clippy::too_many_arguments)]
fn button(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    rect: Rect,
    label: &str,
    enabled: bool,
    active: bool,
) -> bool {
    let hovered = rect.contains(mouse);
    let bg = if !enabled {
        rgba(30, 32, 36, 255)
    } else if active {
        rgba(70, 96, 140, 255)
    } else if hovered {
        rgba(66, 74, 90, 255)
    } else {
        rgba(40, 46, 58, 255)
    };
    draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        1.0,
        if active {
            rgba(150, 190, 255, 255)
        } else {
            rgba(80, 90, 108, 255)
        },
    );
    let size = 15.0;
    let mut label = label.to_string();
    let mut truncated = false;
    while text_width(assets, &label, size) > rect.w - 10.0 && label.len() > 3 {
        label.pop();
        truncated = true;
    }
    if truncated {
        label.push('…');
    }
    text(
        assets,
        &label,
        rect.x + 6.0,
        rect.y + rect.h * 0.5 + 5.0,
        size,
        if enabled {
            rgba(232, 236, 244, 255)
        } else {
            rgba(120, 126, 136, 255)
        },
    );
    clicked && hovered && enabled
}

/// Нарисовать мастер. Возвращает действия этого кадра.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    assets: &Assets,
    textures: &mut Textures,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    wheel: f32,
    chars: &[char],
    backspace: bool,
    state: &mut WizardState,
) -> Vec<WizardAction> {
    let mut actions = Vec::new();
    if !state.open {
        return actions;
    }

    draw_rectangle(0.0, 0.0, viewport.x, viewport.y, rgba(0, 0, 0, 200));
    let x = ((viewport.x - PANEL_WIDTH) * 0.5).max(0.0);
    let y = ((viewport.y - PANEL_HEIGHT) * 0.5).max(0.0);
    draw_rectangle(x, y, PANEL_WIDTH, PANEL_HEIGHT, rgba(20, 22, 28, 252));
    draw_rectangle_lines(
        x,
        y,
        PANEL_WIDTH,
        PANEL_HEIGHT,
        2.0,
        rgba(90, 100, 120, 255),
    );

    text(
        assets,
        "Создание персонажа",
        x + 24.0,
        y + 38.0,
        26.0,
        rgba(240, 243, 250, 255),
    );

    // Шаги.
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 24.0, y + 56.0, 170.0, 32.0),
        "1. Лист персонажа",
        true,
        state.step == 0,
    ) {
        state.step = 0;
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 202.0, y + 56.0, 150.0, 32.0),
        "2. Токен",
        true,
        state.step == 1,
    ) {
        state.step = 1;
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + PANEL_WIDTH - 58.0, y + 18.0, 34.0, 30.0),
        "×",
        true,
        false,
    ) {
        actions.push(WizardAction::Close);
    }

    if state.step == 0 {
        actions.extend(draw_sheet_step(
            assets, mouse, clicked, wheel, chars, backspace, state, x, y,
        ));
    } else if let Some(action) = draw_token_step(assets, textures, mouse, clicked, state, x, y) {
        actions.push(action);
    }

    // Нижняя панель.
    let footer_y = y + PANEL_HEIGHT - 52.0;
    draw_line(
        x + 20.0,
        footer_y - 8.0,
        x + PANEL_WIDTH - 20.0,
        footer_y - 8.0,
        1.0,
        rgba(70, 78, 94, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 24.0, footer_y, 120.0, 34.0),
        "Отмена",
        true,
        false,
    ) {
        actions.push(WizardAction::Close);
    }

    let spent = format!(
        "характеристики {}/{} · навыки {}/{}",
        state.attribute_spent(),
        state.attribute_budget(),
        state.skill_spent(),
        state.skill_budget()
    );
    text(
        assets,
        &spent,
        x + 170.0,
        footer_y + 22.0,
        16.0,
        if state.valid() {
            rgba(160, 200, 160, 255)
        } else {
            rgba(230, 150, 130, 255)
        },
    );

    let can_create = state.valid();
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + PANEL_WIDTH - 220.0, footer_y, 196.0, 34.0),
        "Создать персонажа",
        can_create,
        false,
    ) {
        actions.push(WizardAction::Create(state.data()));
    }

    actions
}

#[allow(clippy::too_many_arguments)]
fn draw_sheet_step(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    wheel: f32,
    chars: &[char],
    backspace: bool,
    state: &mut WizardState,
    x: f32,
    y: f32,
) -> Vec<WizardAction> {
    let left = x + 24.0;
    let mut ly = y + 110.0;

    // Имя.
    text(
        assets,
        "Имя персонажа",
        left,
        ly,
        15.0,
        rgba(170, 178, 194, 255),
    );
    let name_rect = Rect::new(left, ly + 8.0, 300.0, 34.0);
    draw_rectangle(
        name_rect.x,
        name_rect.y,
        name_rect.w,
        name_rect.h,
        rgba(30, 33, 40, 255),
    );
    draw_rectangle_lines(
        name_rect.x,
        name_rect.y,
        name_rect.w,
        name_rect.h,
        1.0,
        if state.focus_name {
            rgba(150, 190, 255, 255)
        } else {
            rgba(80, 90, 108, 255)
        },
    );
    text(
        assets,
        &state.name,
        name_rect.x + 8.0,
        name_rect.y + 23.0,
        18.0,
        rgba(235, 240, 248, 255),
    );
    if clicked {
        state.focus_name = name_rect.contains(mouse);
    }
    if state.focus_name {
        for ch in chars {
            if !ch.is_control() && state.name.chars().count() < 40 {
                state.name.push(*ch);
            }
        }
        if backspace {
            state.name.pop();
        }
    }
    ly += 64.0;

    // Класс.
    text(assets, "Класс", left, ly, 15.0, rgba(170, 178, 194, 255));
    ly += 10.0;
    for (index, class) in ScpClass::ALL.iter().enumerate() {
        let col = index % 3;
        let row = index / 3;
        let rect = Rect::new(
            left + col as f32 * 108.0,
            ly + row as f32 * 32.0,
            104.0,
            28.0,
        );
        if button(
            assets,
            mouse,
            clicked,
            rect,
            class.name_ru(),
            true,
            state.class == *class,
        ) {
            state.class = *class;
        }
    }
    ly += 3.0 * 32.0 + 30.0;

    // Допуск.
    text(assets, "Допуск", left, ly, 15.0, rgba(170, 178, 194, 255));
    ly += 10.0;
    for (index, access) in AccessLevel::ALL.iter().enumerate() {
        let rect = Rect::new(left + index as f32 * 62.0, ly, 58.0, 30.0);
        let label = access.numeral().to_string();
        if button(
            assets,
            mouse,
            clicked,
            rect,
            &label,
            true,
            state.access == *access,
        ) {
            state.access = *access;
        }
    }
    text(
        assets,
        &format!("стоимость допуска: {} очков", state.access.cost()),
        left,
        ly + 52.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
    let mut ability = format!("способность: {}", state.class.ability_ru());
    while text_width(assets, &ability, 13.0) > 320.0 && ability.len() > 4 {
        ability.pop();
    }
    if !ability.ends_with(')') {
        ability.push('…');
    }
    text(
        assets,
        &ability,
        left,
        ly + 74.0,
        13.0,
        rgba(140, 180, 220, 255),
    );

    // Правая колонка: характеристики.
    let right = x + 360.0;
    let mut ry = y + 110.0;
    text(
        assets,
        &format!(
            "Характеристики (потрачено {} из {})",
            state.attribute_spent(),
            state.attribute_budget()
        ),
        right,
        ry,
        15.0,
        rgba(170, 178, 194, 255),
    );
    ry += 10.0;
    for (index, attr) in ScpAttr::ALL.iter().enumerate() {
        let col = index / 5;
        let row = index % 5;
        let cx = right + col as f32 * 290.0;
        let cy = ry + row as f32 * 26.0;
        let value = state.attributes.get(*attr);
        text(
            assets,
            attr.short_ru(),
            cx,
            cy + 20.0,
            15.0,
            rgba(190, 198, 212, 255),
        );
        text(
            assets,
            &format!("{value:02}"),
            cx + 44.0,
            cy + 20.0,
            16.0,
            rgba(232, 236, 244, 255),
        );
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(cx + 70.0, cy, 24.0, 24.0),
            "−",
            value > 1,
            false,
        ) {
            state.attributes.set(*attr, value.saturating_sub(1).max(1));
        }
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(cx + 98.0, cy, 24.0, 24.0),
            "+",
            value < 10,
            false,
        ) {
            state.attributes.set(*attr, (value + 1).min(10));
        }
    }

    // Навыки: две колонки.
    let skills_top = ry + 5.0 * 26.0 + 10.0;
    text(
        assets,
        &format!(
            "Навыки (потрачено {} из {}, максимум 8)",
            state.skill_spent(),
            state.skill_budget()
        ),
        right,
        skills_top,
        15.0,
        rgba(170, 178, 194, 255),
    );
    let list_top = skills_top + 10.0;
    let list_bottom = y + PANEL_HEIGHT - 70.0;
    let row_height = 18.0;
    let per_column = 21;
    let total = (ScpSkill::ALL.len() / 2) as f32 * row_height;
    let inside = mouse.x >= right && mouse.x <= x + PANEL_WIDTH - 20.0;
    if inside && wheel.abs() > 0.0 {
        // Прокрутка не нужна: 42 навыка в две колонки помещаются.
        let _ = total;
    }
    for (index, skill) in ScpSkill::ALL.iter().enumerate() {
        let column = index / per_column;
        let row = index % per_column;
        let cx = right + column as f32 * 300.0;
        let cy = list_top + row as f32 * row_height;
        if cy > list_bottom {
            continue;
        }
        let base = state.skills.get(skill).copied().unwrap_or(0);
        let mut label = skill.name_ru().to_string();
        while text_width(assets, &label, 13.0) > 160.0 && label.len() > 4 {
            label.pop();
        }
        let cost = if base > 0 {
            format!("{base:02}")
        } else {
            "--".to_string()
        };
        text(
            assets,
            &label,
            cx,
            cy + 16.0,
            13.0,
            rgba(214, 220, 230, 255),
        );
        text(
            assets,
            &cost,
            cx + 170.0,
            cy + 16.0,
            13.0,
            rgba(240, 236, 190, 255),
        );
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(cx + 192.0, cy, 22.0, 16.0),
            "−",
            base > 0,
            false,
        ) {
            if base <= 1 {
                state.skills.remove(skill);
            } else {
                state.skills.insert(*skill, base - 1);
            }
        }
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(cx + 216.0, cy, 22.0, 16.0),
            "+",
            base < 8,
            false,
        ) {
            state.skills.insert(*skill, base + 1);
        }
    }
    Vec::new()
}

fn draw_token_step(
    assets: &Assets,
    textures: &mut Textures,
    mouse: Vec2,
    clicked: bool,
    state: &mut WizardState,
    x: f32,
    y: f32,
) -> Option<WizardAction> {
    let mut action = None;
    let left = x + 40.0;
    let mut ly = y + 116.0;

    text(
        assets,
        "Картинка токена",
        left,
        ly,
        18.0,
        rgba(235, 240, 248, 255),
    );
    ly += 16.0;

    let radius = 120.0;
    let center = Vec2::new(left + radius, ly + radius + 6.0);
    draw_circle(center.x, center.y, radius + 3.0, rgba(40, 44, 54, 255));
    let mut drew_image = false;
    if let Some(path) = &state.image {
        if let Some(texture) = textures.get(path) {
            draw_texture_ex(
                texture,
                center.x - radius,
                center.y - radius,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                    ..Default::default()
                },
            );
            drew_image = true;
        }
    }
    if !drew_image {
        draw_circle(center.x, center.y, radius, rgba(90, 98, 114, 255));
        let hint = "нет картинки";
        let width = text_width(assets, hint, 16.0);
        text(
            assets,
            hint,
            center.x - width * 0.5,
            center.y + 6.0,
            16.0,
            rgba(40, 44, 52, 255),
        );
    }
    crate::draw::draw_token_frame(center, radius, 1.0, state.frame, state.color);

    let mut by = center.y + radius + 22.0;
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(left, by, 180.0, 34.0),
        "Выбрать картинку",
        true,
        false,
    ) {
        action = Some(WizardAction::PickImage);
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(left + 190.0, by, 150.0, 34.0),
        "Без картинки",
        state.image.is_some(),
        false,
    ) {
        state.image = None;
    }
    by += 44.0;

    text(assets, "Рамка", left, by, 15.0, rgba(170, 178, 194, 255));
    by += 8.0;
    for (index, frame) in vtt_core::entities::TokenFrame::ALL.iter().enumerate() {
        let rect = Rect::new(left + index as f32 * 96.0, by, 92.0, 30.0);
        if button(
            assets,
            mouse,
            clicked,
            rect,
            frame.name_ru(),
            true,
            state.frame == *frame,
        ) {
            state.frame = *frame;
        }
    }
    by += 44.0;

    text(
        assets,
        "Цвет рамки",
        left,
        by,
        15.0,
        rgba(170, 178, 194, 255),
    );
    by += 8.0;
    let palette: [[u8; 3]; 8] = [
        [240, 200, 80],
        [230, 90, 90],
        [100, 200, 120],
        [90, 160, 240],
        [180, 140, 240],
        [240, 240, 240],
        [160, 160, 170],
        [230, 130, 60],
    ];
    for (index, color) in palette.iter().enumerate() {
        let rect = Rect::new(left + index as f32 * 44.0, by, 38.0, 30.0);
        let hovered = rect.contains(mouse);
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            Color::from_rgba(color[0], color[1], color[2], 255),
        );
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if hovered { 3.0 } else { 1.0 },
            if state.color == *color {
                rgba(255, 255, 255, 255)
            } else {
                rgba(60, 66, 80, 255)
            },
        );
        if clicked && hovered {
            state.color = *color;
        }
    }

    // Правая колонка: итог.
    let right = x + 420.0;
    let mut ry = y + 116.0;
    text(assets, "Итог", right, ry, 18.0, rgba(235, 240, 248, 255));
    ry += 26.0;
    let lines = [
        format!("Имя: {}", state.name),
        format!("Класс: {}", state.class.name_ru()),
        "Организация: Фонд SCP".to_string(),
        format!(
            "Допуск: {} ({})",
            state.access.numeral(),
            state.access.title_ru()
        ),
        format!(
            "Характеристики: {}/{}",
            state.attribute_spent(),
            state.attribute_budget()
        ),
        format!("Навыки: {}/{}", state.skill_spent(), state.skill_budget()),
        format!(
            "Здоровье: {} физ / {} псих",
            state.attributes.get(ScpAttr::Endurance) as i32 * 15,
            state.attributes.get(ScpAttr::Psyche) as i32 * 10
        ),
    ];
    for line in &lines {
        text(assets, line, right, ry, 16.0, rgba(210, 216, 228, 255));
        ry += 24.0;
    }
    ry += 8.0;
    text(
        assets,
        "Мастер создаст персонажа и поставит",
        right,
        ry,
        14.0,
        rgba(150, 158, 174, 255),
    );
    text(
        assets,
        "его токен в центр карты.",
        right,
        ry + 20.0,
        14.0,
        rgba(150, 158, 174, 255),
    );

    action
}

/// Проверить, что панель занимает точку (чтобы не кликать по карте сквозь неё).
#[must_use]
pub fn contains(viewport: Vec2, point: Vec2) -> bool {
    let x = ((viewport.x - PANEL_WIDTH) * 0.5).max(0.0);
    let y = ((viewport.y - PANEL_HEIGHT) * 0.5).max(0.0);
    Rect::new(x, y, PANEL_WIDTH, PANEL_HEIGHT).contains(point)
}

/// Сохранить выбранную картинку в состояние мастера.
pub fn set_image(state: &mut WizardState, path: Option<String>) {
    state.image = path;
}

impl WizardState {
    /// Запомнить путь к выбранной картинке.
    pub fn set_image_path(&mut self, path: &std::path::Path) {
        self.image = Some(path.to_string_lossy().into_owned());
    }
}
