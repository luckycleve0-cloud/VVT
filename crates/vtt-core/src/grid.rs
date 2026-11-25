//! Сетка, привязка к клеткам и измерение дистанций.
//!
//! Разные столы меряют по-разному, и это заметная часть «ощущения» VTT:
//! 5e считает диагональ за 5 футов (Chebyshev), 3.5/PF — 5-10-5,
//! кто-то играет по линейке. Поэтому способ измерения — параметр сцены,
//! а не константа в коде.

use crate::geom::Vec2;
use serde::{Deserialize, Serialize};

/// Тип сетки.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridKind {
    #[default]
    Square,
    /// Гексы острым углом вверх.
    HexPointy,
    /// Гексы плоской стороной вверх.
    HexFlat,
    /// Без сетки (только линейка).
    None,
}

/// Как считать расстояние между клетками.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Measurement {
    /// 5e: диагональ = 1 клетка.
    #[default]
    Chebyshev,
    /// Линейка в пикселях (тоже клетки, но дробные).
    Euclidean,
    /// 3.5e / Pathfinder: 5-10-5 (диагональ ≈ 1.5 клетки).
    Alternating,
    /// Манхэттен — для «клеточных» игр.
    Manhattan,
    /// Для гексов: расстояние по осям.
    Hex,
}

/// Параметры сетки сцены.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub kind: GridKind,
    /// Размер клетки в пикселях.
    pub cell_px: f32,
    /// Смещение сетки относительно угла карты.
    pub offset: Vec2,
    /// Сколько футов в одной клетке.
    pub feet_per_cell: f32,
    pub measurement: Measurement,
}

impl Default for Grid {
    fn default() -> Self {
        Self {
            kind: GridKind::Square,
            cell_px: 70.0,
            offset: Vec2::ZERO,
            feet_per_cell: 5.0,
            measurement: Measurement::Chebyshev,
        }
    }
}

impl Grid {
    #[must_use]
    pub fn new(kind: GridKind, cell_px: f32) -> Self {
        Self {
            kind,
            cell_px,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_measurement(mut self, measurement: Measurement) -> Self {
        self.measurement = measurement;
        self
    }

    #[must_use]
    pub fn cell_px(&self) -> f32 {
        if self.cell_px <= 0.0 {
            70.0
        } else {
            self.cell_px
        }
    }

    /// Координаты клетки, в которой лежит точка.
    #[must_use]
    pub fn cell_of(&self, p: Vec2) -> (i32, i32) {
        let cell = self.cell_px();
        match self.kind {
            GridKind::Square | GridKind::None => (
                ((p.x - self.offset.x) / cell).floor() as i32,
                ((p.y - self.offset.y) / cell).floor() as i32,
            ),
            GridKind::HexPointy | GridKind::HexFlat => {
                let (q, r) = self.pixel_to_axial(p);
                round_axial(q, r)
            }
        }
    }

    /// Центр клетки в мировых координатах.
    #[must_use]
    pub fn center_of(&self, cell: (i32, i32)) -> Vec2 {
        let cell_px = self.cell_px();
        match self.kind {
            GridKind::Square | GridKind::None => Vec2::new(
                self.offset.x + (cell.0 as f32 + 0.5) * cell_px,
                self.offset.y + (cell.1 as f32 + 0.5) * cell_px,
            ),
            GridKind::HexPointy | GridKind::HexFlat => self.axial_to_pixel(cell.0, cell.1),
        }
    }

    /// Привязать точку к центру клетки (если сетка включена).
    #[must_use]
    pub fn snap(&self, p: Vec2) -> Vec2 {
        if self.kind == GridKind::None {
            return p;
        }
        self.center_of(self.cell_of(p))
    }

    /// Привязать, только если до центра меньше `tolerance` (для свободного перетаскивания).
    #[must_use]
    pub fn snap_with_tolerance(&self, p: Vec2, tolerance: f32) -> Vec2 {
        let center = self.snap(p);
        if center.distance(p) <= tolerance {
            center
        } else {
            p
        }
    }

    /// Расстояние в клетках между двумя точками.
    #[must_use]
    pub fn cells_between(&self, a: Vec2, b: Vec2) -> f32 {
        let cell = self.cell_px();
        match self.kind {
            GridKind::HexPointy | GridKind::HexFlat => {
                let (qa, ra) = self.pixel_to_axial(a);
                let (qb, rb) = self.pixel_to_axial(b);
                let (ca, cb) = (round_axial(qa, ra), round_axial(qb, rb));
                hex_distance(ca, cb)
            }
            _ => {
                let dx = (b.x - a.x).abs() / cell;
                let dy = (b.y - a.y).abs() / cell;
                match self.measurement {
                    Measurement::Chebyshev => dx.max(dy),
                    Measurement::Euclidean => (dx * dx + dy * dy).sqrt(),
                    Measurement::Alternating => {
                        let diagonals = dx.min(dy);
                        let straights = dx.max(dy) - diagonals;
                        straights + diagonals * 1.5
                    }
                    Measurement::Manhattan => dx + dy,
                    Measurement::Hex => dx.max(dy),
                }
            }
        }
    }

    /// Расстояние в футах.
    #[must_use]
    pub fn distance_ft(&self, a: Vec2, b: Vec2) -> f32 {
        self.cells_between(a, b) * self.feet_per_cell
    }

    // --- гексы -------------------------------------------------------------

    fn hex_size(&self) -> f32 {
        // Радиус описанной окружности гекса.
        self.cell_px() / 2.0
    }

    fn pixel_to_axial(&self, p: Vec2) -> (f32, f32) {
        let size = self.hex_size();
        let x = p.x - self.offset.x;
        let y = p.y - self.offset.y;
        match self.kind {
            // Pointy: ось Y вниз.
            GridKind::HexFlat => {
                let q = (x * 3.0 / 4.0) / size;
                let r = (-x / 4.0 + (3.0_f32).sqrt() / 2.0 * y) / size;
                (q, r)
            }
            _ => {
                let q = ((3.0_f32).sqrt() / 3.0 * x - y / 3.0) / size;
                let r = (2.0 / 3.0 * y) / size;
                (q, r)
            }
        }
    }

    fn axial_to_pixel(&self, q: i32, r: i32) -> Vec2 {
        let size = self.hex_size();
        let (q, r) = (q as f32, r as f32);
        match self.kind {
            GridKind::HexFlat => Vec2::new(
                self.offset.x + size * (3.0 / 2.0 * q),
                self.offset.y + size * ((3.0_f32).sqrt() / 2.0 * q + (3.0_f32).sqrt() * r),
            ),
            _ => Vec2::new(
                self.offset.x + size * ((3.0_f32).sqrt() * (q + r / 2.0)),
                self.offset.y + size * (3.0 / 2.0 * r),
            ),
        }
    }
}

/// Округление дробных осевых координат до ближайшего гекса.
fn round_axial(q: f32, r: f32) -> (i32, i32) {
    let x = q;
    let z = r;
    let y = -x - z;
    let (mut rx, mut ry, mut rz) = (x.round(), y.round(), z.round());
    let (dx, dy, dz) = ((rx - x).abs(), (ry - y).abs(), (rz - z).abs());
    if dx > dy && dx > dz {
        rx = -ry - rz;
    } else if dy > dz {
        ry = -rx - rz;
    } else {
        rz = -rx - ry;
    }
    // Кубические координаты обязаны в сумме давать ноль — проверка ловит
    // ошибки округления ещё в отладочных сборках.
    debug_assert!(
        (rx + ry + rz).abs() < 0.001,
        "нарушена согласованность кубических координат"
    );
    (rx as i32, rz as i32)
}

fn hex_distance(a: (i32, i32), b: (i32, i32)) -> f32 {
    let dq = (a.0 - b.0) as f32;
    let dr = (a.1 - b.1) as f32;
    (dq.abs() + (dq + dr).abs() + dr.abs()) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        Grid::new(GridKind::Square, 70.0)
    }

    #[test]
    fn cell_and_center_roundtrip() {
        let g = grid();
        for cell in [(0, 0), (1, 2), (-3, 5)] {
            assert_eq!(g.cell_of(g.center_of(cell)), cell);
        }
    }

    #[test]
    fn snap_lands_on_centers() {
        let g = grid();
        let snapped = g.snap(Vec2::new(10.0, 10.0));
        assert_eq!(snapped, Vec2::new(35.0, 35.0));
    }

    #[test]
    fn measurements_differ_as_expected() {
        let a = Vec2::new(35.0, 35.0);
        let b = Vec2::new(35.0 + 140.0, 35.0 + 210.0); // (2, 3) клетки
        let base = grid();

        assert_eq!(base.cells_between(a, b), 3.0); // Chebyshev
        assert_eq!(
            base.with_measurement(Measurement::Manhattan)
                .cells_between(a, b),
            5.0
        );
        let euclid = base
            .with_measurement(Measurement::Euclidean)
            .cells_between(a, b);
        assert!((euclid - 13.0_f32.sqrt()).abs() < 1e-4);
        let alternating = base
            .with_measurement(Measurement::Alternating)
            .cells_between(a, b);
        // dx = 2, dy = 3 → 1 прямая + 2 диагонали × 1.5 = 4.0.
        assert!((alternating - 4.0).abs() < 1e-4);
    }

    #[test]
    fn feet_follow_scale() {
        let g = grid();
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(140.0, 0.0); // 2 клетки
        assert_eq!(g.distance_ft(a, b), 10.0);
    }

    #[test]
    fn hex_grid_snaps_and_measures() {
        let g = Grid::new(GridKind::HexPointy, 70.0);
        let center = g.snap(Vec2::new(30.0, 30.0));
        assert_eq!(g.cell_of(center), g.cell_of(Vec2::new(30.0, 30.0)));
        // Соседний гекс находится ровно в одной клетке.
        let neighbor = g.center_of((1, 0));
        assert!((g.cells_between(center, neighbor) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn gridless_keeps_position() {
        let g = Grid::new(GridKind::None, 70.0);
        let p = Vec2::new(12.3, 45.6);
        assert_eq!(g.snap(p), p);
    }
}
