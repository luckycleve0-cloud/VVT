//! Сцена: карта, токены, стены, свет и туман.
//!
//! Пока это «сердце» данных стола и почти всё, что нужно для первого
//! играбельного среза. Динамический свет и полигональная видимость —
//! следующие шаги (см. `docs/ROADMAP.md`), здесь есть только проверка
//! линии видимости через отрезки стен.

use crate::entities::{Light, Token};
use crate::geom::{segments_intersect, Vec2};
use crate::grid::Grid;
use crate::ids::EntityId;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Стены и двери
// ---------------------------------------------------------------------------

/// Состояние двери.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorState {
    #[default]
    Closed,
    Open,
    Locked,
    /// Секретная дверь: выглядит как стена, пока ГМ не откроет.
    Secret,
}

/// Тип препятствия.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallKind {
    #[default]
    Wall,
    /// Окно: не мешает зрению, но мешает пройти.
    Window,
    Door(DoorState),
}

/// Отрезок стены.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub a: Vec2,
    pub b: Vec2,
    pub kind: WallKind,
    pub blocks_sight: bool,
    pub blocks_movement: bool,
}

impl Wall {
    /// Обычная глухая стена.
    #[must_use]
    pub fn solid(a: Vec2, b: Vec2) -> Self {
        Self {
            a,
            b,
            kind: WallKind::Wall,
            blocks_sight: true,
            blocks_movement: true,
        }
    }

    /// Окно: видно, но не пройти.
    #[must_use]
    pub fn window(a: Vec2, b: Vec2) -> Self {
        Self {
            a,
            b,
            kind: WallKind::Window,
            blocks_sight: false,
            blocks_movement: true,
        }
    }

    /// Дверь с состоянием.
    #[must_use]
    pub fn door(a: Vec2, b: Vec2, state: DoorState) -> Self {
        Self {
            a,
            b,
            kind: WallKind::Door(state),
            blocks_sight: true,
            blocks_movement: true,
        }
    }

    /// Блокирует ли зрение **сейчас** (открытая дверь — нет).
    #[must_use]
    pub fn blocks_sight_now(&self) -> bool {
        self.blocks_sight && !matches!(self.kind, WallKind::Door(DoorState::Open))
    }

    /// Блокирует ли проход **сейчас**.
    #[must_use]
    pub fn blocks_movement_now(&self) -> bool {
        self.blocks_movement && !matches!(self.kind, WallKind::Door(DoorState::Open))
    }

    /// Меняет ли взгляд положение двери.
    pub fn toggle_door(&mut self) {
        if let WallKind::Door(state) = self.kind {
            self.kind = WallKind::Door(match state {
                DoorState::Open => DoorState::Closed,
                DoorState::Closed | DoorState::Locked | DoorState::Secret => DoorState::Open,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Свет и туман
// ---------------------------------------------------------------------------

/// Источник света на карте (факел, костёр, лампа).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LightSource {
    pub id: EntityId,
    pub pos: Vec2,
    pub light: Light,
    pub enabled: bool,
}

/// Режим тумана.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FogMode {
    /// Туман выключен.
    #[default]
    Off,
    /// Ручной: ГМ закрашивает области.
    Manual,
    /// «Память исследования»: области открываются при появлении токенов.
    Explored,
}

/// Фигура тумана.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FogShape {
    Rect { min: Vec2, max: Vec2 },
    Circle { center: Vec2, radius: f32 },
    Polygon { points: Vec<Vec2> },
}

impl FogShape {
    #[must_use]
    pub fn contains(&self, p: Vec2) -> bool {
        match self {
            Self::Rect { min, max } => p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y,
            Self::Circle { center, radius } => center.distance(p) <= *radius,
            Self::Polygon { points } => point_in_polygon(p, points),
        }
    }
}

/// Состояние тумана на сцене.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Fog {
    pub mode: FogMode,
    /// Области, закрытые туманом (ручной режим).
    pub shapes: Vec<FogShape>,
}

impl Fog {
    /// Закрыта ли точка туманом.
    #[must_use]
    pub fn covers(&self, p: Vec2) -> bool {
        self.mode == FogMode::Manual && self.shapes.iter().any(|shape| shape.contains(p))
    }

    pub fn add_shape(&mut self, shape: FogShape) {
        self.shapes.push(shape);
    }

    pub fn clear(&mut self) {
        self.shapes.clear();
    }
}

fn point_in_polygon(p: Vec2, points: &[Vec2]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (pi, pj) = (points[i], points[j]);
        let crosses = (pi.y > p.y) != (pj.y > p.y)
            && p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x;
        if crosses {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ---------------------------------------------------------------------------
// Шаблоны областей (AoE)
// ---------------------------------------------------------------------------

/// Форма шаблона заклинания.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateKind {
    #[default]
    Circle,
    Cone,
    Line,
    Cube,
    Cylinder,
    Sphere,
}

/// Шаблон области на сцене.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub id: EntityId,
    pub kind: TemplateKind,
    pub origin: Vec2,
    /// Размер в футах: радиус для круга/сферы, длина для конуса/линии.
    pub size_ft: f32,
    /// Направление в радианах (0 — вправо, по часовой стрелке).
    pub direction_rad: f32,
    pub color: [u8; 3],
}

impl Template {
    #[must_use]
    pub fn new(id: EntityId, kind: TemplateKind, origin: Vec2, size_ft: f32) -> Self {
        Self {
            id,
            kind,
            origin,
            size_ft,
            direction_rad: 0.0,
            color: [235, 120, 40],
        }
    }

    /// Попадает ли точка внутрь шаблона (приближённо, в футах).
    #[must_use]
    pub fn contains(&self, p: Vec2, grid: &Grid) -> bool {
        let px_per_ft = (grid.cell_px() / grid.feet_per_cell).max(0.001);
        let delta = p - self.origin;
        let distance_ft = delta.length() / px_per_ft;
        let dir = Vec2::new(self.direction_rad.cos(), self.direction_rad.sin());

        match self.kind {
            TemplateKind::Circle | TemplateKind::Cylinder | TemplateKind::Sphere => {
                distance_ft <= self.size_ft
            }
            TemplateKind::Cone => {
                if distance_ft > self.size_ft {
                    return false;
                }
                if distance_ft <= f32::EPSILON {
                    return true;
                }
                // Конус 5e: угол ~53°, половина — atan(0.5).
                let half_angle = 0.5_f32.atan();
                let angle = dir.cross(delta).atan2(dir.dot(delta)).abs();
                angle <= half_angle
            }
            TemplateKind::Line => {
                if distance_ft < f32::EPSILON {
                    return false;
                }
                let along = delta.dot(dir) / px_per_ft;
                let across = (dir.cross(delta) / px_per_ft).abs();
                (0.0..=self.size_ft).contains(&along) && across <= 2.5
            }
            TemplateKind::Cube => {
                let along = delta.dot(dir) / px_per_ft;
                let across = (dir.cross(delta) / px_per_ft).abs();
                along.abs() <= self.size_ft / 2.0 && across <= self.size_ft / 2.0
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Сцена
// ---------------------------------------------------------------------------

/// Сцена = одна карта со всем содержимым.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: EntityId,
    pub name: String,
    pub grid: Grid,
    pub width_px: f32,
    pub height_px: f32,
    /// Ссылка на фон (позже — хэш ассета).
    pub background: Option<String>,
    pub tokens: Vec<Token>,
    pub walls: Vec<Wall>,
    pub lights: Vec<LightSource>,
    pub fog: Fog,
    pub templates: Vec<Template>,
    pub notes: String,
}

impl Scene {
    #[must_use]
    pub fn new(id: EntityId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            grid: Grid::default(),
            width_px: 1400.0,
            height_px: 1050.0,
            background: None,
            tokens: Vec::new(),
            walls: Vec::new(),
            lights: Vec::new(),
            fog: Fog::default(),
            templates: Vec::new(),
            notes: String::new(),
        }
    }

    #[must_use]
    pub fn with_grid(mut self, grid: Grid) -> Self {
        self.grid = grid;
        self
    }

    pub fn add_token(&mut self, token: Token) -> EntityId {
        let id = token.id;
        self.tokens.push(token);
        id
    }

    pub fn remove_token(&mut self, id: EntityId) -> Option<Token> {
        let idx = self.tokens.iter().position(|t| t.id == id)?;
        Some(self.tokens.remove(idx))
    }

    #[must_use]
    pub fn token(&self, id: EntityId) -> Option<&Token> {
        self.tokens.iter().find(|t| t.id == id)
    }

    pub fn token_mut(&mut self, id: EntityId) -> Option<&mut Token> {
        self.tokens.iter_mut().find(|t| t.id == id)
    }

    /// Передвинуть токен. Возвращает `false`, если токена нет.
    pub fn move_token(&mut self, id: EntityId, pos: Vec2) -> bool {
        match self.token_mut(id) {
            Some(token) => {
                token.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Токен под точкой (верхний по порядку отрисовки).
    #[must_use]
    pub fn token_at(&self, p: Vec2) -> Option<&Token> {
        let cell = self.grid.cell_px();
        self.tokens.iter().rev().find(|token| {
            let half = token.size.cells() * cell / 2.0;
            (p.x - token.pos.x).abs() <= half && (p.y - token.pos.y).abs() <= half
        })
    }

    /// Токены, которые видит участник (скрытые — только ГМ).
    pub fn visible_tokens(&self, is_gm: bool) -> impl Iterator<Item = &Token> {
        self.tokens.iter().filter(move |t| is_gm || !t.hidden)
    }

    #[must_use]
    pub fn distance_ft(&self, a: Vec2, b: Vec2) -> f32 {
        self.grid.distance_ft(a, b)
    }

    /// Перекрыта ли линия от `from` до `to` стеной.
    ///
    /// Простая проверка «отрезок против отрезков»: дёшево и достаточно,
    /// чтобы понять, может ли токен видеть другой. Полигональная видимость
    /// с мягкими краями — отдельная задача (см. ROADMAP).
    #[must_use]
    pub fn los_blocked(&self, from: Vec2, to: Vec2) -> bool {
        self.walls
            .iter()
            .any(|wall| wall.blocks_sight_now() && segments_intersect(from, to, wall.a, wall.b))
    }

    /// Можно ли пройти по прямой (упрощённо, без обхода).
    #[must_use]
    pub fn path_blocked(&self, from: Vec2, to: Vec2) -> bool {
        self.walls
            .iter()
            .any(|wall| wall.blocks_movement_now() && segments_intersect(from, to, wall.a, wall.b))
    }

    /// Открыть/закрыть дверь по индексу стены в списке.
    pub fn toggle_door(&mut self, wall_index: usize) -> bool {
        match self.walls.get_mut(wall_index) {
            Some(wall) => {
                wall.toggle_door();
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::TokenSize;

    fn scene_with_wall() -> Scene {
        let mut scene = Scene::new(EntityId(1), "Данж");
        scene.grid = Grid::default();
        scene
            .walls
            .push(Wall::solid(Vec2::new(100.0, 0.0), Vec2::new(100.0, 500.0)));
        scene
    }

    #[test]
    fn wall_blocks_line_of_sight() {
        let scene = scene_with_wall();
        assert!(scene.los_blocked(Vec2::new(0.0, 100.0), Vec2::new(300.0, 100.0)));
        assert!(!scene.los_blocked(Vec2::new(10.0, 100.0), Vec2::new(90.0, 100.0)));
    }

    #[test]
    fn open_door_does_not_block_sight() {
        let mut scene = Scene::new(EntityId(1), "Дверь");
        scene.walls.push(Wall::door(
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 500.0),
            DoorState::Open,
        ));
        assert!(!scene.los_blocked(Vec2::new(0.0, 100.0), Vec2::new(300.0, 100.0)));

        scene.toggle_door(0);
        assert!(scene.los_blocked(Vec2::new(0.0, 100.0), Vec2::new(300.0, 100.0)));
    }

    #[test]
    fn window_blocks_movement_but_not_sight() {
        let mut scene = Scene::new(EntityId(1), "Окно");
        scene
            .walls
            .push(Wall::window(Vec2::new(100.0, 0.0), Vec2::new(100.0, 500.0)));
        assert!(!scene.los_blocked(Vec2::new(0.0, 100.0), Vec2::new(300.0, 100.0)));
        assert!(scene.path_blocked(Vec2::new(0.0, 100.0), Vec2::new(300.0, 100.0)));
    }

    #[test]
    fn tokens_can_be_added_moved_and_found() {
        let mut scene = Scene::new(EntityId(1), "Поляна");
        let mut token = Token::new(EntityId(2), "Гоблин", Vec2::new(35.0, 35.0));
        token.size = TokenSize::Medium;
        scene.add_token(token);

        assert!(scene.token(EntityId(2)).is_some());
        assert!(scene.token_at(Vec2::new(30.0, 30.0)).is_some());
        assert!(scene.token_at(Vec2::new(500.0, 500.0)).is_none());

        assert!(scene.move_token(EntityId(2), Vec2::new(105.0, 105.0)));
        assert!(scene.token_at(Vec2::new(100.0, 100.0)).is_some());
        assert!(!scene.move_token(EntityId(999), Vec2::ZERO));
    }

    #[test]
    fn large_token_covers_more_area() {
        let mut scene = Scene::new(EntityId(1), "Огр");
        let mut token = Token::new(EntityId(2), "Огр", Vec2::new(100.0, 100.0));
        token.size = TokenSize::Large;
        scene.add_token(token);
        // Large занимает 2 клетки → половина 70px.
        assert!(scene.token_at(Vec2::new(100.0 + 60.0, 100.0)).is_some());
    }

    #[test]
    fn hidden_tokens_filtered_for_players() {
        let mut scene = Scene::new(EntityId(1), "Засада");
        let mut hidden = Token::new(EntityId(2), "Убийца", Vec2::ZERO);
        hidden.hidden = true;
        scene.add_token(hidden);
        scene.add_token(Token::new(EntityId(3), "Друг", Vec2::ZERO));

        assert_eq!(scene.visible_tokens(true).count(), 2);
        assert_eq!(scene.visible_tokens(false).count(), 1);
    }

    #[test]
    fn manual_fog_covers_points() {
        let mut fog = Fog {
            mode: FogMode::Manual,
            ..Fog::default()
        };
        fog.add_shape(FogShape::Rect {
            min: Vec2::new(0.0, 0.0),
            max: Vec2::new(100.0, 100.0),
        });
        assert!(fog.covers(Vec2::new(50.0, 50.0)));
        assert!(!fog.covers(Vec2::new(500.0, 50.0)));

        let polygon = FogShape::Polygon {
            points: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(100.0, 0.0),
                Vec2::new(100.0, 100.0),
            ],
        };
        assert!(polygon.contains(Vec2::new(70.0, 20.0)));
        assert!(!polygon.contains(Vec2::new(10.0, 90.0)));
    }

    #[test]
    fn cone_template_covers_only_front() {
        let grid = Grid::default();
        let template = Template {
            direction_rad: 0.0,
            ..Template::new(EntityId(1), TemplateKind::Cone, Vec2::ZERO, 15.0)
        };
        // 15 футов = 3 клетки = 210 px.
        assert!(template.contains(Vec2::new(140.0, 0.0), &grid));
        assert!(!template.contains(Vec2::new(-140.0, 0.0), &grid));
        assert!(!template.contains(Vec2::new(0.0, 140.0), &grid));
    }

    #[test]
    fn scene_json_roundtrip() {
        let mut scene = scene_with_wall();
        scene.add_token(Token::new(EntityId(7), "Стражник", Vec2::new(50.0, 50.0)));
        let json = serde_json::to_string(&scene).unwrap();
        let back: Scene = serde_json::from_str(&json).unwrap();
        assert_eq!(scene, back);
    }
}
