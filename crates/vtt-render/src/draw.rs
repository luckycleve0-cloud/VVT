//! Отрисовка сцены (прототип на `macroquad`).
//!
//! Здесь только рисование: никаких решений и никакого состояния игры.
//! Всё, что нужно знать о мире, приходит параметрами. Поэтому при переходе на
//! `wgpu` этот файл заменяется целиком, а `camera`, `interact` и `session`
//! остаются без изменений.

use macroquad::prelude::*;

use vtt_core::geom::Vec2;
use vtt_core::ids::EntityId;
use vtt_core::scene::{DoorState, FogShape, Scene, WallKind};

use crate::camera::Camera;
use crate::interact::Interaction;
use crate::session::LocalSession;

/// Загруженные ресурсы окна.
pub struct Assets {
    /// Текстура карты (если загрузилась).
    pub map: Option<Texture2D>,
    /// Шрифт с кириллицей (если найден).
    pub font: Option<Font>,
}

impl std::fmt::Debug for Assets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Assets")
            .field("map", &self.map.is_some())
            .field("font", &self.font.is_some())
            .finish()
    }
}

/// Кеш картинок токенов: грузим с диска один раз и обрезаем в круг.
#[derive(Default)]
pub struct Textures {
    cache: std::collections::HashMap<String, Texture2D>,
}

impl std::fmt::Debug for Textures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Textures")
            .field("loaded", &self.cache.len())
            .finish()
    }
}

impl Textures {
    /// Текстура для пути; `None`, если файл не читается.
    pub fn get(&mut self, path: &str) -> Option<&Texture2D> {
        if !self.cache.contains_key(path) {
            if let Some(texture) = load_round_texture(path) {
                self.cache.insert(path.to_string(), texture);
            }
        }
        self.cache.get(path)
    }

    #[must_use]
    /// Круглая текстура с выбранной областью (зум и сдвиг кадра).
    pub fn get_cropped(&mut self, path: &str, zoom: f32, ox: f32, oy: f32) -> Option<&Texture2D> {
        let key = format!("{path}|{zoom:.2}|{ox:.3}|{oy:.3}");
        if !self.cache.contains_key(&key) {
            if let Some(texture) = load_round_texture_cropped(path, zoom, ox, oy) {
                self.cache.insert(key.clone(), texture);
            }
        }
        self.cache.get(&key)
    }

    pub fn loaded(&self) -> usize {
        self.cache.len()
    }
}

/// Загрузить картинку и сделать её круглым токеном (альфа за пределами круга).
fn load_round_texture(path: &str) -> Option<Texture2D> {
    let (width, height, bytes) = decode_rgba(path)?;
    let image = round_mask(Image {
        width,
        height,
        bytes,
    });
    Some(Texture2D::from_image(&image))
}

/// Загрузить текстуру (карту) без обрезки.
pub fn load_texture(path: &str) -> Option<Texture2D> {
    let (width, height, bytes) = decode_rgba(path)?;
    Some(Texture2D::from_image(&Image {
        width,
        height,
        bytes,
    }))
}

/// Декодировать картинку в RGBA8 (png/jpeg/bmp/webp).
///
/// `macroquad` умеет только PNG, поэтому JPEG/BMP/WebP идут через крейт `image`.
pub fn decode_rgba(path: &str) -> Option<(u16, u16, Vec<u8>)> {
    let bytes = std::fs::read(path).ok()?;
    if let Ok(image) = Image::from_file_with_format(&bytes, None) {
        return Some((image.width, image.height, image.bytes));
    }
    let decoded = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (width, height) = decoded.dimensions();
    Some((width as u16, height as u16, decoded.into_raw()))
}

/// Вырезать круг по меньшей стороне и сгладить край.
fn round_mask(mut image: Image) -> Image {
    let width = u32::from(image.width);
    let height = u32::from(image.height);
    if width == 0 || height == 0 {
        return image;
    }
    let radius = width.min(height) as f32 / 2.0;
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > radius {
                image.set_pixel(x, y, Color::new(0.0, 0.0, 0.0, 0.0));
            } else if distance > radius - 1.5 {
                let mut color = image.get_pixel(x, y);
                color.a *= ((radius - distance) / 1.5).clamp(0.0, 1.0);
                image.set_pixel(x, y, color);
            }
        }
    }
    image
}

pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba(r, g, b, a)
}

fn to_screen(camera: &Camera, viewport: Vec2, world: Vec2) -> Vec2 {
    camera.world_to_screen(world, viewport)
}

pub fn text(assets: &Assets, body: &str, x: f32, y: f32, size: f32, color: Color) {
    draw_text_ex(
        body,
        x,
        y,
        TextParams {
            font: assets.font.as_ref(),
            font_size: size as u16,
            color,
            ..Default::default()
        },
    );
}

pub fn text_width(assets: &Assets, body: &str, size: f32) -> f32 {
    measure_text(body, assets.font.as_ref(), size as u16, 1.0).width
}

/// Режим верхней информационной плашки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HudMode {
    /// Только название, сцена и игроки.
    #[default]
    Compact,
    /// Полная версия с зумом и подсказками.
    Full,
    /// Скрыть совсем.
    Hidden,
}

/// Нарисовать кадр целиком.
#[allow(clippy::too_many_arguments)]
pub fn frame(
    camera: &Camera,
    viewport: Vec2,
    session: &LocalSession,
    assets: &Assets,
    textures: &mut Textures,
    interaction: &Interaction,
    previews: &[(EntityId, Vec2)],
    wall_preview: Option<(Vec2, Vec2)>,
    rubber: Option<(Vec2, Vec2)>,
    show_grid: bool,
    hud_x: f32,
    hud_y: f32,
    hud: HudMode,
) {
    clear_background(rgba(18, 20, 24, 255));
    let scene = session.scene();

    draw_map(camera, viewport, scene, assets);
    if show_grid {
        draw_grid(camera, viewport, scene);
    }
    draw_walls(camera, viewport, scene);
    draw_tokens(
        camera,
        viewport,
        scene,
        assets,
        textures,
        previews,
        &session.selection,
    );
    draw_fog(camera, viewport, scene);
    draw_fog_cells(camera, viewport, session);
    draw_measure(camera, viewport, scene, assets, interaction);
    draw_wall_preview(camera, viewport, wall_preview);
    draw_rubber(camera, viewport, rubber);
    draw_pings(camera, viewport, session, assets);
    draw_hud(
        camera,
        session,
        assets,
        interaction,
        show_grid,
        hud_x,
        hud_y,
        hud,
        viewport,
    );
}

fn draw_map(camera: &Camera, viewport: Vec2, scene: &Scene, assets: &Assets) {
    let pos = to_screen(camera, viewport, Vec2::ZERO);
    let size = Vec2::new(scene.width_px, scene.height_px) * camera.zoom;

    match (&assets.map, &scene.background) {
        (Some(texture), Some(_)) => {
            draw_texture_ex(
                texture,
                pos.x,
                pos.y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(size.x, size.y)),
                    ..Default::default()
                },
            );
        }
        _ => {
            draw_rectangle(pos.x, pos.y, size.x, size.y, rgba(58, 62, 74, 255));
            draw_rectangle_lines(pos.x, pos.y, size.x, size.y, 2.0, rgba(100, 106, 122, 255));
            text(
                assets,
                "карта не задана — R → «Поле» → «Выбрать карту»",
                pos.x + 16.0,
                pos.y + 32.0,
                20.0,
                rgba(190, 196, 210, 255),
            );
        }
    }
}

fn draw_grid(camera: &Camera, viewport: Vec2, scene: &Scene) {
    let cell = scene.grid.cell_px;
    if cell <= 0.0 {
        return;
    }
    let step = cell * camera.zoom;
    if step < 6.0 {
        return;
    }
    let (min, max) = camera.visible_world_rect(viewport);
    let color = rgba(255, 255, 255, 26);

    let mut x = (min.x / cell).floor() * cell;
    while x <= max.x {
        let screen = to_screen(camera, viewport, Vec2::new(x, 0.0));
        draw_line(screen.x, 0.0, screen.x, viewport.y, 1.0, color);
        x += cell;
    }
    let mut y = (min.y / cell).floor() * cell;
    while y <= max.y {
        let screen = to_screen(camera, viewport, Vec2::new(0.0, y));
        draw_line(0.0, screen.y, viewport.x, screen.y, 1.0, color);
        y += cell;
    }
}

fn draw_walls(camera: &Camera, viewport: Vec2, scene: &Scene) {
    for wall in &scene.walls {
        let a = to_screen(camera, viewport, wall.a);
        let b = to_screen(camera, viewport, wall.b);
        let color = match wall.kind {
            WallKind::Door(DoorState::Open) => rgba(120, 220, 140, 220),
            WallKind::Door(_) => rgba(220, 180, 90, 220),
            WallKind::Window => rgba(120, 200, 240, 200),
            WallKind::Wall => rgba(200, 205, 215, 200),
        };
        draw_line(a.x, a.y, b.x, b.y, 2.0, color);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tokens(
    camera: &Camera,
    viewport: Vec2,
    scene: &Scene,
    assets: &Assets,
    textures: &mut Textures,
    previews: &[(EntityId, Vec2)],
    selection: &std::collections::BTreeSet<EntityId>,
) {
    for token in &scene.tokens {
        let world = previews
            .iter()
            .find(|(id, _)| *id == token.id)
            .map_or(token.pos, |(_, pos)| *pos);
        let center = to_screen(camera, viewport, world);
        let radius = (token.size.cells() * scene.grid.cell_px * 0.5).max(4.0) * camera.zoom;

        let alpha = (if token.hidden { 110.0 } else { 255.0 }) * token.opacity.clamp(0.05, 1.0);
        let fill = if token.dead {
            rgba(110, 110, 118, alpha as u8)
        } else if token.owner.is_some() {
            rgba(70, 130, 200, alpha as u8)
        } else {
            rgba(190, 90, 70, alpha as u8)
        };

        // Картинка токена (уже обрезана в круг), иначе — цветной кружок.
        let mut drew_image = false;
        if let Some(path) = &token.image {
            if let Some(texture) = textures.get(path) {
                draw_texture_ex(
                    texture,
                    center.x - radius,
                    center.y - radius,
                    Color::new(1.0, 1.0, 1.0, alpha / 255.0),
                    DrawTextureParams {
                        dest_size: Some(vec2(radius * 2.0, radius * 2.0)),
                        ..Default::default()
                    },
                );
                drew_image = true;
            }
        }
        if !drew_image {
            draw_circle(center.x, center.y, radius, fill);
            draw_circle_lines(center.x, center.y, radius, 2.0, rgba(255, 255, 255, 150));
        }

        draw_token_frame(center, radius, camera.zoom, token.frame, token.frame_color);

        if token.concentration.is_some() {
            draw_circle_lines(
                center.x,
                center.y,
                radius + 4.0,
                2.0,
                rgba(120, 200, 255, 210),
            );
        }

        if selection.contains(&token.id) {
            draw_circle_lines(
                center.x,
                center.y,
                radius + 7.0,
                3.0,
                rgba(255, 220, 120, 240),
            );
        }

        let name_size = 14.0f32.max(radius * 0.4).min(28.0);
        let width = text_width(assets, &token.name, name_size);
        text(
            assets,
            &token.name,
            center.x - width * 0.5,
            center.y - radius - 8.0,
            name_size,
            rgba(235, 238, 245, 255),
        );

        if let Some(hp) = token.hp {
            let bar_w = radius * 2.0;
            let bar_h = (4.0 * camera.zoom).clamp(3.0, 8.0);
            let y = center.y + radius + 4.0;
            draw_rectangle(center.x - radius, y, bar_w, bar_h, rgba(30, 30, 34, 220));
            let ratio = if hp.max > 0 {
                (f32::from(hp.value as i16) / f32::from(hp.max as i16)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            draw_rectangle(
                center.x - radius,
                y,
                bar_w * ratio,
                bar_h,
                rgba(200, 60, 60, 255),
            );
        }
    }
}

/// Рамка токена: кольцо, двойное кольцо или пунктир.
pub(crate) fn draw_token_frame(
    center: Vec2,
    radius: f32,
    zoom: f32,
    frame: vtt_core::entities::TokenFrame,
    color: [u8; 3],
) {
    use vtt_core::entities::TokenFrame;
    if frame == TokenFrame::None {
        return;
    }
    let stroke = Color::from_rgba(color[0], color[1], color[2], 255);
    let thickness = (2.5 * zoom).clamp(1.5, 7.0);

    match frame {
        TokenFrame::Ring => draw_circle_lines(center.x, center.y, radius, thickness, stroke),
        TokenFrame::Double => {
            draw_circle_lines(center.x, center.y, radius, thickness * 0.7, stroke);
            draw_circle_lines(
                center.x,
                center.y,
                (radius - 4.0 * zoom).max(radius * 0.6),
                thickness * 0.5,
                stroke,
            );
        }
        TokenFrame::Dashed => {
            const DASHES: usize = 24;
            for index in 0..DASHES {
                if index % 2 != 0 {
                    continue;
                }
                let a0 = std::f32::consts::TAU * index as f32 / DASHES as f32;
                let a1 = std::f32::consts::TAU * (index + 1) as f32 / DASHES as f32;
                let p0 = Vec2::new(center.x + a0.cos() * radius, center.y + a0.sin() * radius);
                let p1 = Vec2::new(center.x + a1.cos() * radius, center.y + a1.sin() * radius);
                draw_line(p0.x, p0.y, p1.x, p1.y, thickness, stroke);
            }
        }
        TokenFrame::None => {}
    }
}

/// Прямоугольник выделения мышью.
fn draw_rubber(camera: &Camera, viewport: Vec2, rubber: Option<(Vec2, Vec2)>) {
    let Some((from, to)) = rubber else {
        return;
    };
    let a = to_screen(camera, viewport, from);
    let b = to_screen(camera, viewport, to);
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let width = (a.x - b.x).abs();
    let height = (a.y - b.y).abs();
    draw_rectangle(x, y, width, height, rgba(120, 180, 255, 35));
    draw_rectangle_lines(x, y, width, height, 1.5, rgba(140, 200, 255, 230));
}

fn draw_fog(camera: &Camera, viewport: Vec2, scene: &Scene) {
    let fog = rgba(5, 6, 8, 235);
    for shape in &scene.fog.shapes {
        match shape {
            FogShape::Circle { center, radius } => {
                let screen = to_screen(camera, viewport, *center);
                draw_circle(screen.x, screen.y, radius * camera.zoom, fog);
            }
            FogShape::Rect { min, max } => {
                let a = to_screen(camera, viewport, *min);
                let b = to_screen(camera, viewport, *max);
                draw_rectangle(a.x, a.y, b.x - a.x, b.y - a.y, fog);
            }
            // Полигоны появятся вместе с инструментом «лассо».
            FogShape::Polygon { .. } => {}
        }
    }
}

fn draw_measure(
    camera: &Camera,
    viewport: Vec2,
    scene: &Scene,
    assets: &Assets,
    interaction: &Interaction,
) {
    if let Some((from, to)) = interaction.measure_line() {
        let a = to_screen(camera, viewport, from);
        let b = to_screen(camera, viewport, to);
        draw_line(a.x, a.y, b.x, b.y, 2.0, rgba(255, 230, 120, 230));
        draw_circle(a.x, a.y, 4.0, rgba(255, 230, 120, 230));
        draw_circle(b.x, b.y, 4.0, rgba(255, 230, 120, 230));

        let feet = scene.grid.distance_ft(from, to);
        text(
            assets,
            &format!("{feet:.0} фт"),
            b.x + 10.0,
            b.y - 10.0,
            20.0,
            rgba(255, 235, 150, 255),
        );
    }
}

/// Предпросмотр стены, которую сейчас тянут мышью.
fn draw_wall_preview(camera: &Camera, viewport: Vec2, wall: Option<(Vec2, Vec2)>) {
    let Some((from, to)) = wall else {
        return;
    };
    let a = to_screen(camera, viewport, from);
    let b = to_screen(camera, viewport, to);
    draw_line(a.x, a.y, b.x, b.y, 3.0, rgba(255, 210, 90, 220));
    draw_circle(a.x, a.y, 4.0, rgba(255, 210, 90, 220));
    draw_circle(b.x, b.y, 4.0, rgba(255, 210, 90, 220));
}

/// Туман по клеткам: невидимое — чёрное, виденное раньше — тёмное.
fn draw_fog_cells(camera: &Camera, viewport: Vec2, session: &LocalSession) {
    if !session.fog_active || !session.show_fog {
        return;
    }
    let scene = session.scene();
    let cell = scene.grid.cell_px;
    if cell <= 0.0 {
        return;
    }
    let (min, max) = camera.visible_world_rect(viewport);
    let min_cell = scene.grid.cell_of(min);
    let max_cell = scene.grid.cell_of(max);

    for cy in min_cell.1..=max_cell.1 {
        for cx in min_cell.0..=max_cell.0 {
            if session.visible.contains(&(cx, cy)) {
                continue;
            }
            let explored = session.explored.contains(&(cx, cy));
            let alpha = if explored { 155 } else { 246 };
            let center = scene.grid.center_of((cx, cy));
            let corner = Vec2::new(center.x - cell * 0.5, center.y - cell * 0.5);
            let pos = to_screen(camera, viewport, corner);
            let size = cell * camera.zoom + 0.5;
            draw_rectangle(pos.x, pos.y, size, size, rgba(4, 5, 8, alpha));
        }
    }
}

fn draw_pings(camera: &Camera, viewport: Vec2, session: &LocalSession, assets: &Assets) {
    for ping in &session.pings {
        let screen = to_screen(camera, viewport, ping.at);
        let progress = ping.progress(session_clock());
        let radius = 12.0 + 70.0 * progress;
        let alpha = ((1.0 - progress) * 220.0) as u8;
        draw_circle_lines(screen.x, screen.y, radius, 3.0, rgba(255, 210, 90, alpha));
        text(
            assets,
            "!",
            screen.x - 4.0,
            screen.y + 8.0,
            26.0,
            rgba(255, 210, 90, alpha),
        );
    }
}

/// Время кадра; вынесено отдельно, чтобы `draw` не зависел от `macroquad`-цикла.
fn session_clock() -> f64 {
    get_time()
}

#[allow(clippy::too_many_arguments)]
fn draw_hud(
    camera: &Camera,
    session: &LocalSession,
    assets: &Assets,
    interaction: &Interaction,
    show_grid: bool,
    hud_x: f32,
    hud_y: f32,
    hud: HudMode,
    _viewport: Vec2,
) {
    if hud == HudMode::Hidden {
        return;
    }
    let campaign = &session.server.campaign.name;
    let scene = session.scene();
    let x0 = hud_x;
    let y0 = hud_y + 4.0;

    if hud == HudMode::Compact {
        draw_rectangle(x0, y0, 320.0, 82.0, rgba(12, 14, 18, 220));
        text(
            assets,
            campaign,
            x0 + 12.0,
            y0 + 28.0,
            20.0,
            rgba(240, 242, 248, 255),
        );
        text(
            assets,
            &format!("сцена «{}»", scene.name),
            x0 + 12.0,
            y0 + 50.0,
            15.0,
            rgba(170, 176, 192, 255),
        );
        text(
            assets,
            &format!("игроков: {}", session.server.client_count()),
            x0 + 12.0,
            y0 + 70.0,
            14.0,
            rgba(150, 158, 174, 255),
        );
        return;
    }

    draw_rectangle(x0, y0, 470.0, 146.0, rgba(12, 14, 18, 220));
    text(
        assets,
        campaign,
        x0 + 12.0,
        y0 + 28.0,
        24.0,
        rgba(240, 242, 248, 255),
    );
    text(
        assets,
        &format!("сцена «{}» · {}", scene.name, scene.id),
        x0 + 12.0,
        y0 + 54.0,
        18.0,
        rgba(170, 176, 192, 255),
    );
    text(
        assets,
        &format!(
            "инструмент: {} · игроков: {} · туман: {} · кадров: {}",
            interaction.tool.name_ru(),
            session.server.client_count(),
            scene.fog.shapes.len(),
            get_fps()
        ),
        x0 + 12.0,
        y0 + 80.0,
        18.0,
        rgba(170, 176, 192, 255),
    );
    text(
        assets,
        &format!(
            "зум: {:.0}% · сетка: {} · {}",
            camera.zoom * 100.0,
            if show_grid { "вкл" } else { "выкл" },
            if session.dirty {
                "есть изменения"
            } else {
                "сохранено"
            }
        ),
        x0 + 12.0,
        y0 + 106.0,
        18.0,
        rgba(140, 150, 170, 255),
    );
    text(
        assets,
        "иконки сверху · колесо зум · ПКМ панорама · S сохранить",
        x0 + 12.0,
        y0 + 132.0,
        14.0,
        rgba(150, 158, 174, 255),
    );
}
/// Подсказка в консоль при запуске.
pub fn print_controls() {
    println!(
        "\
Управление окном:
  ЛКМ                выбрать/двинуть токен, применить инструмент
  Ctrl+ЛКМ           добавить токен к выделению
  рамка мышью        выделить группу токенов
  ПКМ или Space      панорамирование
  колесо             зум шагом 10% (10–400%)
  1 2 3 4 5 6        инструменты: выбор, линейка, туман, пинг, стена, дверь
  P                  мастер создания персонажа
  L                  список персонажей (как Characters в Roll20)
  Tab                лист персонажа выбранного токена
  Y                  панель токена: размер, прозрачность, скрытие
  T                  чат (Enter — отправить)
  R                  настройки комнаты и поля
  F                  туман по клеткам
  край карты         потянуть, чтобы изменить границы поля (ГМ)
  G                  показать/скрыть сетку
  B                  поставить гоблина в точку курсора
  N                  создать пустой токен
  Del                удалить выбранные токены
  C                  убрать весь туман
  S                  сохранить кампанию
  Esc                отменить действие
  Ctrl+Q             выход (сохраняет кампанию)"
    );
}

/// Мягкая тень под панелью/окном (несколько прямоугольников с прозрачностью).
pub fn panel_shadow(x: f32, y: f32, w: f32, h: f32, strength: f32) {
    for step in 1..=4 {
        let grow = step as f32 * 3.0;
        let alpha = (strength / step as f32) * 0.35;
        draw_rectangle(
            x - grow,
            y + 4.0,
            w + grow * 2.0,
            h + grow,
            rgba(0, 0, 0, (alpha * 255.0) as u8),
        );
    }
}

/// Панель со скруглёнными углами и рамкой (аппроксимация кругами).
pub fn rounded_panel(x: f32, y: f32, w: f32, h: f32, fill: Color, border: Color) {
    let radius = 8.0;
    draw_rectangle(x + radius, y, w - radius * 2.0, h, fill);
    draw_rectangle(x, y + radius, w, h - radius * 2.0, fill);
    draw_circle(x + radius, y + radius, radius, fill);
    draw_circle(x + w - radius, y + radius, radius, fill);
    draw_circle(x + radius, y + h - radius, radius, fill);
    draw_circle(x + w - radius, y + h - radius, radius, fill);
    draw_line(x + radius, y, x + w - radius, y, 1.5, border);
    draw_line(x + radius, y + h, x + w - radius, y + h, 1.5, border);
    draw_line(x, y + radius, x, y + h - radius, 1.5, border);
    draw_line(x + w, y + radius, x + w, y + h - radius, 1.5, border);
}

/// Вырезать из картинки квадратную область под круглый токен.
///
/// `zoom` >= 1 — насколько крупнее берём область; `ox`/`oy` — сдвиг в долях
/// ширины/высоты (перетаскивание внутри круга).
fn crop_square(path: &str, zoom: f32, ox: f32, oy: f32) -> Option<image::RgbaImage> {
    let (width, height, bytes) = decode_rgba(path)?;
    let image = image::RgbaImage::from_raw(width as u32, height as u32, bytes)?;
    let side = ((width.min(height) as f32) / zoom.max(1.0)).round() as u32;
    let side = side.clamp(8, width as u32).min(height as u32);
    let half = side as f32 * 0.5;
    let cx = (width as f32 * 0.5 + ox * width as f32).clamp(half, width as f32 - half);
    let cy = (height as f32 * 0.5 + oy * height as f32).clamp(half, height as f32 - half);
    let x0 = (cx - half).round().max(0.0) as u32;
    let y0 = (cy - half).round().max(0.0) as u32;
    Some(image::imageops::crop_imm(&image, x0, y0, side, side).to_image())
}

/// Круглая текстура из выбранной области картинки.
fn load_round_texture_cropped(path: &str, zoom: f32, ox: f32, oy: f32) -> Option<Texture2D> {
    let cropped = crop_square(path, zoom, ox, oy)?;
    let resized =
        image::imageops::resize(&cropped, 256, 256, image::imageops::FilterType::Lanczos3);
    let image = Image {
        width: 256,
        height: 256,
        bytes: resized.into_raw(),
    };
    Some(Texture2D::from_image(&round_mask(image)))
}

/// Сохранить выбранную область картинки квадратным PNG (для токена).
pub fn save_cropped_token(path: &str, zoom: f32, ox: f32, oy: f32, out: &std::path::Path) -> bool {
    let Some(cropped) = crop_square(path, zoom, ox, oy) else {
        return false;
    };
    let resized =
        image::imageops::resize(&cropped, 256, 256, image::imageops::FilterType::Lanczos3);
    resized.save(out).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_dir() -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-images");
        std::fs::create_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn decodes_png_and_jpeg() {
        let rgb = image::RgbImage::from_pixel(64, 48, image::Rgb([200, 100, 50]));

        let png = image_dir().join("decoder.png");
        rgb.clone().save(&png).expect("png saved");
        let (width, height, bytes) = decode_rgba(png.to_str().unwrap()).expect("png decoded");
        assert_eq!((width, height), (64, 48));
        assert_eq!(bytes.len(), 64 * 48 * 4);

        let jpg = image_dir().join("decoder.jpg");
        image::DynamicImage::ImageRgb8(rgb)
            .save(&jpg)
            .expect("jpeg saved");
        let (width, height, bytes) = decode_rgba(jpg.to_str().unwrap()).expect("jpeg decoded");
        assert_eq!((width, height), (64, 48));
        assert_eq!(bytes.len(), 64 * 48 * 4);
    }

    #[test]
    fn round_mask_makes_corners_transparent() {
        let image = Image {
            width: 32,
            height: 32,
            bytes: vec![255u8; 32 * 32 * 4],
        };
        let masked = round_mask(image);
        let center_alpha = masked.bytes[(16 * 32 + 16) * 4 + 3];
        let corner_alpha = masked.bytes[3];
        assert_eq!(center_alpha, 255, "центр должен остаться непрозрачным");
        assert_eq!(corner_alpha, 0, "угол должен стать прозрачным");
    }
}
