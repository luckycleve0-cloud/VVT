//! Минимальная 2D-геометрия.
//!
//! Своя пара `Vec2` вместо `glam`/`nalgebra`: нам нужны только точка,
//! вектор и пара операций. Это экономит зависимость и время сборки.

use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Точка или вектор в мировых координатах сцены (в пикселях).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    #[must_use]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Псевдоскалярное произведение (z-компонента векторного).
    #[must_use]
    pub fn cross(self, other: Self) -> f32 {
        self.x * other.y - self.y * other.x
    }

    #[must_use]
    pub fn distance(self, other: Self) -> f32 {
        (self - other).length()
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            Self::ZERO
        } else {
            self * (1.0 / len)
        }
    }

    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }

    #[must_use]
    pub fn perp(self) -> Self {
        Self::new(-self.y, self.x)
    }
}

impl Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;
    fn div(self, rhs: f32) -> Self {
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl Neg for Vec2 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

/// Пересекаются ли отрезки `a1a2` и `b1b2` (включая касание концами).
///
/// Используется для проверки линии видимости через стены.
#[must_use]
pub fn segments_intersect(a1: Vec2, a2: Vec2, b1: Vec2, b2: Vec2) -> bool {
    let d1 = (a2 - a1).cross(b1 - a1);
    let d2 = (a2 - a1).cross(b2 - a1);
    let d3 = (b2 - b1).cross(a1 - b1);
    let d4 = (b2 - b1).cross(a2 - b1);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }

    // Коллинеарные/касающиеся случаи.
    (d1.abs() < f32::EPSILON && on_segment(a1, a2, b1))
        || (d2.abs() < f32::EPSILON && on_segment(a1, a2, b2))
        || (d3.abs() < f32::EPSILON && on_segment(b1, b2, a1))
        || (d4.abs() < f32::EPSILON && on_segment(b1, b2, a2))
}

fn on_segment(a: Vec2, b: Vec2, p: Vec2) -> bool {
    p.x >= a.x.min(b.x) - f32::EPSILON
        && p.x <= a.x.max(b.x) + f32::EPSILON
        && p.y >= a.y.min(b.y) - f32::EPSILON
        && p.y <= a.y.max(b.y) + f32::EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_math() {
        let a = Vec2::new(3.0, 4.0);
        assert!((a.length() - 5.0).abs() < 1e-6);
        assert!((a.normalized().length() - 1.0).abs() < 1e-6);
        assert_eq!(
            Vec2::new(1.0, 2.0) + Vec2::new(2.0, 3.0),
            Vec2::new(3.0, 5.0)
        );
    }

    #[test]
    fn crossing_segments() {
        // Крест.
        assert!(segments_intersect(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 0.0)
        ));
        // Параллельные — нет.
        assert!(!segments_intersect(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 5.0),
            Vec2::new(10.0, 5.0)
        ));
        // Т-образное касание — да.
        assert!(segments_intersect(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(5.0, 0.0),
            Vec2::new(5.0, 5.0)
        ));
    }
}
