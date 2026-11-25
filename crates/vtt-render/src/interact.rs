//! Взаимодействие мышью: выбор инструмента, перетаскивание, линейка, туман.
//!
//! Здесь нет ни одной функции рисования — только состояние и чистые решения
//! «что произошло». Благодаря этому логику можно тестировать без окна, а
//! рисование (macroquad сейчас, wgpu потом) остаётся тонким слоем.

use vtt_core::geom::Vec2;
use vtt_core::grid::Grid;
use vtt_core::ids::EntityId;
use vtt_core::scene::Scene;

/// Активный инструмент.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Выбор и перетаскивание токенов.
    #[default]
    Select,
    /// Линейка: расстояние в футах.
    Measure,
    /// Круг тумана (закрыть область).
    Fog,
    /// Пинг — «смотрите сюда».
    Ping,
    /// Рисование стены (протянуть мышью).
    Wall,
    /// Открыть/закрыть дверь под курсором.
    Door,
}

impl Tool {
    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Select => "выбор",
            Self::Measure => "линейка",
            Self::Fog => "туман",
            Self::Ping => "пинг",
            Self::Wall => "стена",
            Self::Door => "дверь",
        }
    }
}

/// Что игра должна сделать по итогам жеста.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Передвинуть токен (мировые координаты, ещё не привязанные к сетке).
    MoveToken { token: EntityId, to: Vec2 },
    /// Показать пинг всем.
    Ping { at: Vec2 },
    /// Закрыть круг тумана.
    Fog { center: Vec2, radius: f32 },
    /// Добавить стену или дверь.
    AddWall { a: Vec2, b: Vec2, door: bool },
    /// Открыть/закрыть дверь рядом с точкой.
    ToggleDoorNear { at: Vec2 },
}

/// Состояние мыши.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pointer {
    pub world: Vec2,
    pub left_down: bool,
    pub right_down: bool,
}

/// Текущее перетаскивание токена.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drag {
    pub token: EntityId,
    /// Смещение от центра токена до точки захвата.
    pub grab_offset: Vec2,
}

/// Машина взаимодействия.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Interaction {
    pub tool: Tool,
    drag: Option<Drag>,
    measure_from: Option<Vec2>,
    pub measure_to: Option<Vec2>,
    pub ping_at: Option<Vec2>,
    pub fog_radius: f32,
    wall_start: Option<Vec2>,
}

impl Interaction {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool: Tool::Select,
            drag: None,
            measure_from: None,
            measure_to: None,
            ping_at: None,
            fog_radius: 60.0,
            wall_start: None,
        }
    }

    #[must_use]
    pub fn drag(&self) -> Option<Drag> {
        self.drag
    }

    /// Нажатие левой кнопкой.
    pub fn press(&mut self, pointer: Pointer, scene: &Scene) -> Option<Action> {
        match self.tool {
            Tool::Select => {
                if let Some(token) = scene.token_at(pointer.world) {
                    self.drag = Some(Drag {
                        token: token.id,
                        grab_offset: pointer.world - token.pos,
                    });
                }
                None
            }
            Tool::Measure => {
                self.measure_from = Some(pointer.world);
                self.measure_to = Some(pointer.world);
                None
            }
            Tool::Ping => {
                self.ping_at = Some(pointer.world);
                Some(Action::Ping { at: pointer.world })
            }
            Tool::Fog => None,
            Tool::Wall => {
                self.wall_start = Some(pointer.world);
                None
            }
            Tool::Door => Some(Action::ToggleDoorNear { at: pointer.world }),
        }
    }

    /// Движение мыши.
    pub fn move_pointer(&mut self, pointer: Pointer) {
        if let Some(drag) = self.drag {
            // Позицию отдаёт вызывающая сторона через `drag_target`.
            let _ = drag;
        }
        if self.measure_from.is_some() {
            self.measure_to = Some(pointer.world);
        }
    }

    /// Текущая позиция перетаскиваемого токена (без привязки к сетке).
    #[must_use]
    pub fn drag_target(&self, pointer: Pointer) -> Option<(EntityId, Vec2)> {
        self.drag
            .map(|drag| (drag.token, pointer.world - drag.grab_offset))
    }

    /// Отпускание левой кнопкой.
    pub fn release(&mut self, pointer: Pointer, grid: &Grid) -> Option<Action> {
        if self.tool == Tool::Wall {
            let start = self.wall_start.take()?;
            if start.distance(pointer.world) < 8.0 {
                return None;
            }
            return Some(Action::AddWall {
                a: start,
                b: pointer.world,
                door: false,
            });
        }

        if let Some(drag) = self.drag.take() {
            let target = grid.snap(pointer.world - drag.grab_offset);
            return Some(Action::MoveToken {
                token: drag.token,
                to: target,
            });
        }

        if self.tool == Tool::Fog {
            self.ping_at = None;
            return Some(Action::Fog {
                center: pointer.world,
                radius: self.fog_radius,
            });
        }

        self.measure_from = None;
        self.measure_to = None;
        None
    }

    /// Длина текущей линейки в футах.
    #[must_use]
    pub fn measure_feet(&self, grid: &Grid) -> Option<f32> {
        let from = self.measure_from?;
        let to = self.measure_to?;
        Some(grid.distance_ft(from, to))
    }

    /// Сбросить временное состояние (например, по Esc).
    pub fn cancel(&mut self) {
        self.drag = None;
        self.measure_from = None;
        self.measure_to = None;
        self.ping_at = None;
        self.wall_start = None;
    }

    /// Начало и конец текущей стены (для предпросмотра).
    #[must_use]
    pub fn wall_line(&self, pointer: Pointer) -> Option<(Vec2, Vec2)> {
        self.wall_start.map(|start| (start, pointer.world))
    }

    /// Начало и конец текущей линейки (для отрисовки).
    #[must_use]
    pub fn measure_line(&self) -> Option<(Vec2, Vec2)> {
        let from = self.measure_from?;
        let to = self.measure_to?;
        Some((from, to))
    }

    /// Последняя точка пинга (для отрисовки «волны»).
    #[must_use]
    pub fn last_ping(&self) -> Option<Vec2> {
        self.ping_at
    }
}

/// Индекс ближайшей стены к точке (в пределах `max_distance` пикселей).
///
/// Нужен инструменту «дверь»: клик рядом со стеной переключает её.
#[must_use]
pub fn nearest_wall(scene: &Scene, point: Vec2, max_distance: f32) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (index, wall) in scene.walls.iter().enumerate() {
        let distance = point_segment_distance(point, wall.a, wall.b);
        if distance <= max_distance
            && best.is_none_or(|(_, best_distance)| distance < best_distance)
        {
            best = Some((index, distance));
        }
    }
    best.map(|(index, _)| index)
}

fn point_segment_distance(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let segment = b - a;
    let length_squared = segment.dot(segment);
    if length_squared <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(a + segment * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtt_core::entities::Token;

    fn scene_with_token() -> (Scene, EntityId) {
        let mut scene = Scene::new(EntityId(1), "Тест");
        let token = Token::new(EntityId(2), "Гоблин", Vec2::new(35.0, 35.0));
        let id = scene.add_token(token);
        (scene, id)
    }

    fn pointer_at(world: Vec2) -> Pointer {
        Pointer {
            world,
            left_down: true,
            right_down: false,
        }
    }

    #[test]
    fn select_drags_token_to_snapped_position() {
        let (scene, token) = scene_with_token();
        let mut interaction = Interaction::new();
        let grid = Grid::default();

        // Захватили токен чуть левее центра.
        interaction.press(pointer_at(Vec2::new(30.0, 30.0)), &scene);
        assert_eq!(interaction.drag().map(|d| d.token), Some(token));

        // Тянем в точку около другой клетки.
        interaction.move_pointer(pointer_at(Vec2::new(110.0, 100.0)));
        let action = interaction.release(pointer_at(Vec2::new(110.0, 100.0)), &grid);

        // Смещение захвата (−5,−5) сохраняется, затем снап к сетке.
        assert_eq!(
            action,
            Some(Action::MoveToken {
                token,
                to: Vec2::new(105.0, 105.0)
            })
        );
    }

    #[test]
    fn click_on_empty_space_does_nothing() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        let grid = Grid::default();
        interaction.press(pointer_at(Vec2::new(500.0, 500.0)), &scene);
        assert!(interaction.drag().is_none());
        assert_eq!(
            interaction.release(pointer_at(Vec2::new(500.0, 500.0)), &grid),
            None
        );
    }

    #[test]
    fn measure_tool_reports_feet() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        interaction.tool = Tool::Measure;
        let grid = Grid::default();

        interaction.press(pointer_at(Vec2::ZERO), &scene);
        interaction.move_pointer(pointer_at(Vec2::new(140.0, 0.0)));
        // 2 клетки по 5 футов.
        assert_eq!(interaction.measure_feet(&grid), Some(10.0));
        interaction.release(pointer_at(Vec2::new(140.0, 0.0)), &grid);
    }

    #[test]
    fn ping_is_immediate() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        interaction.tool = Tool::Ping;
        let action = interaction.press(pointer_at(Vec2::new(300.0, 200.0)), &scene);
        assert_eq!(
            action,
            Some(Action::Ping {
                at: Vec2::new(300.0, 200.0)
            })
        );
        assert!(interaction.ping_at.is_some());
    }

    #[test]
    fn fog_tool_creates_circle_on_release() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        interaction.tool = Tool::Fog;
        interaction.fog_radius = 90.0;
        let grid = Grid::default();

        interaction.press(pointer_at(Vec2::new(100.0, 100.0)), &scene);
        let action = interaction.release(pointer_at(Vec2::new(100.0, 100.0)), &grid);
        assert_eq!(
            action,
            Some(Action::Fog {
                center: Vec2::new(100.0, 100.0),
                radius: 90.0
            })
        );
    }

    #[test]
    fn cancel_clears_state() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        interaction.press(pointer_at(Vec2::new(35.0, 35.0)), &scene);
        assert!(interaction.drag().is_some());
        interaction.cancel();
        assert!(interaction.drag().is_none());
    }

    #[test]
    fn wall_tool_draws_segment_on_release() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        interaction.tool = Tool::Wall;
        let grid = Grid::default();

        interaction.press(pointer_at(Vec2::new(0.0, 0.0)), &scene);
        assert!(interaction
            .wall_line(pointer_at(Vec2::new(100.0, 0.0)))
            .is_some());
        let action = interaction.release(pointer_at(Vec2::new(100.0, 0.0)), &grid);
        assert_eq!(
            action,
            Some(Action::AddWall {
                a: Vec2::new(0.0, 0.0),
                b: Vec2::new(100.0, 0.0),
                door: false
            })
        );
    }

    #[test]
    fn wall_tool_ignores_tiny_segments() {
        let (scene, _) = scene_with_token();
        let mut interaction = Interaction::new();
        interaction.tool = Tool::Wall;
        let grid = Grid::default();
        interaction.press(pointer_at(Vec2::new(0.0, 0.0)), &scene);
        assert_eq!(
            interaction.release(pointer_at(Vec2::new(3.0, 0.0)), &grid),
            None
        );
    }

    #[test]
    fn door_tool_clicks_near_wall() {
        let mut scene = Scene::new(EntityId(1), "Двери");
        scene.walls.push(vtt_core::scene::Wall::door(
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 200.0),
            vtt_core::scene::DoorState::Closed,
        ));
        let mut interaction = Interaction::new();
        interaction.tool = Tool::Door;
        let action = interaction.press(pointer_at(Vec2::new(104.0, 100.0)), &scene);
        assert!(matches!(action, Some(Action::ToggleDoorNear { .. })));

        assert_eq!(nearest_wall(&scene, Vec2::new(104.0, 100.0), 15.0), Some(0));
        assert_eq!(nearest_wall(&scene, Vec2::new(300.0, 100.0), 15.0), None);
    }
}
