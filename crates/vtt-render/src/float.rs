//! Плавающие окна: перетаскивание за заголовок и ресайз за края/углы.
//!
//! Содержимое окна рисуется в собственный `RenderTarget` (размер содержимого),
//! затем вписывается в окно с масштабированием — поэтому окно можно свободно
//! тянуть и менять размер, а содержимое не вылезает за рамки.

use macroquad::prelude::Camera2D;
use macroquad::prelude::*;
use macroquad::texture::RenderTarget;

use vtt_core::geom::Vec2;

use crate::draw::{rgba, text, Assets};

const TITLE: f32 = 26.0;
const BORDER: f32 = 4.0;
const HANDLE: f32 = 6.0;

/// Направление ресайза.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resize {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Плавающее окно.
pub struct FloatWindow {
    /// Положение и размер окна на логическом экране.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Размер содержимого (рисуется в отдельный таргет).
    pub content: Vec2,
    /// Минимальный размер окна.
    min: Vec2,
    title: &'static str,
    target: Option<RenderTarget>,
    drag: Option<Vec2>,
    resize: Option<Resize>,
}

impl std::fmt::Debug for FloatWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FloatWindow({}x{} at {}, {})",
            self.w, self.h, self.x, self.y
        )
    }
}

impl FloatWindow {
    /// Создать окно по центру экрана.
    #[must_use]
    pub fn centered(viewport: Vec2, content: Vec2, title: &'static str) -> Self {
        let w = (content.x + BORDER * 2.0).min(viewport.x - 40.0);
        let h = (content.y + TITLE + BORDER * 2.0).min(viewport.y - 40.0);
        Self {
            x: ((viewport.x - w) * 0.5).max(10.0),
            y: ((viewport.y - h) * 0.5).max(10.0),
            w,
            h,
            content,
            min: Vec2::new((content.x * 0.5).max(220.0), (content.y * 0.4).max(140.0)),
            title,
            target: None,
            drag: None,
            resize: None,
        }
    }

    /// Окно, прижатое к правому нижнему углу (для панели токена).
    #[must_use]
    pub fn bottom_right(viewport: Vec2, content: Vec2, title: &'static str) -> Self {
        let w = content.x + BORDER * 2.0;
        let h = content.y + TITLE + BORDER * 2.0;
        Self {
            x: viewport.x - w - 20.0,
            y: viewport.y - h - 70.0,
            w,
            h,
            content,
            min: Vec2::new(content.x * 0.60, content.y * 0.60),
            title,
            target: None,
            drag: None,
            resize: None,
        }
    }

    /// Внутренняя область (без рамки и заголовка).
    #[must_use]
    pub fn inner(&self) -> (f32, f32, f32, f32) {
        (
            self.x + BORDER,
            self.y + TITLE,
            self.w - BORDER * 2.0,
            self.h - TITLE - BORDER,
        )
    }

    /// Маус в координатах содержимого.
    #[must_use]
    pub fn content_mouse(&self, mouse: Vec2) -> Vec2 {
        let (ix, iy, iw, ih) = self.inner();
        let scale = (iw / self.content.x).min(ih / self.content.y);
        let ox = ix + (iw - self.content.x * scale) * 0.5;
        let oy = iy + (ih - self.content.y * scale) * 0.5;
        (mouse - Vec2::new(ox, oy)) / scale
    }

    /// Масштаб содержимого.
    #[must_use]
    pub fn scale(&self) -> f32 {
        let (_, _, iw, ih) = self.inner();
        (iw / self.content.x).min(ih / self.content.y)
    }

    /// Размер содержимого.
    #[must_use]
    pub fn content_size(&self) -> Vec2 {
        self.content
    }

    /// Точка в области содержимого (не заголовок и не ручки ресайза).
    #[must_use]
    pub fn is_content_point(&self, mouse: Vec2) -> bool {
        if !self.contains(mouse) {
            return false;
        }
        if mouse.y <= self.y + TITLE + 2.0 {
            return false;
        }
        self.resize_at(mouse).is_none()
    }

    /// Попала ли точка в окно (включая рамку/заголовок).
    #[must_use]
    pub fn contains(&self, mouse: Vec2) -> bool {
        mouse.x >= self.x
            && mouse.x <= self.x + self.w
            && mouse.y >= self.y
            && mouse.y <= self.y + self.h
    }

    fn resize_at(&self, mouse: Vec2) -> Option<Resize> {
        let left = (mouse.x - self.x).abs() <= HANDLE;
        let right = (mouse.x - (self.x + self.w)).abs() <= HANDLE;
        let top = (mouse.y - self.y).abs() <= HANDLE;
        let bottom = (mouse.y - (self.y + self.h)).abs() <= HANDLE;
        let in_x = mouse.x >= self.x - HANDLE && mouse.x <= self.x + self.w + HANDLE;
        let in_y = mouse.y >= self.y - HANDLE && mouse.y <= self.y + self.h + HANDLE;
        match (left && top, right && top, left && bottom, right && bottom) {
            (true, _, _, _) if in_y => Some(Resize::TopLeft),
            (_, true, _, _) if in_y => Some(Resize::TopRight),
            (_, _, true, _) if in_y => Some(Resize::BottomLeft),
            (_, _, _, true) if in_y => Some(Resize::BottomRight),
            _ if left && in_y => Some(Resize::Left),
            _ if right && in_y => Some(Resize::Right),
            _ if top && in_x => Some(Resize::Top),
            _ if bottom && in_x => Some(Resize::Bottom),
            _ => None,
        }
    }

    /// Обработать мышь (перетаскивание/ресайз). Возвращает `true`, если окно
    /// забирает ввод (курсор над окном).
    pub fn update(&mut self, mouse: Vec2, pressed: bool, down: bool, viewport: Vec2) -> bool {
        if pressed {
            if let Some(direction) = self.resize_at(mouse) {
                self.resize = Some(direction);
            } else if mouse.y >= self.y
                && mouse.y <= self.y + TITLE
                && mouse.x >= self.x
                && mouse.x <= self.x + self.w
            {
                self.drag = Some(Vec2::new(mouse.x - self.x, mouse.y - self.y));
            }
        }

        if !down {
            self.drag = None;
            self.resize = None;
        }

        if let Some(offset) = self.drag {
            self.x = (mouse.x - offset.x).clamp(0.0, (viewport.x - self.w).max(0.0));
            self.y = (mouse.y - offset.y).clamp(0.0, (viewport.y - self.h).max(0.0));
        }

        if let Some(direction) = self.resize {
            let min = self.min;
            match direction {
                Resize::Left => {
                    let right = self.x + self.w;
                    self.x = mouse.x.clamp(0.0, right - min.x);
                    self.w = right - self.x;
                }
                Resize::Right => {
                    self.w = (mouse.x - self.x).clamp(min.x, viewport.x - self.x);
                }
                Resize::Top => {
                    let bottom = self.y + self.h;
                    self.y = mouse.y.clamp(0.0, bottom - min.y);
                    self.h = bottom - self.y;
                }
                Resize::Bottom => {
                    self.h = (mouse.y - self.y).clamp(min.y, viewport.y - self.y);
                }
                Resize::TopLeft => {
                    let right = self.x + self.w;
                    let bottom = self.y + self.h;
                    self.x = mouse.x.clamp(0.0, right - min.x);
                    self.y = mouse.y.clamp(0.0, bottom - min.y);
                    self.w = right - self.x;
                    self.h = bottom - self.y;
                }
                Resize::TopRight => {
                    let bottom = self.y + self.h;
                    self.w = (mouse.x - self.x).clamp(min.x, viewport.x - self.x);
                    self.y = mouse.y.clamp(0.0, bottom - min.y);
                    self.h = bottom - self.y;
                }
                Resize::BottomLeft => {
                    let right = self.x + self.w;
                    self.x = mouse.x.clamp(0.0, right - min.x);
                    self.w = right - self.x;
                    self.h = (mouse.y - self.y).clamp(min.y, viewport.y - self.y);
                }
                Resize::BottomRight => {
                    self.w = (mouse.x - self.x).clamp(min.x, viewport.x - self.x);
                    self.h = (mouse.y - self.y).clamp(min.y, viewport.y - self.y);
                }
            }
        }

        self.contains(mouse)
    }

    /// Нарисовать рамку, заголовок и содержимое.
    ///
    /// Возвращает мышь в координатах содержимого и признак клика по содержимому.
    pub fn begin_content(&mut self, assets: &Assets, mouse: Vec2) -> (Vec2, bool) {
        // Рамка окна.
        draw_rectangle(self.x, self.y, self.w, self.h, rgba(18, 20, 26, 248));
        draw_rectangle_lines(self.x, self.y, self.w, self.h, 1.5, rgba(96, 108, 128, 255));
        // Заголовок.
        draw_rectangle(
            self.x + 1.0,
            self.y + 1.0,
            self.w - 2.0,
            TITLE - 2.0,
            rgba(34, 40, 52, 255),
        );
        text(
            assets,
            self.title,
            self.x + 14.0,
            self.y + 18.0,
            14.0,
            rgba(228, 234, 244, 255),
        );
        draw_line(
            self.x + 10.0,
            self.y + TITLE - 1.0,
            self.x + self.w - 10.0,
            self.y + TITLE - 1.0,
            2.0,
            rgba(232, 192, 90, 210),
        );
        // Уголок ресайза.
        for index in 0..3 {
            let offset = index as f32 * 4.0;
            draw_line(
                self.x + self.w - 10.0 + offset,
                self.y + self.h - 4.0,
                self.x + self.w - 4.0,
                self.y + self.h - 10.0 + offset,
                1.0,
                rgba(120, 132, 150, 255),
            );
        }

        // Содержимое рисуется в свой render target и затем вписывается.
        let target = self.target.get_or_insert_with(|| {
            let target = render_target(
                self.content.x.max(100.0) as u32,
                self.content.y.max(100.0) as u32,
            );
            target.texture.set_filter(FilterMode::Linear);
            target
        });
        let mut camera =
            Camera2D::from_display_rect(Rect::new(0.0, 0.0, self.content.x, self.content.y));
        camera.render_target = Some(target.clone());
        set_camera(&camera);

        let content_mouse = self.content_mouse(mouse);
        (content_mouse, true)
    }

    /// Закончить содержимое: вернуть камеру UI и нарисовать таргет в окно.
    pub fn end_content(&mut self, ui_cam: &Camera2D, mouse: Vec2) {
        let (ix, iy, iw, ih) = self.inner();
        let scale = self.scale();
        let draw_w = self.content.x * scale;
        let draw_h = self.content.y * scale;
        let ox = ix + (iw - draw_w) * 0.5;
        let oy = iy + (ih - draw_h) * 0.5;

        set_camera(ui_cam);
        if let Some(target) = &self.target {
            draw_texture_ex(
                &target.texture,
                ox,
                oy,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(draw_w, draw_h)),
                    flip_y: true,
                    ..Default::default()
                },
            );
        }
        let _ = mouse;
    }
}
