//! Видимость: что персонаж видит из точки с учётом стен и радиуса.
//!
//! Два представления:
//!
//! - [`can_see`] / [`visible_cells`] — «клеточная» видимость: дёшево, удобно для
//!   маски тумана (прототип окна использует именно её);
//! - [`visibility_polygon`] — полигон видимости угловым обходом: пригодится для
//!   мягкого динамического света на GPU (M4+).
//!
//! Стены берутся из [`Scene::walls`], причём открытые двери не блокируют зрение
//! (см. [`crate::scene::Wall::blocks_sight_now`]).

use crate::geom::Vec2;
use crate::grid::Grid;
use crate::scene::Scene;

/// Видна ли точка `target` из `origin` (с учётом стен и радиуса).
#[must_use]
pub fn can_see(scene: &Scene, origin: Vec2, target: Vec2, radius_px: f32) -> bool {
    if radius_px > 0.0 && origin.distance(target) > radius_px {
        return false;
    }
    !scene.los_blocked(origin, target)
}

/// Все клетки сетки, которые видны из точки.
///
/// Проверяется центр клетки: для тумана этого достаточно и это дёшево.
/// Возвращает координаты клеток `(x, y)`.
#[must_use]
pub fn visible_cells(scene: &Scene, origin: Vec2, radius_px: f32) -> Vec<(i32, i32)> {
    let grid = scene.grid;
    if radius_px <= 0.0 {
        return Vec::new();
    }
    let (min_cell, max_cell) = cells_in_radius(grid, origin, radius_px);
    let mut visible = Vec::new();
    for cy in min_cell.1..=max_cell.1 {
        for cx in min_cell.0..=max_cell.0 {
            let center = grid.center_of((cx, cy));
            if can_see(scene, origin, center, radius_px) {
                visible.push((cx, cy));
            }
        }
    }
    visible
}

/// Границы клеток, попадающих в круг радиуса.
#[must_use]
pub fn cells_in_radius(grid: Grid, origin: Vec2, radius_px: f32) -> ((i32, i32), (i32, i32)) {
    let cell = grid.cell_px();
    let left = ((origin.x - radius_px) / cell).floor() as i32;
    let right = ((origin.x + radius_px) / cell).ceil() as i32;
    let top = ((origin.y - radius_px) / cell).floor() as i32;
    let bottom = ((origin.y + radius_px) / cell).ceil() as i32;
    ((left, top), (right, bottom))
}

/// Полигон видимости: точки, ограничивающие видимую область.
///
/// Алгоритм — угловой обход: собираем направления на концы стен и на точки
/// окружности, сортируем по углу и для каждого пускаем луч до ближайшей стены.
/// Результат упорядочен по углу, поэтому его можно заливать как `TriangleFan`.
#[must_use]
pub fn visibility_polygon(scene: &Scene, origin: Vec2, radius_px: f32) -> Vec<Vec2> {
    if radius_px <= 0.0 {
        return Vec::new();
    }

    // Кандидаты по углу: концы стен в радиусе и точки окружности.
    let mut angles: Vec<f32> = Vec::new();
    const CIRCLE_SAMPLES: usize = 64;
    for step in 0..CIRCLE_SAMPLES {
        let angle = std::f32::consts::TAU * (step as f32) / (CIRCLE_SAMPLES as f32);
        angles.push(angle);
    }
    for wall in &scene.walls {
        if !wall.blocks_sight_now() {
            continue;
        }
        for point in [wall.a, wall.b] {
            let delta = point - origin;
            if delta.length() <= radius_px * 1.5 {
                angles.push(delta.y.atan2(delta.x));
            }
        }
    }

    angles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    angles.dedup_by(|a, b| (*a - *b).abs() < 1e-5);

    let mut polygon = Vec::with_capacity(angles.len() * 3);
    for angle in angles {
        // Два луча вокруг угла: так в полигон попадают «за стену» и сам угол.
        for offset in [-0.0005_f32, 0.0, 0.0005] {
            let dir = Vec2::new((angle + offset).cos(), (angle + offset).sin());
            let distance = nearest_hit(scene, origin, dir, radius_px);
            polygon.push(origin + dir * distance);
        }
    }
    polygon
}

/// Ближайшее расстояние до стены вдоль луча (или радиус, если стены нет).
fn nearest_hit(scene: &Scene, origin: Vec2, dir: Vec2, radius_px: f32) -> f32 {
    let mut nearest = radius_px;
    for wall in &scene.walls {
        if !wall.blocks_sight_now() {
            continue;
        }
        // Пересечение луча с отрезком (в параметрах луча и отрезка).
        let segment = wall.b - wall.a;
        let denom = dir.cross(segment);
        if denom.abs() < 1e-6 {
            continue;
        }
        let offset = wall.a - origin;
        let t = offset.cross(segment) / denom;
        let u = offset.cross(dir) / denom;
        if t > 0.0 && t < nearest && (0.0..=1.0).contains(&u) {
            nearest = t;
        }
    }
    nearest
}

/// Область видимости токена: полигон и клетки.
#[derive(Debug, Clone, PartialEq)]
pub struct VisionArea {
    pub polygon: Vec<Vec2>,
    pub cells: Vec<(i32, i32)>,
}

impl VisionArea {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

/// Посчитать область видимости.
#[must_use]
pub fn vision_area(scene: &Scene, origin: Vec2, radius_px: f32) -> VisionArea {
    VisionArea {
        polygon: visibility_polygon(scene, origin, radius_px),
        cells: visible_cells(scene, origin, radius_px),
    }
}

/// Радиус обзора токена в пикселях (с учётом масштаба сцены).
///
/// `range_ft <= 0` — обзора нет.
#[must_use]
pub fn vision_radius_px(grid: &Grid, range_ft: f32) -> f32 {
    if range_ft <= 0.0 {
        return 0.0;
    }
    range_ft * (grid.cell_px() / grid.feet_per_cell.max(0.001))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::Token;
    use crate::ids::EntityId;
    use crate::scene::{DoorState, Wall};

    fn empty_scene() -> Scene {
        let mut scene = Scene::new(EntityId(1), "Тест");
        scene.grid = Grid::default();
        scene
    }

    fn scene_with_wall() -> Scene {
        let mut scene = empty_scene();
        scene
            .walls
            .push(Wall::solid(Vec2::new(100.0, 0.0), Vec2::new(100.0, 500.0)));
        scene
    }

    #[test]
    fn seeing_is_blocked_by_wall() {
        let scene = scene_with_wall();
        let origin = Vec2::new(35.0, 100.0);
        assert!(can_see(&scene, origin, Vec2::new(80.0, 100.0), 500.0));
        assert!(!can_see(&scene, origin, Vec2::new(200.0, 100.0), 500.0));
    }

    #[test]
    fn radius_limits_vision() {
        let scene = empty_scene();
        let origin = Vec2::new(0.0, 0.0);
        assert!(can_see(&scene, origin, Vec2::new(50.0, 0.0), 100.0));
        assert!(!can_see(&scene, origin, Vec2::new(150.0, 0.0), 100.0));
        assert!(visible_cells(&scene, origin, 0.0).is_empty());
    }

    #[test]
    fn open_door_does_not_block_vision() {
        let mut scene = empty_scene();
        scene.walls.push(Wall::door(
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 500.0),
            DoorState::Open,
        ));
        let origin = Vec2::new(35.0, 100.0);
        assert!(can_see(&scene, origin, Vec2::new(200.0, 100.0), 500.0));

        scene.toggle_door(0);
        assert!(!can_see(&scene, origin, Vec2::new(200.0, 100.0), 500.0));
    }

    #[test]
    fn visible_cells_exclude_cells_behind_wall() {
        let scene = scene_with_wall();
        let origin = Vec2::new(35.0, 100.0);
        let cells = visible_cells(&scene, origin, 300.0);
        assert!(!cells.is_empty());
        // Слева от стены видно, справа — нет.
        assert!(cells.iter().any(|(x, _)| *x == 0));
        assert!(
            !cells.iter().any(|(x, _)| *x > 1),
            "клетки за стеной: {cells:?}"
        );
    }

    #[test]
    fn visible_cells_stay_within_radius() {
        let scene = empty_scene();
        let origin = Vec2::new(35.0, 35.0);
        let cells = visible_cells(&scene, origin, 100.0);
        for (cx, cy) in cells {
            let center = scene.grid.center_of((cx, cy));
            assert!(origin.distance(center) <= 100.0 + 1e-3);
        }
    }

    #[test]
    fn polygon_in_empty_scene_respects_radius() {
        let scene = empty_scene();
        let origin = Vec2::new(70.0, 70.0);
        let polygon = visibility_polygon(&scene, origin, 200.0);
        assert!(polygon.len() >= 60);
        for point in &polygon {
            assert!(origin.distance(*point) <= 200.0 + 1e-3);
        }
    }

    #[test]
    fn polygon_is_clipped_by_wall() {
        // Стена длинная, чтобы обойти её сверху/снизу было нельзя.
        let mut scene = empty_scene();
        scene.walls.push(Wall::solid(
            Vec2::new(100.0, -5000.0),
            Vec2::new(100.0, 5000.0),
        ));
        let origin = Vec2::new(35.0, 100.0);
        let polygon = visibility_polygon(&scene, origin, 400.0);
        assert!(!polygon.is_empty());
        assert!(
            polygon.iter().all(|point| point.x <= 100.5),
            "в полигоне есть точка за стеной"
        );
    }

    #[test]
    fn vision_area_bundles_both_views() {
        let scene = scene_with_wall();
        let area = vision_area(&scene, Vec2::new(35.0, 100.0), 200.0);
        assert!(!area.is_empty());
        assert!(!area.polygon.is_empty());
    }

    #[test]
    fn radius_conversion_uses_grid_scale() {
        let grid = Grid::default(); // 70 px на клетку, 5 футов
        assert!((vision_radius_px(&grid, 30.0) - 420.0).abs() < 1e-3);
        assert_eq!(vision_radius_px(&grid, 0.0), 0.0);
    }

    #[test]
    fn token_without_vision_sees_nothing() {
        // Токен без зрения не должен создавать область.
        let scene = empty_scene();
        let token = Token::new(EntityId(2), "Слепой", Vec2::ZERO);
        assert!(token.vision.is_none());
        assert!(vision_area(&scene, token.pos, 0.0).is_empty());
    }
}
