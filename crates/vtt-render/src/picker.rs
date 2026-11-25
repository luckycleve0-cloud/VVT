//! Мини-проводник для выбора картинки токена.
//!
//! Нативные диалоги требуют отдельной зависимости и прав, поэтому здесь простой
//! список файлов: папки кликабельны, картинки png/jpg выбираются мышью.

use std::path::{Path, PathBuf};

use macroquad::prelude::*;

use vtt_core::geom::Vec2;

use crate::draw::{rgba, text, text_width, Assets};

/// Расширения, которые умеет читать `macroquad` через `image`.
const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "bmp"];

/// Состояние проводника.
#[derive(Debug, Clone)]
pub struct PickerState {
    pub open: bool,
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub scroll: f32,
}

/// Строка списка.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

impl Default for PickerState {
    fn default() -> Self {
        let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut picker = Self {
            open: false,
            dir,
            entries: Vec::new(),
            scroll: 0.0,
        };
        picker.refresh();
        picker
    }
}

impl PickerState {
    /// Открыть проводник в текущей папке.
    pub fn show(&mut self) {
        self.open = true;
        self.refresh();
    }

    pub fn hide(&mut self) {
        self.open = false;
    }

    /// Перечитать содержимое папки.
    pub fn refresh(&mut self) {
        let mut entries = Vec::new();

        if let Some(parent) = self.dir.parent() {
            entries.push(Entry {
                name: ".. (наверх)".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }

        if let Ok(read_dir) = std::fs::read_dir(&self.dir) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            for item in read_dir.flatten() {
                let path = item.path();
                let name = item.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                if path.is_dir() {
                    dirs.push(Entry {
                        name,
                        path,
                        is_dir: true,
                    });
                } else if is_image(&path) {
                    files.push(Entry {
                        name,
                        path,
                        is_dir: false,
                    });
                }
            }
            dirs.sort_by_key(|entry| entry.name.to_lowercase());
            files.sort_by_key(|entry| entry.name.to_lowercase());
            entries.extend(dirs);
            entries.extend(files);
        }

        self.entries = entries;
        self.scroll = 0.0;
    }

    pub fn enter(&mut self, path: &Path) {
        self.dir = path.to_path_buf();
        self.refresh();
    }
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Нарисовать проводник. Возвращает выбранный файл, если пользователь кликнул.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    assets: &Assets,
    viewport: Vec2,
    mouse: Vec2,
    clicked: bool,
    wheel: f32,
    picker: &mut PickerState,
) -> Option<PathBuf> {
    if !picker.open {
        return None;
    }

    draw_rectangle(0.0, 0.0, viewport.x, viewport.y, rgba(0, 0, 0, 190));

    let width = 720.0_f32.min(viewport.x - 40.0);
    let height = 560.0_f32.min(viewport.y - 40.0);
    let x = (viewport.x - width) * 0.5;
    let y = (viewport.y - height) * 0.5;
    draw_rectangle(x, y, width, height, rgba(22, 24, 30, 250));
    draw_rectangle_lines(x, y, width, height, 2.0, rgba(90, 100, 118, 255));

    text(
        assets,
        "Выбор картинки токена",
        x + 18.0,
        y + 32.0,
        22.0,
        rgba(240, 243, 250, 255),
    );
    let mut path_text = picker.dir.display().to_string();
    while text_width(assets, &path_text, 14.0) > width - 120.0 && path_text.len() > 10 {
        path_text.truncate(path_text.len() - 2);
    }
    text(
        assets,
        &path_text,
        x + 18.0,
        y + 56.0,
        14.0,
        rgba(150, 158, 174, 255),
    );

    // Кнопка закрытия.
    let close = Rect::new(x + width - 46.0, y + 10.0, 34.0, 30.0);
    let close_hover = close.contains(mouse);
    draw_rectangle(
        close.x,
        close.y,
        close.w,
        close.h,
        if close_hover {
            rgba(120, 60, 60, 255)
        } else {
            rgba(50, 40, 44, 255)
        },
    );
    text(
        assets,
        "×",
        close.x + 11.0,
        close.y + 22.0,
        20.0,
        rgba(240, 220, 220, 255),
    );
    if clicked && close_hover {
        picker.hide();
        return None;
    }

    let list_top = y + 74.0;
    let list_bottom = y + height - 20.0;
    let row_height = 26.0;
    let list_rect = Rect::new(x + 12.0, list_top, width - 24.0, list_bottom - list_top);
    let inside = list_rect.contains(mouse);

    let total = picker.entries.len() as f32 * row_height;
    let max_scroll = (total - list_rect.h).max(0.0);
    if inside && wheel.abs() > 0.0 {
        picker.scroll = (picker.scroll + wheel * 40.0).clamp(0.0, max_scroll);
    }
    picker.scroll = picker.scroll.clamp(0.0, max_scroll);

    let mut chosen = None;
    let mut navigate: Option<PathBuf> = None;
    let mut row_y = list_top - picker.scroll;
    for entry in &picker.entries {
        if row_y + row_height >= list_top && row_y <= list_bottom {
            let row = Rect::new(list_rect.x, row_y, list_rect.w, row_height);
            let hovered = inside && row.contains(mouse);
            if hovered {
                draw_rectangle(row.x, row.y, row.w, row.h, rgba(40, 46, 58, 255));
            }
            let prefix = if entry.is_dir { "▸ " } else { "  " };
            let label = format!("{prefix}{}", entry.name);
            text(
                assets,
                &label,
                row.x + 10.0,
                row.y + 18.0,
                16.0,
                if entry.is_dir {
                    rgba(220, 210, 160, 255)
                } else {
                    rgba(220, 226, 238, 255)
                },
            );
            if clicked && hovered {
                if entry.is_dir {
                    navigate = Some(entry.path.clone());
                } else {
                    chosen = Some(entry.path.clone());
                }
            }
        }
        row_y += row_height;
    }

    if let Some(path) = navigate {
        picker.enter(&path);
        return None;
    }

    if picker.entries.is_empty() {
        text(
            assets,
            "В папке нет картинок png/jpg — поднимитесь выше",
            list_rect.x + 10.0,
            list_top + 24.0,
            16.0,
            rgba(170, 176, 190, 255),
        );
    }

    if let Some(path) = &chosen {
        picker.hide();
        return Some(path.clone());
    }
    None
}

/// Простой прямоугольник (чтобы не зависеть от типов macroquad).
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
