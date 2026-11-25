//! # vtt-render — прототип окна
//!
//! Этот крейт показывает стол на экране: карту, сетку, токены, линейку,
//! шаблоны областей и ручной туман.
//!
//! **Важно:** это прототип на `macroquad`, а не финальный рендерер. Цель —
//! быстро увидеть данные в окне и отладить взаимодействие (камера, попадание
//! мышью, перетаскивание), не тратя недели на `wgpu`-пайплайн.
//! Финальный рендерер (wgpu + egui) заменит `draw.rs`, сохранив `camera`,
//! `interact` и весь остальной код: они не зависят от графической библиотеки.
//! См. `docs/DECISIONS.md`, ADR-009.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod camera;
pub mod draw;
pub mod float;
pub mod interact;
pub mod keys;
pub mod panels;
pub mod picker;
pub mod roster;
pub mod session;
pub mod sheet;
pub mod toolbar;
pub mod wizard;
