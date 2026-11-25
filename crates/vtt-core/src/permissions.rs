//! Роли, права и доступ к объектам.
//!
//! Права проверяются **на сервере**, до применения интента. UI может
//! использовать тот же код, чтобы гасить кнопки, но это лишь удобство:
//! доверять клиенту нельзя.

use crate::ids::{EntityId, PlayerId};
use serde::{Deserialize, Serialize};

/// Роль подключённого участника.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Gm,
    CoGm,
    #[default]
    Player,
    /// Смотрит, но не играет (зритель стрима, «присутствующий»).
    Spectator,
}

impl Role {
    #[must_use]
    pub fn is_gm_like(self) -> bool {
        matches!(self, Self::Gm | Self::CoGm)
    }

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Gm => "ГМ",
            Self::CoGm => "Со-ГМ",
            Self::Player => "Игрок",
            Self::Spectator => "Зритель",
        }
    }
}

/// Что участник пытается сделать.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Видеть объект в снапшоте.
    View,
    /// Двигать токен.
    Move,
    /// Менять содержимое (поля, хиты, текст).
    Edit,
    /// Удалять.
    Delete,
    /// Управлять комнатой: кик, пароль, смена сцены.
    ManageRoom,
}

/// К чему применяется действие.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    Token(EntityId),
    Actor(EntityId),
    Scene(EntityId),
    Journal(EntityId),
    /// Настройки сервера/комнаты.
    Settings,
}

impl Target {
    #[must_use]
    pub fn entity_id(self) -> Option<EntityId> {
        match self {
            Self::Token(id) | Self::Actor(id) | Self::Scene(id) | Self::Journal(id) => Some(id),
            Self::Settings => None,
        }
    }
}

/// Кто и с какой ролью спрашивает.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Access {
    pub player: PlayerId,
    pub role: Role,
}

impl Access {
    #[must_use]
    pub const fn new(player: PlayerId, role: Role) -> Self {
        Self { player, role }
    }

    #[must_use]
    pub const fn gm() -> Self {
        Self {
            player: PlayerId(0),
            role: Role::Gm,
        }
    }
}

/// Разрешено ли действие.
///
/// `owner` — владелец объекта (для токена/персонажа). Передаётся отдельно,
/// потому что право зависит от объекта, а не от роли.
#[must_use]
pub fn can(
    access: &Access,
    capability: Capability,
    target: Target,
    owner: Option<PlayerId>,
) -> bool {
    match access.role {
        // ГМ может всё.
        Role::Gm => true,
        // Со-ГМ может всё, кроме управления комнатой.
        Role::CoGm => capability != Capability::ManageRoom,
        // Игрок: смотрит всё, меняет только своё.
        Role::Player => match capability {
            Capability::View => true,
            Capability::ManageRoom => false,
            Capability::Move | Capability::Edit | Capability::Delete => match target {
                Target::Token(_) | Target::Actor(_) => owner == Some(access.player),
                Target::Scene(_) | Target::Journal(_) | Target::Settings => false,
            },
        },
        // Зритель: только смотрит, и только игровые объекты.
        Role::Spectator => {
            capability == Capability::View
                && !matches!(target, Target::Settings)
                && !matches!(target, Target::Actor(_))
                && !matches!(target, Target::Journal(_))
        }
    }
}

/// Скрывать ли объект от этого участника в снапшоте.
///
/// Отдельная функция, потому что скрытность — не «право», а фильтр данных:
/// скрытый токен не должен даже приехать клиенту, иначе его видно в памяти.
#[must_use]
pub fn is_object_hidden(access: &Access, hidden: bool) -> bool {
    hidden && !access.role.is_gm_like()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GM: Access = Access::new(PlayerId(0), Role::Gm);
    const COGM: Access = Access::new(PlayerId(1), Role::CoGm);
    const ALICE: Access = Access::new(PlayerId(2), Role::Player);
    const BOB: Access = Access::new(PlayerId(3), Role::Player);
    const VIEWER: Access = Access::new(PlayerId(4), Role::Spectator);

    fn token(id: u64) -> Target {
        Target::Token(EntityId(id))
    }

    #[test]
    fn gm_can_everything() {
        for cap in [
            Capability::View,
            Capability::Move,
            Capability::Edit,
            Capability::Delete,
            Capability::ManageRoom,
        ] {
            assert!(can(&GM, cap, token(1), Some(PlayerId(2))));
            assert!(can(&GM, cap, Target::Settings, None));
        }
    }

    #[test]
    fn co_gm_cannot_manage_room() {
        assert!(can(&COGM, Capability::Edit, token(1), None));
        assert!(!can(&COGM, Capability::ManageRoom, Target::Settings, None));
    }

    #[test]
    fn player_moves_only_own_token() {
        assert!(can(&ALICE, Capability::Move, token(1), Some(PlayerId(2))));
        assert!(!can(&ALICE, Capability::Move, token(1), Some(PlayerId(3))));
        assert!(!can(&ALICE, Capability::Move, token(1), None));
    }

    #[test]
    fn player_cannot_manage_room_or_settings() {
        assert!(!can(&ALICE, Capability::ManageRoom, Target::Settings, None));
        assert!(!can(&ALICE, Capability::Edit, Target::Settings, None));
        assert!(!can(
            &ALICE,
            Capability::Edit,
            Target::Scene(EntityId(5)),
            None
        ));
        assert!(!can(
            &ALICE,
            Capability::Edit,
            Target::Journal(EntityId(6)),
            None
        ));
    }

    #[test]
    fn player_can_view_anything() {
        assert!(can(&ALICE, Capability::View, token(1), Some(PlayerId(3))));
        assert!(can(
            &ALICE,
            Capability::View,
            Target::Scene(EntityId(9)),
            None
        ));
    }

    #[test]
    fn spectator_only_views_game_objects() {
        assert!(can(&VIEWER, Capability::View, token(1), None));
        assert!(can(
            &VIEWER,
            Capability::View,
            Target::Scene(EntityId(2)),
            None
        ));
        assert!(!can(&VIEWER, Capability::View, Target::Settings, None));
        assert!(!can(
            &VIEWER,
            Capability::View,
            Target::Actor(EntityId(3)),
            None
        ));
        assert!(!can(&VIEWER, Capability::Move, token(1), Some(PlayerId(4))));
        assert!(!can(
            &BOB,
            Capability::Delete,
            Target::Scene(EntityId(1)),
            None
        ));
    }

    #[test]
    fn hidden_objects_are_filtered_for_players() {
        assert!(is_object_hidden(&ALICE, true));
        assert!(!is_object_hidden(&ALICE, false));
        assert!(!is_object_hidden(&GM, true));
        assert!(!is_object_hidden(&COGM, true));
    }
}
