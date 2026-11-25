//! Панели окна: чат, настройки токена, настройки комнаты и поля.

use macroquad::prelude::*;

use vtt_core::entities::TokenSize;
use vtt_core::geom::Vec2;
use vtt_core::grid::GridKind;
use vtt_core::permissions::Role;
use vtt_proto::{PlayerId, PlayerInfo};

use crate::draw::HudMode;
use crate::draw::{rgba, text, text_width, Assets};
use crate::keys::{Action, Keymap};
use crate::session::LocalSession;

/// Действие панелей.
#[derive(Debug, Clone, PartialEq)]
pub enum UiAction {
    SendChat { text: String, to_gm: bool },
    RollDice { formula: String, to_gm: bool },
    TokenSize(TokenSize),
    TokenOpacity(f32),
    TokenHidden(bool),
    Grid(GridKind),
    CellPx(f32),
    FeetPerCell(f32),
    PickMap,
    ClearMap,
    Role(PlayerId, Role),
    Kick(PlayerId),
    ToggleFog,
    HudMode(HudMode),
    EditKey(Action),
    CloseSettings,
}

/// Состояние чата.
#[derive(Debug, Clone, Default)]
pub struct ChatState {
    pub input: String,
    pub focus: bool,
    pub to_gm: bool,
}

/// Вкладка настроек.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Room,
    Field,
    Interface,
    Keys,
}

/// Состояние окна настроек.
#[derive(Debug, Clone)]
pub struct SettingsState {
    pub open: bool,
    pub tab: SettingsTab,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            open: false,
            tab: SettingsTab::Room,
        }
    }
}

/// Открыто ли модальное окно (блокирует стол).
#[must_use]
pub fn modal_open(settings: &SettingsState) -> bool {
    settings.open
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

fn button(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    rect: Rect,
    label: &str,
    active: bool,
    enabled: bool,
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
    let size = 14.0;
    let mut label = label.to_string();
    while text_width(assets, &label, size) > rect.w - 8.0 && label.len() > 2 {
        label.pop();
    }
    let width = text_width(assets, &label, size);
    text(
        assets,
        &label,
        rect.x + (rect.w - width) * 0.5,
        rect.y + rect.h * 0.5 + 4.0,
        size,
        if enabled {
            rgba(232, 236, 244, 255)
        } else {
            rgba(120, 126, 136, 255)
        },
    );
    clicked && hovered && enabled
}

// ---------------------------------------------------------------------------
// Чат
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Панель токена: размер, прозрачность, скрытие
// ---------------------------------------------------------------------------

/// Панель настроек выбранных токенов.
#[allow(clippy::too_many_arguments)]
pub fn token_panel(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    session: &LocalSession,
    open: bool,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if !open || session.selection.is_empty() {
        return actions;
    }
    let Some(primary) = session.selected_token else {
        return actions;
    };
    let Some(token) = session.scene().token(primary) else {
        return actions;
    };

    let width = 560.0;
    let height = 132.0;
    let x = (viewport.x - width) * 0.5;
    let y = viewport.y - height - 56.0 - 244.0;
    draw_rectangle(x, y, width, height, rgba(14, 16, 20, 252));
    draw_rectangle_lines(x, y, width, height, 1.0, rgba(90, 100, 120, 255));
    text(
        assets,
        &format!("Токенов выбрано: {}", session.selection.len()),
        x + 12.0,
        y + 22.0,
        16.0,
        rgba(230, 234, 242, 255),
    );

    // Размер.
    text(
        assets,
        "Размер",
        x + 12.0,
        y + 48.0,
        14.0,
        rgba(160, 168, 184, 255),
    );
    let sizes = [
        (TokenSize::Tiny, "T"),
        (TokenSize::Small, "S"),
        (TokenSize::Medium, "M"),
        (TokenSize::Large, "L"),
        (TokenSize::Huge, "H"),
        (TokenSize::Gargantuan, "G"),
    ];
    for (index, (size, label)) in sizes.iter().enumerate() {
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(x + 74.0 + index as f32 * 40.0, y + 34.0, 36.0, 26.0),
            label,
            token.size == *size,
            true,
        ) {
            actions.push(UiAction::TokenSize(*size));
        }
    }

    // Прозрачность.
    let opacity = token.opacity.clamp(0.05, 1.0);
    text(
        assets,
        &format!("Прозрачность: {:.0}%", opacity * 100.0),
        x + 12.0,
        y + 80.0,
        14.0,
        rgba(160, 168, 184, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 180.0, y + 66.0, 30.0, 24.0),
        "−",
        false,
        opacity > 0.1,
    ) {
        actions.push(UiAction::TokenOpacity((opacity - 0.1).max(0.05)));
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 214.0, y + 66.0, 30.0, 24.0),
        "+",
        false,
        opacity < 1.0,
    ) {
        actions.push(UiAction::TokenOpacity((opacity + 0.1).min(1.0)));
    }

    // Скрытие от игроков.
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 12.0, y + 96.0, 240.0, 26.0),
        if token.hidden {
            "Показать игрокам"
        } else {
            "Скрыть от игроков"
        },
        token.hidden,
        true,
    ) {
        actions.push(UiAction::TokenHidden(!token.hidden));
    }

    // Рамка.
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 264.0, y + 96.0, 280.0, 26.0),
        &format!(
            "Рамка: {} · {}",
            token.frame.name_ru(),
            if token.image.is_some() {
                "картинка"
            } else {
                "без картинки"
            }
        ),
        false,
        false,
    ) {
        // Рамка меняется в мастере/через данные токена; здесь только подпись.
    }

    actions
}

// ---------------------------------------------------------------------------
// Настройки комнаты и поля
// ---------------------------------------------------------------------------

/// Модальное окно настроек.
#[allow(clippy::too_many_arguments)]
pub fn settings_panel(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    session: &LocalSession,
    state: &mut SettingsState,
    hud: HudMode,
    keys: &Keymap,
    capturing: Option<Action>,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    if !state.open {
        return actions;
    }
    draw_rectangle(0.0, 0.0, viewport.x, viewport.y, rgba(0, 0, 0, 200));

    let width = 820.0_f32.min(viewport.x - 60.0);
    let height = 540.0_f32.min(viewport.y - 60.0);
    let x = (viewport.x - width) * 0.5;
    let y = (viewport.y - height) * 0.5;
    draw_rectangle(x, y, width, height, rgba(20, 22, 28, 252));
    draw_rectangle_lines(x, y, width, height, 2.0, rgba(90, 100, 120, 255));

    text(
        assets,
        "Настройки",
        x + 22.0,
        y + 36.0,
        24.0,
        rgba(240, 243, 250, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + width - 55.0, y + 16.0, 34.0, 30.0),
        "×",
        false,
        true,
    ) {
        actions.push(UiAction::CloseSettings);
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 22.0, y + 52.0, 150.0, 32.0),
        "Комната",
        state.tab == SettingsTab::Room,
        true,
    ) {
        state.tab = SettingsTab::Room;
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 180.0, y + 52.0, 150.0, 32.0),
        "Поле",
        state.tab == SettingsTab::Field,
        true,
    ) {
        state.tab = SettingsTab::Field;
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 338.0, y + 52.0, 160.0, 32.0),
        "Интерфейс",
        state.tab == SettingsTab::Interface,
        true,
    ) {
        state.tab = SettingsTab::Interface;
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 506.0, y + 52.0, 140.0, 32.0),
        "Клавиши",
        state.tab == SettingsTab::Keys,
        true,
    ) {
        state.tab = SettingsTab::Keys;
    }

    match state.tab {
        SettingsTab::Room => room_tab(assets, mouse, clicked, session, x, y, &mut actions),
        SettingsTab::Field => field_tab(assets, mouse, clicked, session, x, y, &mut actions),
        SettingsTab::Interface => interface_tab(assets, mouse, clicked, hud, x, y, &mut actions),
        SettingsTab::Keys => keys_tab(assets, mouse, clicked, keys, capturing, x, y, &mut actions),
    }

    actions
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Gm => "ГМ",
        Role::CoGm => "со-ГМ",
        Role::Player => "игрок",
        Role::Spectator => "зритель",
    }
}

fn room_tab(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    session: &LocalSession,
    x: f32,
    y: f32,
    actions: &mut Vec<UiAction>,
) {
    text(
        assets,
        &format!("Код комнаты: {}", session.server.room_code()),
        x + 22.0,
        y + 120.0,
        20.0,
        rgba(240, 220, 140, 255),
    );
    if let Some(ip) = local_ip() {
        text(
            assets,
            &format!(
                "Адрес для игроков: vtt join {ip}:{} --code {}",
                PORT,
                session.server.room_code()
            ),
            x + 22.0,
            y + 146.0,
            15.0,
            rgba(170, 178, 194, 255),
        );
    }

    draw_line(
        x + 22.0,
        y + 160.0,
        x + 798.0,
        y + 160.0,
        1.0,
        rgba(70, 78, 94, 255),
    );
    text(
        assets,
        &format!("В комнате: {}", session.server.client_count()),
        x + 22.0,
        y + 186.0,
        17.0,
        rgba(226, 232, 242, 255),
    );

    let players: Vec<PlayerInfo> = session.server.players();
    let mut row_y = y + 200.0;
    for player in &players {
        let is_gm = player.role == Role::Gm;
        text(
            assets,
            &format!(
                "#{}{} — {}",
                player.player.0,
                if player.connected {
                    ""
                } else {
                    " (отключён)"
                },
                player.name
            ),
            x + 30.0,
            row_y + 20.0,
            16.0,
            rgba(220, 226, 238, 255),
        );
        text(
            assets,
            role_name(player.role),
            x + 360.0,
            row_y + 20.0,
            15.0,
            if is_gm {
                rgba(240, 200, 90, 255)
            } else {
                rgba(160, 200, 240, 255)
            },
        );
        if !is_gm
            && button(
                assets,
                mouse,
                clicked,
                Rect::new(x + 460.0, row_y + 2.0, 130.0, 26.0),
                "сменить роль",
                false,
                true,
            )
        {
            let next = match player.role {
                Role::Player => Role::CoGm,
                Role::CoGm => Role::Spectator,
                Role::Spectator => Role::Player,
                Role::Gm => Role::Player,
            };
            actions.push(UiAction::Role(player.player, next));
        }
        if !is_gm
            && button(
                assets,
                mouse,
                clicked,
                Rect::new(x + 600.0, row_y + 2.0, 90.0, 26.0),
                "кикнуть",
                false,
                true,
            )
        {
            actions.push(UiAction::Kick(player.player));
        }
        row_y += 34.0;
    }

    text(
        assets,
        "Игроки подключаются командой vtt join <адрес> --code <код>.",
        x + 22.0,
        y + 500.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
}

fn field_tab(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    session: &LocalSession,
    x: f32,
    y: f32,
    actions: &mut Vec<UiAction>,
) {
    let scene = session.scene();
    text(
        assets,
        &format!(
            "Границы: {:.0} × {:.0} px (тяните край карты)",
            scene.width_px, scene.height_px
        ),
        x + 22.0,
        y + 120.0,
        16.0,
        rgba(200, 206, 220, 255),
    );

    text(
        assets,
        "Сетка",
        x + 22.0,
        y + 156.0,
        15.0,
        rgba(160, 168, 184, 255),
    );
    let kinds = [
        (GridKind::Square, "квадрат"),
        (GridKind::HexPointy, "гекс ↑"),
        (GridKind::HexFlat, "гекс →"),
        (GridKind::None, "без сетки"),
    ];
    for (index, (kind, label)) in kinds.iter().enumerate() {
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(x + 100.0 + index as f32 * 120.0, y + 140.0, 112.0, 28.0),
            label,
            scene.grid.kind == *kind,
            true,
        ) {
            actions.push(UiAction::Grid(*kind));
        }
    }

    text(
        assets,
        &format!("Клетка: {:.0} px", scene.grid.cell_px),
        x + 22.0,
        y + 200.0,
        16.0,
        rgba(200, 206, 220, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 200.0, y + 184.0, 30.0, 26.0),
        "−",
        false,
        scene.grid.cell_px > 20.0,
    ) {
        actions.push(UiAction::CellPx(scene.grid.cell_px - 10.0));
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 234.0, y + 184.0, 30.0, 26.0),
        "+",
        false,
        scene.grid.cell_px < 200.0,
    ) {
        actions.push(UiAction::CellPx(scene.grid.cell_px + 10.0));
    }

    text(
        assets,
        &format!("1 клетка = {:.0} фт", scene.grid.feet_per_cell),
        x + 300.0,
        y + 200.0,
        16.0,
        rgba(200, 206, 220, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 500.0, y + 184.0, 30.0, 26.0),
        "−",
        false,
        scene.grid.feet_per_cell > 1.0,
    ) {
        actions.push(UiAction::FeetPerCell(scene.grid.feet_per_cell - 1.0));
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 534.0, y + 184.0, 30.0, 26.0),
        "+",
        false,
        scene.grid.feet_per_cell < 50.0,
    ) {
        actions.push(UiAction::FeetPerCell(scene.grid.feet_per_cell + 1.0));
    }

    text(
        assets,
        "Карта сцены",
        x + 22.0,
        y + 250.0,
        15.0,
        rgba(160, 168, 184, 255),
    );
    let map_label =
        scene
            .background
            .as_ref()
            .map_or("нет картинки".to_string(), |path| {
                std::path::Path::new(path)
                    .file_name()
                    .map_or(path.clone(), |name| name.to_string_lossy().into_owned())
            });
    text(
        assets,
        &map_label,
        x + 140.0,
        y + 250.0,
        14.0,
        rgba(220, 226, 238, 255),
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 22.0, y + 264.0, 170.0, 30.0),
        "Выбрать карту",
        false,
        true,
    ) {
        actions.push(UiAction::PickMap);
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 202.0, y + 264.0, 130.0, 30.0),
        "Убрать карту",
        false,
        scene.background.is_some(),
    ) {
        actions.push(UiAction::ClearMap);
    }

    let fog_label = format!(
        "Туман по клеткам: {} (F)",
        if session.show_fog {
            "вкл"
        } else {
            "выкл"
        }
    );
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + 22.0, y + 316.0, 300.0, 30.0),
        &fog_label,
        session.show_fog,
        true,
    ) {
        actions.push(UiAction::ToggleFog);
    }
    text(
        assets,
        "Туман памяти включайте только для тёмных сцен: он затемняет уже виденное.",
        x + 22.0,
        y + 372.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
    text(
        assets,
        "Права: роли и кик доступны только ГМу; настройки поля тоже.",
        x + 22.0,
        y + 396.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
}

/// Порт комнаты (фиксируется при запуске окна).
pub const PORT: u16 = 7878;

fn local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

/// Ширина правой панели (как chat sidebar в Roll20).
pub const SIDEBAR_WIDTH: f32 = 340.0;

/// Высота карточки броска.
const CARD_HEIGHT: f32 = 92.0;

/// Правая панель: карточки бросков, чат, кубы, ввод.
#[allow(clippy::too_many_arguments)]
pub fn sidebar(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    chars: &[char],
    backspace: bool,
    enter: bool,
    feed: &[crate::session::FeedItem],
    state: &mut ChatState,
) -> Vec<UiAction> {
    use crate::session::FeedItem;

    let mut actions = Vec::new();
    let width = SIDEBAR_WIDTH;
    let x = viewport.x - width;
    let y = crate::toolbar::HEIGHT;
    let height = viewport.y - y;

    draw_rectangle(x, y, width, height, rgba(14, 16, 20, 250));
    draw_line(x, y, x, viewport.y, 2.0, rgba(70, 80, 96, 255));

    text(
        assets,
        "Журнал и чат",
        x + 12.0,
        y + 24.0,
        18.0,
        rgba(238, 242, 250, 255),
    );
    text(
        assets,
        "броски, кубы и события",
        x + 12.0,
        y + 42.0,
        12.0,
        rgba(140, 148, 164, 255),
    );

    // Кубы.
    let dice = ["d4", "d6", "d8", "d10", "d12", "d20", "d%"];
    let mut dx = x + 10.0;
    for die in dice {
        let rect = Rect::new(dx, y + 54.0, 38.0, 26.0);
        if button(assets, mouse, clicked, rect, die, false, true) {
            actions.push(UiAction::RollDice {
                formula: format!("1{}", die),
                to_gm: state.to_gm,
            });
        }
        dx += 40.0;
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + width - 92.0, y + 4.0, 82.0, 22.0),
        if state.to_gm {
            "Тихий"
        } else {
            "Общий"
        },
        state.to_gm,
        true,
    ) {
        state.to_gm = !state.to_gm;
    }

    // Лента: снизу вверх, чтобы новые были видны.
    let input_y = viewport.y - 44.0;
    let area_top = y + 88.0;
    let area_bottom = input_y - 10.0;
    let mut yy = area_bottom;
    for item in feed.iter().rev() {
        let item_height = match item {
            FeedItem::Roll(_) => CARD_HEIGHT,
            FeedItem::Chat { text, .. } => {
                let lines = (text.chars().count() / 40 + 1) as f32;
                lines * 16.0 + 20.0
            }
            FeedItem::Text(_) => 18.0,
        };
        if yy - item_height < area_top {
            break;
        }
        yy -= item_height;
        draw_feed_item(assets, item, x + 6.0, yy, width - 12.0, item_height);
        yy -= 6.0;
    }

    // Ввод.
    let input_rect = Rect::new(x + 10.0, input_y, width - 104.0, 32.0);
    draw_rectangle(
        input_rect.x,
        input_rect.y,
        input_rect.w,
        input_rect.h,
        rgba(28, 31, 38, 255),
    );
    draw_rectangle_lines(
        input_rect.x,
        input_rect.y,
        input_rect.w,
        input_rect.h,
        1.0,
        if state.focus {
            rgba(150, 190, 255, 255)
        } else {
            rgba(70, 78, 94, 255)
        },
    );
    let shown = if state.input.is_empty() {
        "сообщение или /r 1d20…".to_string()
    } else {
        state.input.clone()
    };
    text(
        assets,
        &shown,
        input_rect.x + 8.0,
        input_rect.y + 22.0,
        14.0,
        if state.input.is_empty() {
            rgba(120, 126, 136, 255)
        } else {
            rgba(232, 236, 244, 255)
        },
    );
    if clicked {
        state.focus = input_rect.contains(mouse);
    }
    if state.focus {
        for ch in chars {
            if !ch.is_control() && state.input.chars().count() < 200 {
                state.input.push(*ch);
            }
        }
        if backspace {
            state.input.pop();
        }
        if enter {
            submit(&mut actions, &mut state.input);
        }
    }
    if button(
        assets,
        mouse,
        clicked,
        Rect::new(x + width - 88.0, input_y, 78.0, 32.0),
        "Отправить",
        false,
        !state.input.trim().is_empty(),
    ) {
        submit(&mut actions, &mut state.input);
    }

    actions
}

fn submit(actions: &mut Vec<UiAction>, input: &mut String) {
    let value = input.trim().to_string();
    if value.is_empty() {
        return;
    }
    input.clear();
    for (prefix, to_gm) in [
        ("/gmroll ", true),
        ("/gr ", true),
        ("/roll ", false),
        ("/r ", false),
    ] {
        if let Some(rest) = value.strip_prefix(prefix) {
            actions.push(UiAction::RollDice {
                formula: rest.trim().to_string(),
                to_gm,
            });
            return;
        }
    }
    if let Some(rest) = value.strip_prefix("/w ") {
        actions.push(UiAction::SendChat {
            text: rest.trim().to_string(),
            to_gm: true,
        });
        return;
    }
    actions.push(UiAction::SendChat {
        text: value,
        to_gm: false,
    });
}

fn draw_feed_item(
    assets: &Assets,
    item: &crate::session::FeedItem,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    use crate::session::FeedItem;
    match item {
        FeedItem::Roll(card) => {
            let gm = card.gm_only;
            let outline = if gm {
                rgba(170, 140, 230, 255)
            } else {
                match card.success {
                    Some(true) => rgba(120, 190, 120, 255),
                    Some(false) => rgba(200, 110, 100, 255),
                    None => rgba(120, 90, 70, 255),
                }
            };
            draw_rectangle(x, y, width, height, rgba(52, 24, 26, 252));
            draw_rectangle_lines(x, y, width, height, 1.5, outline);
            text(
                assets,
                &card.actor,
                x + 10.0,
                y + 18.0,
                13.0,
                rgba(220, 226, 238, 255),
            );
            text(
                assets,
                &card.title,
                x + 10.0,
                y + 40.0,
                16.0,
                rgba(245, 238, 226, 255),
            );
            let formula = card.formula.clone();
            let mut formula = formula;
            while crate::draw::text_width(assets, &formula, 11.0) > width - 92.0
                && formula.len() > 4
            {
                formula.pop();
            }
            text(
                assets,
                &formula,
                x + 10.0,
                y + 60.0,
                11.0,
                rgba(190, 170, 160, 255),
            );
            if let Some(note) = &card.note {
                text(
                    assets,
                    note,
                    x + 10.0,
                    y + 78.0,
                    11.0,
                    rgba(160, 150, 150, 255),
                );
            }
            // Ячейка с итогом.
            let box_x = x + width - 82.0;
            draw_rectangle(box_x, y + 22.0, 72.0, 52.0, rgba(30, 14, 16, 255));
            draw_rectangle_lines(box_x, y + 22.0, 72.0, 52.0, 1.0, outline);
            let total = card.total.to_string();
            let tw = crate::draw::text_width(assets, &total, 24.0);
            text(
                assets,
                &total,
                box_x + (72.0 - tw) * 0.5,
                y + 58.0,
                24.0,
                rgba(250, 244, 234, 255),
            );
            if gm {
                text(
                    assets,
                    "тихий",
                    box_x + 18.0,
                    y + 16.0,
                    11.0,
                    rgba(190, 160, 240, 255),
                );
            }
        }
        FeedItem::Chat {
            from,
            text: body,
            to_gm,
        } => {
            let prefix = if *to_gm {
                format!("{from} (шёпот): ")
            } else {
                format!("{from}: ")
            };
            let mut line = format!("{prefix}{body}");
            while crate::draw::text_width(assets, &line, 12.0) > width - 8.0 && line.len() > 5 {
                line.pop();
            }
            text(
                assets,
                &line,
                x + 4.0,
                y + 14.0,
                12.0,
                if *to_gm {
                    rgba(225, 200, 150, 255)
                } else {
                    rgba(214, 220, 232, 255)
                },
            );
        }
        FeedItem::Text(body) => {
            let mut line = body.clone();
            while crate::draw::text_width(assets, &line, 11.0) > width - 8.0 && line.len() > 5 {
                line.pop();
            }
            text(
                assets,
                &line,
                x + 4.0,
                y + 13.0,
                11.0,
                rgba(150, 158, 174, 255),
            );
        }
    }
}
fn interface_tab(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    hud: HudMode,
    x: f32,
    y: f32,
    actions: &mut Vec<UiAction>,
) {
    text(
        assets,
        "Верхняя плашка с информацией",
        x + 22.0,
        y + 130.0,
        17.0,
        rgba(226, 232, 242, 255),
    );
    let variants = [
        (HudMode::Compact, "Компактная"),
        (HudMode::Full, "Полная"),
        (HudMode::Hidden, "Скрыть"),
    ];
    for (index, (mode, label)) in variants.iter().enumerate() {
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(x + 22.0 + index as f32 * 180.0, y + 146.0, 170.0, 34.0),
            label,
            hud == *mode,
            true,
        ) {
            actions.push(UiAction::HudMode(*mode));
        }
    }
    text(
        assets,
        "Компактная показывает название кампании, сцену и число игроков.",
        x + 22.0,
        y + 206.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
    text(
        assets,
        "Полная добавляет инструмент, туман, зум, FPS и подсказки.",
        x + 22.0,
        y + 230.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
    text(
        assets,
        "Клавиша H переключает режимы по кругу.",
        x + 22.0,
        y + 254.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
}

#[allow(clippy::too_many_arguments)]
fn keys_tab(
    assets: &Assets,
    mouse: Vec2,
    clicked: bool,
    keys: &Keymap,
    capturing: Option<Action>,
    x: f32,
    y: f32,
    actions: &mut Vec<UiAction>,
) {
    text(
        assets,
        "Горячие клавиши: нажмите «назначить», затем нужную клавишу",
        x + 22.0,
        y + 112.0,
        15.0,
        rgba(200, 206, 220, 255),
    );
    let row_height = 23.0;
    for (index, action) in Action::ALL.iter().enumerate() {
        let column = index / 10;
        let row = index % 10;
        let cx = x + 22.0 + column as f32 * 385.0;
        let row_y = y + 120.0 + row as f32 * row_height;
        text(
            assets,
            action.name_ru(),
            cx,
            row_y + 15.0,
            13.0,
            rgba(214, 220, 232, 255),
        );
        let is_capture = capturing == Some(*action);
        let key_text = if is_capture {
            "нажмите…".to_string()
        } else {
            keys.label(*action)
        };
        draw_rectangle(
            cx + 200.0,
            row_y,
            100.0,
            20.0,
            if is_capture {
                rgba(58, 82, 120, 255)
            } else {
                rgba(30, 33, 40, 255)
            },
        );
        draw_rectangle_lines(cx + 200.0, row_y, 100.0, 20.0, 1.0, rgba(80, 90, 108, 255));
        text(
            assets,
            &key_text,
            cx + 207.0,
            row_y + 14.0,
            12.0,
            rgba(235, 240, 248, 255),
        );
        if button(
            assets,
            mouse,
            clicked,
            Rect::new(cx + 308.0, row_y, 74.0, 20.0),
            if is_capture {
                "отмена"
            } else {
                "назначить"
            },
            is_capture,
            true,
        ) {
            actions.push(UiAction::EditKey(*action));
        }
    }
}
