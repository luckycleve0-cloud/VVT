//! Трекер инициативы и раунды.
//!
//! «Полуавтомат»: приложение хранит порядок и раунды, но не решает за ГМа,
//! кто ходит — он просто ведёт счётчик и подсвечивает активного.

use crate::ids::EntityId;
use serde::{Deserialize, Serialize};

/// Участник боя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Combatant {
    pub token: EntityId,
    pub name: String,
    pub initiative: i32,
}

/// Состояние боя на сцене.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Combat {
    pub active: bool,
    /// Номер раунда (с 1).
    pub round: u32,
    /// Отсортирован по инициативе: первый — ходит первым.
    pub order: Vec<Combatant>,
    /// Индекс того, чей сейчас ход.
    pub turn: usize,
}

impl Combat {
    /// Начать бой.
    pub fn start(&mut self) {
        self.active = true;
        if self.round == 0 {
            self.round = 1;
        }
        if self.turn >= self.order.len() {
            self.turn = 0;
        }
    }

    /// Закончить бой (порядок сохраняется — пригодится для следующего).
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Добавить участника с сортировкой по инициативе (по убыванию).
    pub fn add(&mut self, combatant: Combatant) {
        self.order.push(combatant);
        self.sort();
    }

    pub fn remove(&mut self, token: EntityId) {
        self.order.retain(|c| c.token != token);
        if self.turn >= self.order.len() {
            self.turn = 0;
        }
    }

    pub fn set_initiative(&mut self, token: EntityId, value: i32) -> bool {
        match self.order.iter_mut().find(|c| c.token == token) {
            Some(c) => {
                c.initiative = value;
                self.sort();
                true
            }
            None => false,
        }
    }

    fn sort(&mut self) {
        self.order
            .sort_by(|a, b| b.initiative.cmp(&a.initiative).then(a.name.cmp(&b.name)));
    }

    #[must_use]
    pub fn current(&self) -> Option<&Combatant> {
        self.order.get(self.turn)
    }

    /// Передать ход следующему; после последнего — новый раунд.
    pub fn next_turn(&mut self) -> Option<&Combatant> {
        if self.order.is_empty() {
            return None;
        }
        self.turn += 1;
        if self.turn >= self.order.len() {
            self.turn = 0;
            self.round = self.round.saturating_add(1);
        }
        self.order.get(self.turn)
    }

    pub fn clear(&mut self) {
        self.order.clear();
        self.turn = 0;
        self.round = 0;
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combatant(id: u64, name: &str, initiative: i32) -> Combatant {
        Combatant {
            token: EntityId(id),
            name: name.to_string(),
            initiative,
        }
    }

    #[test]
    fn order_follows_initiative() {
        let mut combat = Combat::default();
        combat.add(combatant(1, "Гоблин", 12));
        combat.add(combatant(2, "Торин", 18));
        combat.add(combatant(3, "Эльф", 15));
        let order: Vec<_> = combat.order.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(order, vec!["Торин", "Эльф", "Гоблин"]);
    }

    #[test]
    fn turns_wrap_and_increment_round() {
        let mut combat = Combat::default();
        combat.add(combatant(1, "A", 20));
        combat.add(combatant(2, "B", 10));
        combat.start();
        assert_eq!(combat.round, 1);
        assert_eq!(combat.current().unwrap().name, "A");

        assert_eq!(combat.next_turn().unwrap().name, "B");
        assert_eq!(combat.round, 1);

        assert_eq!(combat.next_turn().unwrap().name, "A");
        assert_eq!(combat.round, 2);
    }

    #[test]
    fn initiative_change_resorts() {
        let mut combat = Combat::default();
        combat.add(combatant(1, "A", 5));
        combat.add(combatant(2, "B", 10));
        assert!(combat.set_initiative(EntityId(1), 20));
        assert_eq!(combat.order[0].name, "A");
        assert!(!combat.set_initiative(EntityId(99), 1));
    }

    #[test]
    fn remove_keeps_indices_valid() {
        let mut combat = Combat::default();
        combat.add(combatant(1, "A", 20));
        combat.add(combatant(2, "B", 10));
        combat.start();
        combat.next_turn();
        combat.remove(EntityId(2));
        assert_eq!(combat.turn, 0);
        assert_eq!(combat.order.len(), 1);
    }

    #[test]
    fn empty_combat_is_safe() {
        let mut combat = Combat::default();
        assert!(combat.next_turn().is_none());
        assert!(combat.current().is_none());
        combat.start();
        assert!(combat.next_turn().is_none());
    }
}
