//! Нотация бросков и роллер.
//!
//! Поддерживается практическое подмножество нотации Roll20/Foundry:
//!
//! | Запись | Значение |
//! |---|---|
//! | `1d20` | один d20 |
//! | `2d6+3` | два d6 и +3 |
//! | `4d6kh3` | четыре d6, оставить три старших |
//! | `2d20kl1` | два d20, оставить младший |
//! | `d100` | `count` по умолчанию 1 |
//! | `-2+1d8` | плоский модификатор и кубик |
//!
//! Advantage/Disadvantage — не синтаксис строки, а [`RollMode`]: для одиночного
//! `1d20` бросаются два кубика, берётся лучший/худший. Это соответствует
//! правилам 5e/5.5e (бросок «с преимуществом»).
//!
//! Роллер принимает [`Rng`] снаружи — поэтому результат воспроизводим.

use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Максимум кубиков в одном терме (`100d6` допустимо, `1000d6` — нет).
pub const MAX_DICE_PER_TERM: u32 = 100;
/// Меньше d2 не бывает.
pub const MIN_SIDES: u32 = 2;
/// Больше d1000 в настолках не встречается.
pub const MAX_SIDES: u32 = 1000;

/// Режим броска d20.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RollMode {
    /// Обычный бросок.
    #[default]
    Normal,
    /// Преимущество: бросаем два d20, берём больший.
    Advantage,
    /// Помеха: бросаем два d20, берём меньший.
    Disadvantage,
}

impl RollMode {
    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Normal => "обычный",
            Self::Advantage => "преимущество",
            Self::Disadvantage => "помеха",
        }
    }
}

/// Ошибка разбора нотации.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiceError {
    /// Пустая строка или висячий знак.
    Empty,
    /// Ожидалось число.
    ExpectedNumber(usize),
    /// После `d` не указано число граней.
    ExpectedSides(usize),
    /// После `kh`/`kl` не указано количество.
    ExpectedKeepCount(usize),
    /// Неизвестный символ.
    UnexpectedChar(char),
    /// Граней меньше 2 или больше 1000.
    UnsupportedSides(u32),
    /// Слишком много кубиков.
    TooManyDice(u32),
    /// Оставить больше кубиков, чем брошено.
    InvalidKeep(u32),
    /// Отрицательные кубики не поддерживаются.
    NegativeDice,
    /// Число не влезло в `u32`/`i32`.
    InvalidNumber(usize),
}

impl fmt::Display for DiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "пустая формула броска"),
            Self::ExpectedNumber(pos) => write!(f, "ожидалось число в позиции {pos}"),
            Self::ExpectedSides(pos) => {
                write!(f, "после 'd' нужны грани (например, d20), позиция {pos}")
            }
            Self::ExpectedKeepCount(pos) => {
                write!(f, "после 'kh'/'kl' нужно число, позиция {pos}")
            }
            Self::UnexpectedChar(c) => write!(f, "неожиданный символ '{c}'"),
            Self::UnsupportedSides(sides) => write!(f, "неподдерживаемое число граней: {sides}"),
            Self::TooManyDice(count) => write!(f, "слишком много кубиков: {count}"),
            Self::InvalidKeep(count) => write!(f, "нельзя оставить {count} кубиков — их меньше"),
            Self::NegativeDice => write!(f, "отрицательные кубики не поддерживаются"),
            Self::InvalidNumber(pos) => write!(f, "слишком большое число в позиции {pos}"),
        }
    }
}

impl std::error::Error for DiceError {}

/// Что оставить после броска.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keep {
    All,
    Highest(u32),
    Lowest(u32),
}

/// Терм формулы: группа кубиков либо плоский модификатор.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Term {
    Dice { count: u32, sides: u32, keep: Keep },
    Flat { value: i32 },
}

/// Разобранная формула броска.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiceExpr {
    pub terms: Vec<Term>,
}

impl DiceExpr {
    /// Разобрать строку вроде `2d6+3` или `4d6kh3`.
    pub fn parse(input: &str) -> Result<Self, DiceError> {
        let chars: Vec<char> = input.chars().filter(|c| !c.is_whitespace()).collect();
        if chars.is_empty() {
            return Err(DiceError::Empty);
        }

        let mut terms = Vec::new();
        let mut i = 0usize;
        let mut sign: i32 = 1;
        match chars.first() {
            Some('+') => i = 1,
            Some('-') => {
                sign = -1;
                i = 1;
            }
            _ => {}
        }
        if i >= chars.len() {
            return Err(DiceError::Empty);
        }

        loop {
            let start = i;
            let digits_start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let has_digits = i > digits_start;
            let number = if has_digits {
                parse_u32(&chars[digits_start..i]).ok_or(DiceError::InvalidNumber(digits_start))?
            } else {
                0
            };

            if i < chars.len() && (chars[i] == 'd' || chars[i] == 'D') {
                if sign < 0 {
                    return Err(DiceError::NegativeDice);
                }
                i += 1;
                let sides_start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i == sides_start {
                    return Err(DiceError::ExpectedSides(i));
                }
                let sides = parse_u32(&chars[sides_start..i])
                    .ok_or(DiceError::InvalidNumber(sides_start))?;
                if !(MIN_SIDES..=MAX_SIDES).contains(&sides) {
                    return Err(DiceError::UnsupportedSides(sides));
                }
                let count = if has_digits { number } else { 1 };
                if count == 0 || count > MAX_DICE_PER_TERM {
                    return Err(DiceError::TooManyDice(count));
                }

                let mut keep = Keep::All;
                if i + 1 < chars.len() && (chars[i] == 'k' || chars[i] == 'K') {
                    let is_low = matches!(chars[i + 1], 'l' | 'L');
                    let is_high = matches!(chars[i + 1], 'h' | 'H');
                    if !is_low && !is_high {
                        return Err(DiceError::UnexpectedChar(chars[i]));
                    }
                    i += 2;
                    let keep_start = i;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i == keep_start {
                        return Err(DiceError::ExpectedKeepCount(i));
                    }
                    let k = parse_u32(&chars[keep_start..i])
                        .ok_or(DiceError::InvalidNumber(keep_start))?;
                    if k == 0 || k > count {
                        return Err(DiceError::InvalidKeep(k));
                    }
                    keep = if is_high {
                        Keep::Highest(k)
                    } else {
                        Keep::Lowest(k)
                    };
                }

                terms.push(Term::Dice { count, sides, keep });
            } else {
                if !has_digits {
                    return Err(DiceError::ExpectedNumber(start));
                }
                let value = i64::from(number) * i64::from(sign);
                if value > i64::from(i32::MAX) || value < i64::from(i32::MIN) {
                    return Err(DiceError::InvalidNumber(start));
                }
                terms.push(Term::Flat {
                    value: value as i32,
                });
            }

            if i >= chars.len() {
                break;
            }
            match chars[i] {
                '+' => {
                    sign = 1;
                    i += 1;
                }
                '-' => {
                    sign = -1;
                    i += 1;
                }
                c => return Err(DiceError::UnexpectedChar(c)),
            }
            if i >= chars.len() {
                return Err(DiceError::Empty);
            }
        }

        Ok(Self { terms })
    }

    /// Обратно в каноничную строку.
    #[must_use]
    pub fn notation(&self) -> String {
        let mut out = String::new();
        for (idx, term) in self.terms.iter().enumerate() {
            match *term {
                Term::Dice { count, sides, keep } => {
                    if idx > 0 {
                        out.push('+');
                    }
                    out.push_str(&format!("{count}d{sides}"));
                    match keep {
                        Keep::All => {}
                        Keep::Highest(k) => out.push_str(&format!("kh{k}")),
                        Keep::Lowest(k) => out.push_str(&format!("kl{k}")),
                    }
                }
                Term::Flat { value } => {
                    if value >= 0 && idx > 0 {
                        out.push('+');
                    }
                    if value < 0 {
                        out.push('-');
                    }
                    out.push_str(&value.unsigned_abs().to_string());
                }
            }
        }
        out
    }
}

/// Один брошенный кубик.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DieRoll {
    pub value: u32,
    /// `false`, если кубик отброшен (`kh`/`kl` или второй d20 при преимуществе).
    pub kept: bool,
}

/// Результат по одному терму формулы.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartResult {
    pub label: String,
    /// Грани, если это терм с кубиками (нужно для определения «натуральной 20»).
    pub sides: Option<u32>,
    pub rolls: Vec<DieRoll>,
    pub flat: Option<i32>,
}

impl PartResult {
    /// Сумма сохранённых кубиков плюс плоский модификатор.
    #[must_use]
    pub fn subtotal(&self) -> i32 {
        let dice: i32 = self
            .rolls
            .iter()
            .filter(|r| r.kept)
            .map(|r| r.value as i32)
            .sum();
        dice + self.flat.unwrap_or(0)
    }
}

/// Полный результат броска.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollResult {
    pub notation: String,
    pub mode: RollMode,
    pub total: i32,
    pub parts: Vec<PartResult>,
}

impl RollResult {
    /// Значение сохранённого d20 (если в формуле есть d20-терм).
    #[must_use]
    pub fn natural_d20(&self) -> Option<u32> {
        for part in &self.parts {
            if part.sides == Some(20) {
                return part.rolls.iter().find(|r| r.kept).map(|r| r.value);
            }
        }
        None
    }

    #[must_use]
    pub fn is_critical_hit(&self) -> bool {
        self.natural_d20() == Some(20)
    }

    #[must_use]
    pub fn is_critical_miss(&self) -> bool {
        self.natural_d20() == Some(1)
    }

    /// Человекочитаемая расшифровка для журнала/чата.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut out = self.notation.clone();
        if self.mode != RollMode::Normal {
            out.push_str(&format!(" ({})", self.mode.name_ru()));
        }
        out.push_str(&format!(" = {}", self.total));
        for part in &self.parts {
            if part.rolls.is_empty() {
                continue;
            }
            out.push_str(&format!(
                " [{}: {}]",
                part.label,
                part.rolls
                    .iter()
                    .map(|r| if r.kept {
                        r.value.to_string()
                    } else {
                        format!("~{}", r.value)
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        out
    }
}

/// Бросить разобранную формулу.
///
/// Если `mode` — преимущество/помеха и в формуле ровно один `1d20`,
/// для него бросаются два кубика, а второй помечается отброшенным.
#[must_use]
pub fn roll(expr: &DiceExpr, mode: RollMode, rng: &mut Rng) -> RollResult {
    let advantage_index = if mode == RollMode::Normal {
        None
    } else {
        single_d20_index(expr)
    };

    let mut parts = Vec::with_capacity(expr.terms.len());
    let mut total = 0i32;

    for (idx, term) in expr.terms.iter().enumerate() {
        match *term {
            Term::Flat { value } => {
                total += value;
                parts.push(PartResult {
                    label: if value >= 0 {
                        format!("+{value}")
                    } else {
                        value.to_string()
                    },
                    sides: None,
                    rolls: Vec::new(),
                    flat: Some(value),
                });
            }
            Term::Dice { count, sides, keep } => {
                let rolls = if Some(idx) == advantage_index {
                    let a = rng.roll(sides);
                    let b = rng.roll(sides);
                    let (kept, dropped) = match mode {
                        RollMode::Advantage if a >= b => (a, b),
                        RollMode::Advantage => (b, a),
                        RollMode::Disadvantage if a <= b => (a, b),
                        RollMode::Disadvantage => (b, a),
                        RollMode::Normal => (a, b),
                    };
                    vec![
                        DieRoll {
                            value: kept,
                            kept: true,
                        },
                        DieRoll {
                            value: dropped,
                            kept: false,
                        },
                    ]
                } else {
                    let values = (0..count).map(|_| rng.roll(sides)).collect();
                    apply_keep(values, keep)
                };

                let subtotal: i32 = rolls
                    .iter()
                    .filter(|r| r.kept)
                    .map(|r| r.value as i32)
                    .sum();
                total += subtotal;
                parts.push(PartResult {
                    label: format!("{count}d{sides}"),
                    sides: Some(sides),
                    rolls,
                    flat: None,
                });
            }
        }
    }

    RollResult {
        notation: expr.notation(),
        mode,
        total,
        parts,
    }
}

fn apply_keep(values: Vec<u32>, keep: Keep) -> Vec<DieRoll> {
    let n = values.len();
    let keep_n = match keep {
        Keep::All => n,
        Keep::Highest(k) | Keep::Lowest(k) => (k as usize).min(n),
    };

    let mut order: Vec<usize> = (0..n).collect();
    match keep {
        Keep::All => {}
        Keep::Highest(_) => order.sort_by(|&a, &b| values[b].cmp(&values[a]).then(a.cmp(&b))),
        Keep::Lowest(_) => order.sort_by(|&a, &b| values[a].cmp(&values[b]).then(a.cmp(&b))),
    }

    let mut kept = vec![false; n];
    for &idx in order.iter().take(keep_n) {
        kept[idx] = true;
    }

    values
        .into_iter()
        .enumerate()
        .map(|(i, value)| DieRoll {
            value,
            kept: kept[i],
        })
        .collect()
}

fn single_d20_index(expr: &DiceExpr) -> Option<usize> {
    let mut found = None;
    for (idx, term) in expr.terms.iter().enumerate() {
        if let Term::Dice {
            count: 1,
            sides: 20,
            keep: Keep::All,
        } = *term
        {
            if found.is_some() {
                return None;
            }
            found = Some(idx);
        }
    }
    found
}

fn parse_u32(chars: &[char]) -> Option<u32> {
    let mut value: u64 = 0;
    for &c in chars {
        let digit = c.to_digit(10)? as u64;
        value = value.checked_mul(10)?.checked_add(digit)?;
        if value > u64::from(u32::MAX) {
            return None;
        }
    }
    Some(value as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dice(count: u32, sides: u32, keep: Keep) -> Term {
        Term::Dice { count, sides, keep }
    }

    #[test]
    fn parses_simple_notation() {
        assert_eq!(
            DiceExpr::parse("2d6+3").unwrap().terms,
            vec![dice(2, 6, Keep::All), Term::Flat { value: 3 }]
        );
        assert_eq!(
            DiceExpr::parse("1d20 + 5").unwrap().terms,
            vec![dice(1, 20, Keep::All), Term::Flat { value: 5 }]
        );
        assert_eq!(
            DiceExpr::parse("d100").unwrap().terms,
            vec![dice(1, 100, Keep::All)]
        );
    }

    #[test]
    fn parses_keep_notation() {
        assert_eq!(
            DiceExpr::parse("4d6kh3").unwrap().terms,
            vec![dice(4, 6, Keep::Highest(3))]
        );
        assert_eq!(
            DiceExpr::parse("2d20kl1").unwrap().terms,
            vec![dice(2, 20, Keep::Lowest(1))]
        );
    }

    #[test]
    fn parses_negative_modifiers() {
        assert_eq!(
            DiceExpr::parse("1d8-2").unwrap().terms,
            vec![dice(1, 8, Keep::All), Term::Flat { value: -2 }]
        );
        assert_eq!(
            DiceExpr::parse("-3+1d8").unwrap().terms,
            vec![Term::Flat { value: -3 }, dice(1, 8, Keep::All)]
        );
    }

    #[test]
    fn notation_roundtrip() {
        for input in ["2d6+3", "4d6kh3", "1d20+5", "1d8-2", "2d20kl1"] {
            let expr = DiceExpr::parse(input).unwrap();
            assert_eq!(
                DiceExpr::parse(&expr.notation()).unwrap(),
                expr,
                "не сошлось для {input}"
            );
        }
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(DiceExpr::parse(""), Err(DiceError::Empty));
        assert_eq!(DiceExpr::parse("+"), Err(DiceError::Empty));
        assert!(DiceExpr::parse("abc").is_err());
        assert!(DiceExpr::parse("2d").is_err());
        assert!(DiceExpr::parse("d1").is_err());
        assert!(DiceExpr::parse("4d6kh9").is_err());
        assert!(DiceExpr::parse("-1d6").is_err());
        assert!(DiceExpr::parse("2d6+").is_err());
        assert!(DiceExpr::parse("101d6").is_err());
    }

    #[test]
    fn error_messages_are_russian() {
        let err = DiceExpr::parse("2d").unwrap_err();
        assert!(err.to_string().contains("грани"), "{err}");
    }

    #[test]
    fn rolls_in_expected_range() {
        let mut rng = Rng::new(42);
        let expr = DiceExpr::parse("2d6+3").unwrap();
        for _ in 0..1_000 {
            let result = roll(&expr, RollMode::Normal, &mut rng);
            assert!((5..=15).contains(&result.total), "{}", result.total);
        }
    }

    #[test]
    fn deterministic_with_same_seed() {
        let expr = DiceExpr::parse("3d8+2").unwrap();
        let a = roll(&expr, RollMode::Normal, &mut Rng::new(7));
        let b = roll(&expr, RollMode::Normal, &mut Rng::new(7));
        assert_eq!(a, b);
    }

    #[test]
    fn keeps_highest_three() {
        let expr = DiceExpr::parse("4d6kh3").unwrap();
        let mut rng = Rng::new(123);
        for _ in 0..500 {
            let result = roll(&expr, RollMode::Normal, &mut rng);
            let part = &result.parts[0];
            assert_eq!(part.rolls.len(), 4);
            assert_eq!(part.rolls.iter().filter(|r| r.kept).count(), 3);
            let expected: i32 = part
                .rolls
                .iter()
                .filter(|r| r.kept)
                .map(|r| r.value as i32)
                .sum();
            assert_eq!(result.total, expected);
            assert_eq!(part.subtotal(), expected);
        }
    }

    #[test]
    fn advantage_takes_higher_die() {
        let expr = DiceExpr::parse("1d20+5").unwrap();
        let mut rng = Rng::new(2024);
        for _ in 0..200 {
            let result = roll(&expr, RollMode::Advantage, &mut rng);
            let part = &result.parts[0];
            assert_eq!(part.rolls.len(), 2);
            assert!(part.rolls[0].kept);
            assert!(!part.rolls[1].kept);
            assert!(part.rolls[0].value >= part.rolls[1].value);
            assert_eq!(result.total, part.rolls[0].value as i32 + 5);
        }
    }

    #[test]
    fn disadvantage_takes_lower_die() {
        let expr = DiceExpr::parse("1d20").unwrap();
        let mut rng = Rng::new(11);
        for _ in 0..200 {
            let result = roll(&expr, RollMode::Disadvantage, &mut rng);
            assert!(result.parts[0].rolls[0].value <= result.parts[0].rolls[1].value);
        }
    }

    #[test]
    fn natural_twenty_detected() {
        let expr = DiceExpr::parse("1d20+3").unwrap();
        let mut rng = Rng::new(1);
        let mut found_nat20 = false;
        for _ in 0..2_000 {
            let result = roll(&expr, RollMode::Normal, &mut rng);
            if result.is_critical_hit() {
                found_nat20 = true;
                assert_eq!(result.natural_d20(), Some(20));
            }
        }
        assert!(found_nat20, "за 2000 бросков обязан выпасть крит");
    }

    #[test]
    fn describe_is_readable() {
        let expr = DiceExpr::parse("2d6+3").unwrap();
        let result = roll(&expr, RollMode::Normal, &mut Rng::new(5));
        let text = result.describe();
        assert!(text.contains("2d6+3"), "{text}");
        assert!(text.contains('='), "{text}");
    }
}
