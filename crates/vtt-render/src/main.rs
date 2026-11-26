//! `vtt-play` — окно: карта, токены, лист персонажа, мастер, чат и настройки.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use macroquad::input::get_char_pressed;
use macroquad::prelude::*;

use vtt_core::entities::Token;
use vtt_core::geom::Vec2;
use vtt_core::ids::EntityId;
use vtt_net::TcpHub;
use vtt_proto::{CharacterPatch, ClientMsg, Intent, ServerMsg};
use vtt_render::camera::Camera;
use vtt_render::draw::{self, Assets, HudMode, Textures};
use vtt_render::float::FloatWindow;
use vtt_render::interact::{Action, Interaction, Pointer, Tool};
use vtt_render::keys::{self, Action as KeyAction, Keymap};
use vtt_render::panels::{self, ChatState, SettingsState, UiAction};
use vtt_render::picker::{self, PickerState};
use vtt_render::roster::{self, RosterAction};
use vtt_render::session::LocalSession;
use vtt_render::sheet::{self, SheetAction, SheetState};
use vtt_render::toolbar::{self, BarAction, BarState};
use vtt_render::wizard::{self, WizardAction, WizardState};
use vtt_server::Outgoing;

/// Логический размер окна: всё рисуется сюда, затем масштабируется (letterbox).
const VW: f32 = 1440.0;
const VH: f32 = 860.0;

#[derive(Debug)]
struct Args {
    dir: PathBuf,
    map: Option<PathBuf>,
    name: String,
    port: u16,
    screenshot: Option<PathBuf>,
    frames: u32,
    panel: Option<String>,
    demo_art: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Right,
    Bottom,
}

#[derive(Debug, Clone)]
struct GroupDrag {
    start: Vec2,
    origins: Vec<(EntityId, Vec2)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerTarget {
    TokenImage,
    Map,
}

fn window_conf() -> Conf {
    Conf {
        window_title: "VTT — комната ГМа".to_owned(),
        window_width: 1440,
        window_height: 860,
        high_dpi: true,
        sample_count: 4,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("ошибка: {err}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let args = parse_args();
    let mut session = LocalSession::open(&args.dir, args.map.clone(), &args.name)?;

    // Комната: сервер уже есть, добавляем TCP-транспорт для игроков.
    let mut hub: Option<Arc<TcpHub>> = None;
    let mut incoming: Option<mpsc::Receiver<(u64, ClientMsg)>> = None;
    match TcpHub::bind(("0.0.0.0", args.port)) {
        Ok(bound) => {
            let bound = Arc::new(bound);
            let (tx, rx) = mpsc::channel();
            let hub_thread = Arc::clone(&bound);
            thread::spawn(move || loop {
                for item in hub_thread.poll() {
                    if tx.send(item).is_err() {
                        return;
                    }
                }
                thread::sleep(Duration::from_millis(8));
            });
            println!(
                "Комната создана: код {}, порт {}. Игрокам: vtt join <адрес>:{} --code {}",
                session.server.room_code(),
                args.port,
                args.port,
                session.server.room_code()
            );
            hub = Some(bound);
            incoming = Some(rx);
        }
        Err(err) => eprintln!(
            "порт {} занят ({err}); комната только для одного ГМа",
            args.port
        ),
    }

    let mut assets = load_assets(args.map.as_ref()).await;
    let mut textures = Textures::default();
    let mut map_loaded: Option<String> = None;

    let viewport = Vec2::new(VW, VH);
    let mut camera = Camera::default();
    camera.fit(
        Vec2::new(session.scene().width_px, session.scene().height_px),
        viewport,
        40.0,
    );

    let mut interaction = Interaction::new();
    let mut sheet_state = SheetState::default();
    let mut wizard = WizardState::default();
    let mut picker = PickerState::default();
    let mut chat_state = ChatState::default();
    let mut settings = SettingsState::default();
    let mut wizard_win: Option<FloatWindow> = None;
    let mut settings_win: Option<FloatWindow> = None;
    let mut picker_win: Option<FloatWindow> = None;
    let mut native_dialog: Option<mpsc::Receiver<Option<PathBuf>>> = None;
    let mut token_win: Option<FloatWindow> = None;
    let mut picker_target = PickerTarget::TokenImage;
    let mut token_panel_open = false;
    let mut show_chat = true;
    let mut show_grid = true;
    let mut hud_mode = HudMode::Compact;
    let mut keymap = Keymap::load(&args.dir.join("keys.json"));
    let mut capturing: Option<KeyAction> = None;
    let mut show_roster = true;
    let mut group_drag: Option<GroupDrag> = None;
    let mut rubber: Option<Vec2> = None;
    let mut resize_edge: Option<Edge> = None;
    let mut last_mouse = Vec2::ZERO;
    let mut last_save = get_time();
    let mut frame_count: u32 = 0;

    // Демонстрационные картинки: карта и токены из папки.
    if let Some(art) = &args.demo_art {
        let map = art.join("map.jpg");
        if map.exists() {
            session.set_background(Some(map.to_string_lossy().into_owned()));
        }
        let tokens: Vec<EntityId> = session.scene().tokens.iter().map(|t| t.id).collect();
        for (index, token) in tokens.iter().enumerate() {
            let file = art.join(format!("token{}.jpg", index + 1));
            if file.exists() {
                session.set_token_image(*token, Some(file.to_string_lossy().into_owned()));
                session.apply(Intent::SetTokenFrame {
                    token: *token,
                    frame: vtt_core::entities::TokenFrame::Ring,
                    color: [240, 200, 80],
                });
                session.apply(Intent::SetTokenHidden {
                    token: *token,
                    hidden: false,
                });
                let spots = [
                    Vec2::new(500.0, 700.0),
                    Vec2::new(900.0, 900.0),
                    Vec2::new(1250.0, 1150.0),
                ];
                let spot = spots[index.min(spots.len() - 1)];
                session.apply(Intent::MoveToken {
                    token: *token,
                    to: session.scene().grid.snap(spot),
                });
            }
        }
    }

    // Демо-броски для проверки карточек в ленте.
    if args.demo_art.is_some() {
        session.apply(Intent::Roll {
            expr: "1d20".to_string(),
            mode: vtt_core::dice::RollMode::Normal,
            to_gm_only: false,
        });
        session.apply(Intent::Roll {
            expr: "2d6+3".to_string(),
            mode: vtt_core::dice::RollMode::Normal,
            to_gm_only: true,
        });
        if let Some(character) = session.server.campaign.characters.first().map(|c| c.id) {
            session.apply(Intent::RollCharacterCheck {
                character,
                skill: Some(vtt_core::scp::ScpSkill::Pistols),
                attribute: None,
                dc: 15,
            });
        }
    }
    // Режим проверки визуала: открыть нужную панель до скриншота.
    if let Some(panel) = args.panel.as_deref() {
        match panel {
            "wizard-token" => {
                wizard.open();
                wizard.step = 1;
            }
            "picker" => picker.show(),
            "wizard" => wizard.open(),
            "settings" => settings.open = true,
            "settings-keys" => {
                settings.open = true;
                settings.tab = vtt_render::panels::SettingsTab::Keys;
            }
            "settings-interface" => {
                settings.open = true;
                settings.tab = vtt_render::panels::SettingsTab::Interface;
            }
            "settings-field" => {
                settings.open = true;
                settings.tab = vtt_render::panels::SettingsTab::Field;
            }
            "token" => {
                token_panel_open = true;
                if let Some(id) = first_token_with_character(&session) {
                    session.select_only(id);
                }
            }
            "sheet" => {
                session.show_sheet = true;
                if session.selected_character.is_none() {
                    if let Some(character) = session.server.campaign.characters.first() {
                        session.selected_character = Some(character.id);
                    }
                }
            }
            "chat" => show_chat = true,
            _ => {}
        }
    }

    // Виртуальный экран: рисуем в render target и масштабируем в окно.
    let target = render_target(VW as u32, VH as u32);
    target.texture.set_filter(FilterMode::Linear);
    let mut ui_cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, VW, VH));
    ui_cam.render_target = Some(target.clone());

    draw::print_controls();

    loop {
        let scale = f32::min(screen_width() / VW, screen_height() / VH);
        let offset = Vec2::new(
            (screen_width() - VW * scale) * 0.5,
            (screen_height() - VH * scale) * 0.5,
        );
        let mouse = Vec2::new(
            (mouse_position().0 - offset.x) / scale,
            (mouse_position().1 - offset.y) / scale,
        );

        let pointer = Pointer {
            world: camera.screen_to_world(mouse, viewport),
            left_down: is_mouse_button_down(MouseButton::Left),
            right_down: is_mouse_button_down(MouseButton::Right),
        };
        let now = get_time();
        let clicked = is_mouse_button_pressed(MouseButton::Left);
        let released = is_mouse_button_released(MouseButton::Left);
        let (_, wheel) = mouse_wheel();
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);

        let mut chars = Vec::new();
        while let Some(ch) = get_char_pressed() {
            chars.push(ch);
        }
        if let Some(target) = capturing {
            if let Some(key) = keys::first_pressed() {
                if key == KeyCode::Escape {
                    capturing = None;
                } else {
                    keymap.set(target, key);
                    let _ = keymap.save(&args.dir.join("keys.json"));
                    capturing = None;
                }
            }
        }
        if let Some(rx) = &native_dialog {
            if let Ok(choice) = rx.try_recv() {
                native_dialog = None;
                if let Some(path) = choice {
                    let path_text = path.to_string_lossy().into_owned();
                    match picker_target {
                        PickerTarget::TokenImage => wizard.set_image_path(&path),
                        PickerTarget::Map => session.set_background(Some(path_text)),
                    }
                }
            }
        }
        let backspace = is_key_pressed(KeyCode::Backspace);
        let enter = is_key_pressed(KeyCode::Enter);

        // Сеть: сообщения игроков.
        if let (Some(hub), Some(rx)) = (hub.as_ref(), incoming.as_ref()) {
            while let Ok((client, msg)) = rx.try_recv() {
                let outgoing = session.handle_remote(client, msg);
                dispatch(hub, outgoing);
            }
        }

        // Карта: перезагрузить текстуру при смене фона.
        let desired_map = session.scene().background.clone();
        if desired_map != map_loaded {
            assets.map = desired_map.as_deref().and_then(draw::load_texture);
            map_loaded = desired_map;
        }

        let sheet_open = session.show_sheet && session.selected_character.is_some();
        let sheet_hit = sheet_open && mouse.x > VW - sheet::PANEL_WIDTH;
        let roster_hit = show_roster && mouse.x < roster::WIDTH;
        let sidebar_rect =
            show_chat && mouse.x > VW - panels::SIDEBAR_WIDTH && mouse.y > toolbar::HEIGHT;
        let overlay =
            wizard.open || picker.open || settings.open || sheet_hit || roster_hit || sidebar_rect;

        if !overlay && wheel.abs() > 0.0 {
            let current = (camera.zoom * 100.0).round() as i32;
            let step = if wheel > 0.0 { 10 } else { -10 };
            let target_zoom = (current + step).clamp(10, 400);
            if target_zoom != current {
                let factor = (target_zoom as f32 / 100.0) / camera.zoom;
                camera.zoom_at(mouse, viewport, factor);
            }
        }

        let panning = pointer.right_down || (is_key_down(KeyCode::Space) && pointer.left_down);
        if panning && !overlay {
            camera.pan(mouse - last_mouse);
        }
        last_mouse = mouse;

        // Клавиши.
        if keymap.pressed(KeyAction::Select) {
            interaction.cancel();
            interaction.tool = Tool::Select;
        }
        if keymap.pressed(KeyAction::Ruler) {
            interaction.cancel();
            interaction.tool = Tool::Measure;
        }
        if keymap.pressed(KeyAction::Fog) {
            interaction.cancel();
            interaction.tool = Tool::Fog;
        }
        if keymap.pressed(KeyAction::Ping) {
            interaction.cancel();
            interaction.tool = Tool::Ping;
        }
        if keymap.pressed(KeyAction::Wall) {
            interaction.cancel();
            interaction.tool = Tool::Wall;
        }
        if keymap.pressed(KeyAction::Door) {
            interaction.cancel();
            interaction.tool = Tool::Door;
        }
        if is_key_pressed(KeyCode::H) {
            hud_mode = match hud_mode {
                HudMode::Compact => HudMode::Full,
                HudMode::Full => HudMode::Hidden,
                HudMode::Hidden => HudMode::Compact,
            };
        }
        if keymap.pressed(KeyAction::Grid) {
            show_grid = !show_grid;
        }
        if keymap.pressed(KeyAction::FogToggle) {
            session.show_fog = !session.show_fog;
        }
        if keymap.pressed(KeyAction::Sheet) {
            session.show_sheet = !session.show_sheet;
        }
        if keymap.pressed(KeyAction::Roster) {
            show_roster = !show_roster;
        }
        if keymap.pressed(KeyAction::Chat) {
            show_chat = !show_chat;
        }
        if keymap.pressed(KeyAction::TokenPanel) {
            token_panel_open = !token_panel_open;
        }
        if keymap.pressed(KeyAction::Settings) {
            settings.open = !settings.open;
        }
        if keymap.pressed(KeyAction::Wizard) {
            wizard.open();
        }
        if keymap.pressed(KeyAction::ClearFog) {
            session.apply(Intent::ClearFog);
        }
        if keymap.pressed(KeyAction::Save) {
            if let Ok(path) = session.save() {
                println!("сохранено: {}", path.display());
            }
        }
        if keymap.pressed(KeyAction::Monster) && !session.spawn_monster("Гоблин", pointer.world)
        {
            println!("в компендиуме нет «Гоблина»");
        }
        if keymap.pressed(KeyAction::NewToken) {
            session.create_token("Новый токен", pointer.world);
        }
        if is_key_pressed(KeyCode::Escape) {
            interaction.cancel();
            group_drag = None;
            rubber = None;
            resize_edge = None;
            settings.open = false;
            if wizard.open {
                wizard.open = false;
            }
            chat_state.focus = false;
        }
        if keymap.pressed(KeyAction::Delete) && !overlay {
            let tokens = session.selected_ids();
            for token in tokens {
                session.delete_token(token);
            }
            session.clear_selection();
        }
        if keymap.pressed(KeyAction::Quit) && is_key_down(KeyCode::LeftControl) {
            break;
        }

        // Мышь по столу.
        if !overlay && !panning {
            if clicked {
                if let Some(edge) = edge_at(session.scene(), pointer.world, camera.zoom) {
                    resize_edge = Some(edge);
                } else if interaction.tool == Tool::Select {
                    match session.scene().token_at(pointer.world).map(|t| t.id) {
                        Some(token) if ctrl => session.toggle_selected(token),
                        Some(token) => {
                            if !session.is_selected(token) {
                                session.select_only(token);
                            }
                            let origins = selected_positions(&session);
                            group_drag = Some(GroupDrag {
                                start: pointer.world,
                                origins,
                            });
                        }
                        None => {
                            if !ctrl {
                                session.clear_selection();
                            }
                            rubber = Some(pointer.world);
                        }
                    }
                } else if let Some(action) = interaction.press(pointer, session.scene()) {
                    apply_action(&mut session, action, now);
                }
            }
            if pointer.left_down {
                interaction.move_pointer(pointer);
            }
            if released {
                if let Some(edge) = resize_edge.take() {
                    let mut width = session.scene().width_px;
                    let mut height = session.scene().height_px;
                    match edge {
                        Edge::Right => width = pointer.world.x.max(350.0),
                        Edge::Bottom => height = pointer.world.y.max(350.0),
                    }
                    session.set_scene_bounds(width, height);
                } else if let Some(drag) = group_drag.take() {
                    let delta = pointer.world - drag.start;
                    let grid = session.scene().grid;
                    for (token, origin) in drag.origins {
                        session.apply(Intent::MoveToken {
                            token,
                            to: grid.snap(origin + delta),
                        });
                    }
                } else if let Some(start) = rubber.take() {
                    session.select_in_rect(start, pointer.world, ctrl);
                } else {
                    let grid = session.scene().grid;
                    if let Some(action) = interaction.release(pointer, &grid) {
                        apply_action(&mut session, action, now);
                    }
                }
            }
        }

        session.tick(now);

        if session.dirty && now - last_save > 30.0 {
            let _ = session.save();
            last_save = now;
        }

        let previews: Vec<(EntityId, Vec2)> = match &group_drag {
            Some(drag) => {
                let delta = pointer.world - drag.start;
                drag.origins
                    .iter()
                    .map(|(id, origin)| (*id, *origin + delta))
                    .collect()
            }
            None => Vec::new(),
        };
        let rubber_rect = rubber.map(|start| (start, pointer.world));
        let wall_preview = interaction.wall_line(pointer);

        // ---- Отрисовка в виртуальный экран ----
        let hud_x = if show_roster {
            roster::WIDTH + 12.0
        } else {
            12.0
        };
        let bar_visible = !(sheet_open || settings.open || wizard.open || picker.open);
        let hud_y = if bar_visible { toolbar::HEIGHT } else { 0.0 };
        set_camera(&ui_cam);
        clear_background(rgba8(16, 18, 22));

        draw::frame(
            &camera,
            viewport,
            &session,
            &assets,
            &mut textures,
            &interaction,
            &previews,
            wall_preview,
            rubber_rect,
            show_grid,
            hud_x,
            hud_y,
            hud_mode,
        );

        if bar_visible {
            let state = BarState {
                tool: interaction.tool,
                grid: show_grid,
                roster: show_roster,
                sheet: session.show_sheet,
                chat: show_chat,
                token: token_panel_open,
            };
            if let Some(action) = toolbar::draw(&assets, viewport, mouse, clicked, state) {
                match action {
                    BarAction::Tool(tool) => {
                        interaction.cancel();
                        interaction.tool = tool;
                    }
                    BarAction::Grid => show_grid = !show_grid,
                    BarAction::Roster => show_roster = !show_roster,
                    BarAction::NewCharacter => wizard.open(),
                    BarAction::Sheet => {
                        session.show_sheet = !session.show_sheet;
                        if session.show_sheet && session.selected_character.is_none() {
                            let first = session
                                .server
                                .campaign
                                .characters
                                .first()
                                .map(|character| character.id);
                            if let Some(id) = first {
                                session.selected_character = Some(id);
                            }
                        }
                    }
                    BarAction::TokenPanel => token_panel_open = !token_panel_open,
                    BarAction::Chat => show_chat = !show_chat,
                    BarAction::Settings => settings.open = !settings.open,
                }
            }
        }

        if show_roster {
            let characters = session.server.campaign.characters.clone();
            let tokens: Vec<Token> = session.scene().tokens.clone();
            let actions = roster::draw(
                &assets,
                viewport,
                mouse,
                clicked,
                &characters,
                &tokens,
                session.selected_character,
                if bar_visible { toolbar::HEIGHT } else { 0.0 },
            );
            for action in actions {
                match action {
                    RosterAction::Open(id) => {
                        session.selected_character = Some(id);
                        session.show_sheet = true;
                    }
                    RosterAction::New => wizard.open(),
                    RosterAction::Close => show_roster = false,
                    RosterAction::Place(id) => {
                        session.spawn_character_token(id, session.scene_center());
                    }
                }
            }
        }

        if sheet_open {
            if let Some(character) = session.selected_sheet() {
                let has_token = session.selected_sheet_has_token();
                let actions = sheet::draw(
                    &assets,
                    viewport,
                    mouse,
                    clicked,
                    wheel,
                    &character,
                    has_token,
                    &mut sheet_state,
                    if show_chat {
                        panels::SIDEBAR_WIDTH
                    } else {
                        0.0
                    },
                );
                for action in actions {
                    let center = session.scene_center();
                    apply_sheet_action(&mut session, action, center);
                }
            }
        }

        if token_panel_open {
            let win = token_win.get_or_insert_with(|| {
                FloatWindow::bottom_right(viewport, Vec2::new(560.0, 132.0), "Токен")
            });
            win.update(
                mouse,
                clicked,
                is_mouse_button_down(MouseButton::Left),
                viewport,
            );
            let content_click = clicked && win.is_content_point(mouse);
            let content = win.content_size();
            let (content_mouse, _) = win.begin_content(&assets, mouse);
            let actions = panels::token_panel(
                &assets,
                content,
                content_mouse,
                content_click,
                &session,
                true,
            );
            win.end_content(&ui_cam, mouse);
            for action in actions {
                apply_ui_action(&mut session, &mut settings, action, None, &mut hud_mode);
            }
        }

        if clicked && !sidebar_rect {
            chat_state.focus = false;
        }
        if show_chat {
            let actions = panels::sidebar(
                &assets,
                viewport,
                mouse,
                clicked,
                &chars,
                backspace && chat_state.focus,
                enter,
                &session.feed,
                &mut chat_state,
            );
            for action in actions {
                apply_ui_action(
                    &mut session,
                    &mut settings,
                    action,
                    hub.as_deref(),
                    &mut hud_mode,
                );
            }
        }

        if settings.open {
            let win = settings_win.get_or_insert_with(|| {
                FloatWindow::centered(viewport, Vec2::new(820.0, 540.0), "Настройки")
            });
            win.update(
                mouse,
                clicked,
                is_mouse_button_down(MouseButton::Left),
                viewport,
            );
            let content_click = clicked && win.is_content_point(mouse);
            let content = win.content_size();
            let (content_mouse, _) = win.begin_content(&assets, mouse);
            let actions = panels::settings_panel(
                &assets,
                content,
                content_mouse,
                content_click,
                &session,
                &mut settings,
                hud_mode,
                &keymap,
                capturing,
            );
            win.end_content(&ui_cam, mouse);
            for action in actions {
                if let UiAction::EditKey(target) = action {
                    capturing = if capturing == Some(target) {
                        None
                    } else {
                        Some(target)
                    };
                    continue;
                }
                if action == UiAction::PickMap {
                    picker_target = PickerTarget::Map;
                    native_dialog = Some(pick_image_file());
                    continue;
                }
                apply_ui_action(
                    &mut session,
                    &mut settings,
                    action,
                    hub.as_deref(),
                    &mut hud_mode,
                );
            }
        }

        if wizard.open {
            let win = wizard_win.get_or_insert_with(|| {
                FloatWindow::centered(viewport, Vec2::new(1000.0, 720.0), "Создание персонажа")
            });
            win.update(
                mouse,
                clicked,
                is_mouse_button_down(MouseButton::Left),
                viewport,
            );
            let content_click = clicked && win.is_content_point(mouse);
            let content = win.content_size();
            let (content_mouse, _) = win.begin_content(&assets, mouse);
            let actions = wizard::draw(
                &assets,
                &mut textures,
                content,
                content_mouse,
                content_click,
                is_mouse_button_down(MouseButton::Left),
                wheel,
                &chars,
                backspace,
                &mut wizard,
            );
            win.end_content(&ui_cam, mouse);
            for action in actions {
                match action {
                    WizardAction::Close => wizard.open = false,
                    WizardAction::PickImage => {
                        picker_target = PickerTarget::TokenImage;
                        native_dialog = Some(pick_image_file());
                    }
                    WizardAction::Create(mut data) => {
                        if let Some(source) = data.image.clone() {
                            let tokens_dir = args.dir.join("tokens");
                            let _ = std::fs::create_dir_all(&tokens_dir);
                            let stamp = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|value| value.as_millis())
                                .unwrap_or(0);
                            let out = tokens_dir.join(format!("token-{stamp}.png"));
                            if draw::save_cropped_token(
                                &source,
                                data.crop_zoom,
                                data.crop_x,
                                data.crop_y,
                                &out,
                            ) {
                                data.image = Some(out.to_string_lossy().into_owned());
                            }
                        }
                        session.create_character_full(
                            &data.name,
                            data.class,
                            data.access,
                            data.attributes,
                            data.skills,
                            data.image,
                            data.frame,
                            data.color,
                            session.scene_center(),
                        );
                        wizard.open = false;
                    }
                }
            }
        }

        if picker.open {
            let win = picker_win.get_or_insert_with(|| {
                FloatWindow::centered(viewport, Vec2::new(720.0, 560.0), "Выбор файла")
            });
            win.update(
                mouse,
                clicked,
                is_mouse_button_down(MouseButton::Left),
                viewport,
            );
            let content_click = clicked && win.is_content_point(mouse);
            let content = win.content_size();
            let (content_mouse, _) = win.begin_content(&assets, mouse);
            let selection = picker::draw(
                &assets,
                content,
                content_mouse,
                content_click,
                wheel,
                &mut picker,
            );
            win.end_content(&ui_cam, mouse);
            if let Some(path) = selection {
                let path_text = path.to_string_lossy().into_owned();
                match picker_target {
                    PickerTarget::TokenImage => wizard.set_image_path(&path),
                    PickerTarget::Map => session.set_background(Some(path_text)),
                }
            }
        }
        // ---- Перенос виртуального экрана в окно ----
        set_default_camera();
        clear_background(BLACK);
        draw_texture_ex(
            &target.texture,
            offset.x,
            offset.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(VW * scale, VH * scale)),
                flip_y: true,
                ..Default::default()
            },
        );

        // ---- Скриншот для проверки визуала ----
        if let Some(path) = &args.screenshot {
            frame_count += 1;
            if frame_count >= args.frames.max(1) {
                let image = macroquad::texture::get_screen_data();
                let text = path.to_string_lossy().into_owned();
                image.export_png(&text);
                println!("скриншот: {text}");
                std::process::exit(0);
            }
        }

        next_frame().await;
    }

    if session.dirty {
        if let Ok(path) = session.save() {
            println!("сохранено при выходе: {}", path.display());
        }
    }
    Ok(())
}

fn rgba8(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

fn first_token_with_character(session: &LocalSession) -> Option<EntityId> {
    session
        .scene()
        .tokens
        .iter()
        .find(|token| token.character.is_some() || token.image.is_some())
        .map(|token| token.id)
        .or_else(|| session.scene().tokens.first().map(|token| token.id))
}
fn selected_positions(session: &LocalSession) -> Vec<(EntityId, Vec2)> {
    session
        .selected_ids()
        .into_iter()
        .filter_map(|token| session.scene().token(token).map(|t| (token, t.pos)))
        .collect()
}

fn edge_at(scene: &vtt_core::scene::Scene, world: Vec2, zoom: f32) -> Option<Edge> {
    let tolerance = 12.0 / zoom.max(0.05);
    let inside_y = world.y >= -tolerance && world.y <= scene.height_px + tolerance;
    let inside_x = world.x >= -tolerance && world.x <= scene.width_px + tolerance;
    if (world.x - scene.width_px).abs() <= tolerance && inside_y {
        Some(Edge::Right)
    } else if (world.y - scene.height_px).abs() <= tolerance && inside_x {
        Some(Edge::Bottom)
    } else {
        None
    }
}

fn dispatch(hub: &TcpHub, outgoing: Vec<Outgoing>) {
    for item in outgoing {
        match item.to {
            Some(vtt_server::HOST_CLIENT) => {}
            Some(client) => {
                let _ = hub.send_to(client, &item.msg);
            }
            None => {
                let _ = hub.broadcast(&item.msg);
            }
        }
    }
}

fn apply_action(session: &mut LocalSession, action: Action, now: f64) {
    match action {
        Action::MoveToken { token, to } => {
            session.apply(Intent::MoveToken { token, to });
        }
        Action::Ping { at } => session.ping(at, now),
        Action::Fog { center, radius } => {
            session.apply(Intent::AddFog { center, radius });
        }
        Action::AddWall { a, b, door } => session.add_wall(a, b, door),
        Action::ToggleDoorNear { at } => {
            session.toggle_door_near(at);
        }
    }
}

fn apply_ui_action(
    session: &mut LocalSession,
    settings: &mut SettingsState,
    action: UiAction,
    hub: Option<&TcpHub>,
    hud_mode: &mut HudMode,
) {
    match action {
        UiAction::SendChat { text, to_gm } => {
            session.apply(Intent::Chat {
                text,
                to_gm_only: to_gm,
            });
        }
        UiAction::RollDice { formula, to_gm } => {
            session.apply(Intent::Roll {
                expr: formula,
                mode: vtt_core::dice::RollMode::Normal,
                to_gm_only: to_gm,
            });
        }
        UiAction::HudMode(mode) => *hud_mode = mode,
        UiAction::EditKey(_) => {}
        UiAction::TokenSize(size) => session.set_size_for_selection(size),
        UiAction::TokenOpacity(opacity) => session.set_opacity_for_selection(opacity),
        UiAction::TokenHidden(hidden) => session.set_hidden_for_selection(hidden),
        UiAction::Grid(kind) => {
            let grid = session.scene().grid;
            session.set_grid(kind, grid.cell_px, grid.feet_per_cell);
        }
        UiAction::CellPx(cell) => {
            let grid = session.scene().grid;
            session.set_grid(grid.kind, cell, grid.feet_per_cell);
        }
        UiAction::FeetPerCell(feet) => {
            let grid = session.scene().grid;
            session.set_grid(grid.kind, grid.cell_px, feet);
        }
        UiAction::ClearMap => session.set_background(None),
        UiAction::Role(player, role) => session.set_player_role(player, role),
        UiAction::Kick(player) => {
            if let Some(client) = session.client_of(player) {
                if let Some(hub) = hub {
                    let _ = hub.send_to(
                        client,
                        &ServerMsg::Rejected {
                            reason: "ГМ отключил вас от комнаты".to_string(),
                        },
                    );
                    hub.drop_client(client);
                }
                session.server.leave(client);
            }
        }
        UiAction::ToggleFog => session.show_fog = !session.show_fog,
        UiAction::PickMap => {}
        UiAction::CloseSettings => settings.open = false,
    }
}

fn apply_sheet_action(session: &mut LocalSession, action: SheetAction, cursor: Vec2) {
    match action {
        SheetAction::UpdateAttribute(id, attr, delta) => {
            let Some(character) = session.character(id) else {
                return;
            };
            let mut attributes = character.attributes;
            let value = (i32::from(attributes.get(attr)) + delta).clamp(1, 30) as u8;
            attributes.set(attr, value);
            session.update_character(
                id,
                CharacterPatch {
                    attributes: Some(attributes),
                    ..CharacterPatch::default()
                },
            );
        }
        SheetAction::SetSkill(id, skill, delta) => {
            let Some(character) = session.character(id) else {
                return;
            };
            let base = i32::from(character.skills.get(&skill).copied().unwrap_or(0));
            let value = (base + delta).clamp(0, 20) as u8;
            let mut skills = BTreeMap::new();
            skills.insert(skill, value);
            session.update_character(
                id,
                CharacterPatch {
                    skills: Some(skills),
                    ..CharacterPatch::default()
                },
            );
        }
        SheetAction::Damage(id, amount, mental) => adjust_health(session, id, -amount, mental),
        SheetAction::Heal(id, amount, mental) => adjust_health(session, id, amount, mental),
        SheetAction::RollCheck {
            character,
            skill,
            dc,
        } => session.roll_check(character, skill, dc),
        SheetAction::RollAttribute {
            character,
            attr,
            dc,
        } => session.roll_attr(character, attr, dc),
        SheetAction::RollSanity(id) => session.roll_sanity(id),
        SheetAction::GrantXp(id, amount) => session.grant_xp(id, amount),
        SheetAction::LevelUp(id, attr) => session.level_up(id, attr),
        SheetAction::SpawnToken(id) => session.spawn_character_token(id, cursor),
        SheetAction::Close => session.show_sheet = false,
    }
}

fn adjust_health(session: &mut LocalSession, id: EntityId, delta: i32, mental: bool) {
    let Some(character) = session.character(id) else {
        return;
    };
    let (current, max) = if mental {
        (character.mental.current, character.mental.max)
    } else {
        (character.physical.current, character.physical.max)
    };
    let value = (current + delta).clamp(0, max);
    let patch = if mental {
        CharacterPatch {
            mental: Some(value),
            ..CharacterPatch::default()
        }
    } else {
        CharacterPatch {
            physical: Some(value),
            ..CharacterPatch::default()
        }
    };
    session.update_character(id, patch);
}

fn pick_image_file() -> mpsc::Receiver<Option<PathBuf>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let file = rfd::FileDialog::new()
            .add_filter("Картинки", &["png", "jpg", "jpeg", "bmp", "webp"])
            .set_title("Выберите картинку")
            .pick_file();
        let _ = tx.send(file);
    });
    rx
}

fn parse_args() -> Args {
    let mut args = Args {
        dir: PathBuf::from("./campaign"),
        map: None,
        name: "Моя кампания".to_string(),
        port: panels::PORT,
        screenshot: None,
        frames: 15,
        panel: None,
        demo_art: None,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dir" | "-d" => {
                if let Some(value) = iter.next() {
                    args.dir = PathBuf::from(value);
                }
            }
            "--map" | "-m" => {
                if let Some(value) = iter.next() {
                    args.map = Some(PathBuf::from(value));
                }
            }
            "--name" | "-n" => {
                if let Some(value) = iter.next() {
                    args.name = value;
                }
            }
            "--port" | "-p" => {
                if let Some(value) = iter.next() {
                    args.port = value.parse().unwrap_or(panels::PORT);
                }
            }
            "--screenshot" => {
                if let Some(value) = iter.next() {
                    args.screenshot = Some(PathBuf::from(value));
                }
            }
            "--frames" => {
                if let Some(value) = iter.next() {
                    args.frames = value.parse().unwrap_or(15);
                }
            }
            "--demo-art" => {
                if let Some(value) = iter.next() {
                    args.demo_art = Some(PathBuf::from(value));
                }
            }
            "--panel" => {
                if let Some(value) = iter.next() {
                    args.panel = Some(value);
                }
            }
            "--help" | "-h" => {
                println!(
                    "vtt-play [--dir <каталог>] [--map <картинка>] [--name <кампания>] [--port <порт>]"
                );
                std::process::exit(0);
            }
            other => eprintln!("неизвестный аргумент: {other}"),
        }
    }
    args
}

async fn load_assets(map: Option<&PathBuf>) -> Assets {
    let mut texture = None;
    if let Some(path) = map {
        let text = path.to_string_lossy().into_owned();
        match draw::load_texture(&text) {
            Some(loaded) => texture = Some(loaded),
            None => eprintln!("не удалось загрузить карту: {text}"),
        }
    }
    let mut font = None;
    for path in [
        "C:/Windows/Fonts/segoeui.ttf",
        "C:/Windows/Fonts/arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ] {
        if let Ok(loaded) = macroquad::text::load_ttf_font(path).await {
            font = Some(loaded);
            break;
        }
    }
    Assets { map: texture, font }
}
