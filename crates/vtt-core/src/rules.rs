//! Формулы правил 5e / 5.5e.
//!
//! Здесь нет состояния — только чистые функции. Так их проще тестировать,
//! а игровые системы в будущем смогут подменять этот слой целиком
//! (сейчас он один, но интерфейс изолирован намеренно).

/// Модификатор характеристики: `(значение − 10) / 2`, округление вниз
/// (в том числе для значений ниже 10, поэтому `div_euclid`).
#[must_use]
pub fn ability_modifier(score: i32) -> i32 {
    (score - 10).div_euclid(2)
}

/// Бонус мастерства по уровню (1–20).
#[must_use]
pub fn proficiency_bonus(level: u8) -> i32 {
    let level = i32::from(level.clamp(1, 20));
    2 + (level - 1) / 4
}

/// Пассивное значение: 10 + бонус.
#[must_use]
pub fn passive_score(bonus: i32) -> i32 {
    10 + bonus
}

/// СЛ спасброска от заклинания: 8 + мастерство + модификатор.
#[must_use]
pub fn spell_save_dc(prof_bonus: i32, ability_mod: i32) -> i32 {
    8 + prof_bonus + ability_mod
}

/// Штраф за истощение: −1 ко всем тестам d20 за уровень (5e 2024).
#[must_use]
pub fn exhaustion_penalty(level: u8) -> i32 {
    -i32::from(level.min(6))
}

/// Словесная сложность проверки по СЛ.
#[must_use]
pub fn difficulty(dc: i32) -> &'static str {
    match dc {
        ..=5 => "очень просто",
        6..=10 => "просто",
        11..=15 => "средне",
        16..=20 => "сложно",
        21..=25 => "очень сложно",
        26..=30 => "почти невозможно",
        _ => "за пределами правил",
    }
}

/// Результат разрешения атаки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackResolution {
    pub total: i32,
    pub hit: bool,
    pub critical: bool,
    pub critical_miss: bool,
}

impl AttackResolution {
    #[must_use]
    pub fn describe(&self, target_ac: i32) -> String {
        if self.critical {
            format!("крит! {} против КД {target_ac}", self.total)
        } else if self.critical_miss {
            format!("натуральная 1 — промах (было бы {})", self.total)
        } else if self.hit {
            format!("попадание: {} против КД {target_ac}", self.total)
        } else {
            format!("промах: {} против КД {target_ac}", self.total)
        }
    }
}

/// Разрешить атаку. **Не применяет** урон — это делает ГМ.
///
/// Правила: натуральная 20 — всегда попадание и крит, натуральная 1 — всегда промах.
#[must_use]
pub fn resolve_attack(natural_d20: u32, attack_bonus: i32, target_ac: i32) -> AttackResolution {
    let total = natural_d20 as i32 + attack_bonus;
    let critical = natural_d20 == 20;
    let critical_miss = natural_d20 == 1;
    let hit = critical || (!critical_miss && total >= target_ac);
    AttackResolution {
        total,
        hit,
        critical,
        critical_miss,
    }
}

/// Урон с учётом сопротивления и уязвимости.
///
/// Полуавтомат: вызывается только когда ГМ нажал «применить».
#[must_use]
pub fn adjust_damage(raw: i32, resistance: bool, vulnerability: bool) -> i32 {
    let mut value = raw.max(0);
    if resistance {
        value /= 2;
    }
    if vulnerability {
        value *= 2;
    }
    value
}

/// Успех проверки против СЛ.
#[must_use]
pub fn check_succeeds(total: i32, dc: i32) -> bool {
    total >= dc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ability_modifier_table() {
        assert_eq!(ability_modifier(1), -5);
        assert_eq!(ability_modifier(9), -1);
        assert_eq!(ability_modifier(10), 0);
        assert_eq!(ability_modifier(15), 2);
        assert_eq!(ability_modifier(20), 5);
    }

    #[test]
    fn proficiency_at_key_levels() {
        let expected = |level: u8| match level {
            1..=4 => 2,
            5..=8 => 3,
            9..=12 => 4,
            13..=16 => 5,
            _ => 6,
        };
        for level in 1..=20u8 {
            assert_eq!(proficiency_bonus(level), expected(level), "уровень {level}");
        }
    }

    #[test]
    fn proficiency_clamps_out_of_range() {
        assert_eq!(proficiency_bonus(0), 2);
        assert_eq!(proficiency_bonus(99), 6);
    }

    #[test]
    fn passive_is_ten_plus_bonus() {
        assert_eq!(passive_score(5), 15);
        assert_eq!(passive_score(-2), 8);
    }

    #[test]
    fn spell_dc_formula() {
        // Мастерство +3, INT +4 → 8 + 3 + 4 = 15.
        assert_eq!(spell_save_dc(3, 4), 15);
    }

    #[test]
    fn attack_resolution_cases() {
        // Обычное попадание.
        let hit = resolve_attack(15, 5, 18);
        assert!(hit.hit && !hit.critical);
        assert_eq!(hit.total, 20);

        // Промах.
        assert!(!resolve_attack(5, 2, 18).hit);

        // Натуральная 20 пробивает любой КД.
        let crit = resolve_attack(20, 0, 40);
        assert!(crit.hit && crit.critical);

        // Натуральная 1 — промах даже при огромном бонусе.
        let miss = resolve_attack(1, 20, 5);
        assert!(!miss.hit && miss.critical_miss);
    }

    #[test]
    fn damage_resistance_halves() {
        assert_eq!(adjust_damage(11, true, false), 5);
        assert_eq!(adjust_damage(10, true, false), 5);
        assert_eq!(adjust_damage(10, false, true), 20);
        assert_eq!(adjust_damage(10, true, true), 10);
    }

    #[test]
    fn difficulty_labels() {
        assert_eq!(difficulty(5), "очень просто");
        assert_eq!(difficulty(15), "средне");
        assert_eq!(difficulty(30), "почти невозможно");
    }
}
