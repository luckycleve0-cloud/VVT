//! Генератор случайных чисел для бросков и раздачи идентификаторов.
//!
//! Свой маленький PRNG (SplitMix64) вместо `rand`, чтобы:
//!
//! - броски были **воспроизводимы**: один и тот же `seed` → одна и та же серия;
//! - не тянуть зависимость ради 30 строк;
//! - в тестах не приходилось мокать `thread_rng`.
//!
//! Качество распределения здесь важнее криптостойкости, но мы всё равно
//! используем rejection sampling, чтобы убрать перекос по модулю.

use std::time::{SystemTime, UNIX_EPOCH};

/// Детерминированный генератор (SplitMix64).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Засеять из системного времени и PID — для настоящей игры.
    #[must_use]
    pub fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64);
        Self::new(nanos ^ u64::from(std::process::id()).wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    /// Следующее 64-битное число (алгоритм SplitMix64).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Бросок одного кубика: значение в диапазоне `1..=sides`.
    ///
    /// Rejection sampling убирает перекос, который дал бы простой `% sides`.
    pub fn roll(&mut self, sides: u32) -> u32 {
        assert!(sides > 0, "у кубика должно быть хотя бы 2 грани");
        let sides_u64 = u64::from(sides);
        let limit = u64::MAX - (u64::MAX % sides_u64);
        loop {
            let v = self.next_u64();
            if v < limit {
                return (v % sides_u64 + 1) as u32;
            }
        }
    }

    /// `true` с вероятностью `numerator / 100`.
    pub fn chance_percent(&mut self, numerator: u32) -> bool {
        self.roll(100) <= numerator.min(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(1234);
        let mut b = Rng::new(1234);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn d6_stays_in_range() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let v = rng.roll(6);
            assert!((1..=6).contains(&v));
        }
    }

    #[test]
    fn distribution_is_roughly_uniform() {
        let mut rng = Rng::new(99);
        let mut counts = [0usize; 6];
        for _ in 0..60_000 {
            counts[(rng.roll(6) - 1) as usize] += 1;
        }
        for c in counts {
            // 10 000 ± 5% — щедрый допуск, но он ловит грубые ошибки.
            assert!(c > 9_000 && c < 11_000, "перекос: {counts:?}");
        }
    }
}
