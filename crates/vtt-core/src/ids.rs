//! Идентификаторы сущностей.
//!
//! Идея: `EntityId` — просто число, которое выдаёт сервер. Никаких UUID,
//! строк и составных ключей: с ними неудобно и синхронизировать, и отлаживать.
//! В JSON выглядит как обычное число (`"id": 7`).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Идентификатор любой сущности кампании (сцена, токен, персонаж, статья…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u64);

impl EntityId {
    /// «Пустой» идентификатор — означает «не задан».
    pub const NONE: EntityId = EntityId(0);

    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Идентификатор подключённого игрока (не сущности кампании).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(pub u64);

impl PlayerId {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }
}

impl fmt::Display for PlayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "player#{}", self.0)
    }
}

/// Монотонный генератор `EntityId`.
///
/// Живёт на сервере. Начинается с 1, чтобы [`EntityId::NONE`] никогда не
/// совпал с реальной сущностью.
#[derive(Debug, Clone)]
pub struct IdGen {
    next: u64,
}

impl Default for IdGen {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGen {
    #[must_use]
    pub const fn new() -> Self {
        Self { next: 1 }
    }

    /// Продолжить нумерацию после загрузки кампании.
    #[must_use]
    pub const fn starting_at(next: u64) -> Self {
        Self { next }
    }

    pub fn next_id(&mut self) -> EntityId {
        let id = self.next;
        self.next += 1;
        EntityId(id)
    }

    #[must_use]
    pub const fn peek(&self) -> u64 {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_start_at_one() {
        let mut gen = IdGen::new();
        let a = gen.next_id();
        let b = gen.next_id();
        assert_eq!(a, EntityId(1));
        assert_eq!(b, EntityId(2));
        assert_ne!(a, b);
        assert!(!a.is_none());
        assert!(EntityId::NONE.is_none());
    }

    #[test]
    fn ids_serialize_as_numbers() {
        let json = serde_json::to_string(&EntityId(42)).unwrap();
        assert_eq!(json, "42");
    }
}
