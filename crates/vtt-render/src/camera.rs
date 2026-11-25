//! Камера: перевод мира в экран, зум к курсору, панорамирование.
//!
//! Намеренно не зависит от графической библиотеки: это чистая математика,
//! поэтому её можно покрыть тестами без окна. Финальный рендерер (wgpu)
//! использует тот же код.

use vtt_core::geom::Vec2;

/// Минимальный зум (1 пиксель мира = 0.1 пикселя экрана).
pub const MIN_ZOOM: f32 = 0.1;
/// Максимальный зум.
pub const MAX_ZOOM: f32 = 4.0;

/// Состояние камеры.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Мировая точка, которая находится в центре экрана.
    pub center: Vec2,
    /// Пикселей мира на пиксель экрана (1.0 — один к одному).
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl Camera {
    #[must_use]
    pub fn new(center: Vec2, zoom: f32) -> Self {
        Self {
            center,
            zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
        }
    }

    /// Мировые координаты → экранные (пиксели окна).
    #[must_use]
    pub fn world_to_screen(&self, world: Vec2, viewport: Vec2) -> Vec2 {
        (world - self.center) * self.zoom + viewport * 0.5
    }

    /// Экранные координаты → мировые.
    #[must_use]
    pub fn screen_to_world(&self, screen: Vec2, viewport: Vec2) -> Vec2 {
        (screen - viewport * 0.5) / self.zoom + self.center
    }

    /// Сдвинуть камеру перетаскиванием (аргумент — дельта в пикселях экрана).
    pub fn pan(&mut self, screen_delta: Vec2) {
        let delta = screen_delta / self.zoom;
        self.center = self.center - delta;
    }

    /// Зум, при котором точка под курсором остаётся на месте.
    pub fn zoom_at(&mut self, screen_point: Vec2, viewport: Vec2, factor: f32) {
        let before = self.screen_to_world(screen_point, viewport);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(screen_point, viewport);
        self.center = self.center + (before - after);
    }

    /// Показать весь мир с отступом.
    pub fn fit(&mut self, world_size: Vec2, viewport: Vec2, margin: f32) {
        if world_size.x <= 0.0 || world_size.y <= 0.0 {
            return;
        }
        let available_x = (viewport.x - margin * 2.0).max(32.0);
        let available_y = (viewport.y - margin * 2.0).max(32.0);
        let zoom = (available_x / world_size.x).min(available_y / world_size.y);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = world_size * 0.5;
    }

    /// Видимый прямоугольник мира (нужен для отсечения отрисовки).
    #[must_use]
    pub fn visible_world_rect(&self, viewport: Vec2) -> (Vec2, Vec2) {
        let half = viewport * 0.5 / self.zoom;
        (self.center - half, self.center + half)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Vec2 = Vec2::new(1280.0, 800.0);

    #[test]
    fn roundtrip_world_screen() {
        let camera = Camera::new(Vec2::new(500.0, 300.0), 1.5);
        for point in [Vec2::ZERO, Vec2::new(100.0, 200.0), Vec2::new(-50.0, 700.0)] {
            let screen = camera.world_to_screen(point, VIEWPORT);
            let back = camera.screen_to_world(screen, VIEWPORT);
            assert!((back.x - point.x).abs() < 1e-3);
            assert!((back.y - point.y).abs() < 1e-3);
        }
    }

    #[test]
    fn center_maps_to_screen_center() {
        let camera = Camera::new(Vec2::new(300.0, 400.0), 2.0);
        let screen = camera.world_to_screen(camera.center, VIEWPORT);
        assert!((screen.x - VIEWPORT.x / 2.0).abs() < 1e-3);
        assert!((screen.y - VIEWPORT.y / 2.0).abs() < 1e-3);
    }

    #[test]
    fn zoom_keeps_point_under_cursor() {
        let mut camera = Camera::new(Vec2::new(200.0, 200.0), 1.0);
        let cursor = Vec2::new(900.0, 600.0);
        let world_before = camera.screen_to_world(cursor, VIEWPORT);

        camera.zoom_at(cursor, VIEWPORT, 1.8);
        let world_after = camera.screen_to_world(cursor, VIEWPORT);

        assert!(
            (world_before.x - world_after.x).abs() < 1e-2,
            "{world_before:?} vs {world_after:?}"
        );
        assert!((world_before.y - world_after.y).abs() < 1e-2);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut camera = Camera::default();
        for _ in 0..100 {
            camera.zoom_at(VIEWPORT * 0.5, VIEWPORT, 2.0);
        }
        assert!(camera.zoom <= MAX_ZOOM + 1e-6);
        for _ in 0..100 {
            camera.zoom_at(VIEWPORT * 0.5, VIEWPORT, 0.5);
        }
        assert!(camera.zoom >= MIN_ZOOM - 1e-6);
    }

    #[test]
    fn pan_moves_in_screen_space() {
        let mut camera = Camera::new(Vec2::ZERO, 2.0);
        // Сдвиг мышью на 100px вправо → мир уезжает влево на 50.
        camera.pan(Vec2::new(100.0, 0.0));
        assert!((camera.center.x + 50.0).abs() < 1e-4);
    }

    #[test]
    fn fit_shows_whole_map() {
        let mut camera = Camera::default();
        camera.fit(Vec2::new(4000.0, 3000.0), VIEWPORT, 40.0);
        let (min, max) = camera.visible_world_rect(VIEWPORT);
        assert!(min.x <= 0.0 && min.y <= 0.0);
        assert!(max.x >= 4000.0 && max.y >= 3000.0);
    }
}
