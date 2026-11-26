//! Лист персонажа для системы SCP (по правилам пользователя).
//!
//! Источник правил: «SCP. Настольная ролевая игра», автор Великий
//! (файл `SCP_Rules_Fixed.docx`). Документ описывает характеристики, навыки,
//! классы, уровни допуска, оружие, броню и психическое здоровье, но **не
//! задаёт формулу проверок**. Здесь принято явное домашнее правило (см.
//! [`CheckConfig`]) — его можно изменить в одном месте или пусть ГМ меняет СЛ.
//!
//! ## Что описано в правилах и реализовано здесь
//!
//! - 10 характеристик, старт с 1, 70 очков при создании, максимум 10 при создании;
//! - уровни допуска стоят очков характеристик (I — 0, II — 10, III — 30, IV — 50, V — 150);
//! - 42 навыка в 6 категориях, 84 очка навыков, максимум 8 при создании;
//! - 9 классов с бонусами;
//! - физическое здоровье = ВЫН × 15, психическое = ПСИ × 10;
//! - движение = СКО квадратов, без оружия — `1 + СИЛ/5` кубов d6;
//! - опыт: +2% за каждый пункт ИНТ; прокачка характеристики — 50 опыта (до 10), 100 — после;
//! - психоз (d10) и душевный подъём (d4), включая «единицу → подъём вместо психоза».
//!
//! ## Домашние правила (явно, чтобы не было сюрпризов)
//!
//! 1. Проверка: `d20 + характеристика + навык + бонус класса` против СЛ (по умолчанию 15).
//!    Натуральная 20 — успех, натуральная 1 — провал.
//! 2. Инициатива = СКО + бонус класса.
//! 3. Броня без указания — ОС 10 (кустарная), дальше 15/20/25.
//! 4. Бонусы класса к характеристикам (Ликвидатор: +1 ВЫН и ПСИ) считаются
//!    «эффективными»: они не тратят очки создания, но влияют на производные.

use crate::dice::{roll, DiceExpr, RollMode, RollResult};
use crate::ids::{EntityId, PlayerId};
use crate::rng::Rng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Характеристики
// ---------------------------------------------------------------------------

/// Десять характеристик системы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScpAttr {
    Strength,
    Speed,
    Agility,
    Perception,
    Psyche,
    Endurance,
    Charisma,
    Appeal,
    Tech,
    Intellect,
}

impl ScpAttr {
    pub const ALL: [ScpAttr; 10] = [
        ScpAttr::Strength,
        ScpAttr::Speed,
        ScpAttr::Agility,
        ScpAttr::Perception,
        ScpAttr::Psyche,
        ScpAttr::Endurance,
        ScpAttr::Charisma,
        ScpAttr::Appeal,
        ScpAttr::Tech,
        ScpAttr::Intellect,
    ];

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Strength => "Сила",
            Self::Speed => "Скорость",
            Self::Agility => "Ловкость",
            Self::Perception => "Восприятие",
            Self::Psyche => "Психика",
            Self::Endurance => "Выносливость",
            Self::Charisma => "Харизма",
            Self::Appeal => "Привлекательность",
            Self::Tech => "Техника",
            Self::Intellect => "Интеллект",
        }
    }

    #[must_use]
    pub fn short_ru(self) -> &'static str {
        match self {
            Self::Strength => "СИЛ",
            Self::Speed => "СКО",
            Self::Agility => "ЛВК",
            Self::Perception => "ВСП",
            Self::Psyche => "ПСИ",
            Self::Endurance => "ВЫН",
            Self::Charisma => "ХАР",
            Self::Appeal => "ПВЛ",
            Self::Tech => "ТЕХ",
            Self::Intellect => "ИНТ",
        }
    }
}

/// Значения десяти характеристик (база, без бонусов класса).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attributes {
    pub strength: u8,
    pub speed: u8,
    pub agility: u8,
    pub perception: u8,
    pub psyche: u8,
    pub endurance: u8,
    pub charisma: u8,
    pub appeal: u8,
    pub tech: u8,
    pub intellect: u8,
}

impl Default for Attributes {
    fn default() -> Self {
        // По правилам: «в каждой характеристике уже есть 1 параметр».
        Self {
            strength: 1,
            speed: 1,
            agility: 1,
            perception: 1,
            psyche: 1,
            endurance: 1,
            charisma: 1,
            appeal: 1,
            tech: 1,
            intellect: 1,
        }
    }
}

impl Attributes {
    #[must_use]
    pub fn get(self, attr: ScpAttr) -> u8 {
        match attr {
            ScpAttr::Strength => self.strength,
            ScpAttr::Speed => self.speed,
            ScpAttr::Agility => self.agility,
            ScpAttr::Perception => self.perception,
            ScpAttr::Psyche => self.psyche,
            ScpAttr::Endurance => self.endurance,
            ScpAttr::Charisma => self.charisma,
            ScpAttr::Appeal => self.appeal,
            ScpAttr::Tech => self.tech,
            ScpAttr::Intellect => self.intellect,
        }
    }

    pub fn set(&mut self, attr: ScpAttr, value: u8) {
        match attr {
            ScpAttr::Strength => self.strength = value,
            ScpAttr::Speed => self.speed = value,
            ScpAttr::Agility => self.agility = value,
            ScpAttr::Perception => self.perception = value,
            ScpAttr::Psyche => self.psyche = value,
            ScpAttr::Endurance => self.endurance = value,
            ScpAttr::Charisma => self.charisma = value,
            ScpAttr::Appeal => self.appeal = value,
            ScpAttr::Tech => self.tech = value,
            ScpAttr::Intellect => self.intellect = value,
        }
    }

    /// Сумма всех характеристик (старт — 10, потому что каждая начинается с 1).
    #[must_use]
    pub fn total(self) -> u32 {
        ScpAttr::ALL
            .iter()
            .map(|attr| u32::from(self.get(*attr)))
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Навыки
// ---------------------------------------------------------------------------

/// Категория навыка (по правилам их шесть).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    Agility,
    Perception,
    Endurance,
    Charisma,
    Tech,
    Intellect,
}

impl SkillCategory {
    pub const ALL: [SkillCategory; 6] = [
        SkillCategory::Agility,
        SkillCategory::Perception,
        SkillCategory::Endurance,
        SkillCategory::Charisma,
        SkillCategory::Tech,
        SkillCategory::Intellect,
    ];

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Agility => "Ловкость",
            Self::Perception => "Восприятие",
            Self::Endurance => "Выносливость",
            Self::Charisma => "Харизма",
            Self::Tech => "Техника",
            Self::Intellect => "Интеллект",
        }
    }

    /// Характеристика, на которой основаны навыки категории.
    #[must_use]
    pub fn attr(self) -> ScpAttr {
        match self {
            Self::Agility => ScpAttr::Agility,
            Self::Perception => ScpAttr::Perception,
            Self::Endurance => ScpAttr::Endurance,
            Self::Charisma => ScpAttr::Charisma,
            Self::Tech => ScpAttr::Tech,
            Self::Intellect => ScpAttr::Intellect,
        }
    }
}

/// Сорок два навыка из правил.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScpSkill {
    // Ловкость
    Stealth,
    Athletics,
    Acrobatics,
    Melee,
    Dodge,
    Theft,
    Lockpicking,
    SleightOfHand,
    // Восприятие
    Awareness,
    Tracking,
    Driving,
    Piloting,
    AutomaticFire,
    Pistols,
    Rifles,
    Shotguns,
    HeavyWeapons,
    // Выносливость
    TortureResistance,
    // Харизма
    Communication,
    Intimidation,
    Interrogation,
    Persuasion,
    Trading,
    Rapport,
    Leadership,
    // Техника
    TechKnowledge,
    Cybertech,
    Explosives,
    Electronics,
    AutoMechanics,
    Gunsmithing,
    Armorsmithing,
    Traps,
    FirstAid,
    Pharmaceuticals,
    Surgery,
    Psychology,
    // Интеллект
    Deduction,
    Erudition,
    Science,
    AnomalousScience,
    Analysis,
}

impl ScpSkill {
    pub const ALL: [ScpSkill; 42] = [
        ScpSkill::Stealth,
        ScpSkill::Athletics,
        ScpSkill::Acrobatics,
        ScpSkill::Melee,
        ScpSkill::Dodge,
        ScpSkill::Theft,
        ScpSkill::Lockpicking,
        ScpSkill::SleightOfHand,
        ScpSkill::Awareness,
        ScpSkill::Tracking,
        ScpSkill::Driving,
        ScpSkill::Piloting,
        ScpSkill::AutomaticFire,
        ScpSkill::Pistols,
        ScpSkill::Rifles,
        ScpSkill::Shotguns,
        ScpSkill::HeavyWeapons,
        ScpSkill::TortureResistance,
        ScpSkill::Communication,
        ScpSkill::Intimidation,
        ScpSkill::Interrogation,
        ScpSkill::Persuasion,
        ScpSkill::Trading,
        ScpSkill::Rapport,
        ScpSkill::Leadership,
        ScpSkill::TechKnowledge,
        ScpSkill::Cybertech,
        ScpSkill::Explosives,
        ScpSkill::Electronics,
        ScpSkill::AutoMechanics,
        ScpSkill::Gunsmithing,
        ScpSkill::Armorsmithing,
        ScpSkill::Traps,
        ScpSkill::FirstAid,
        ScpSkill::Pharmaceuticals,
        ScpSkill::Surgery,
        ScpSkill::Psychology,
        ScpSkill::Deduction,
        ScpSkill::Erudition,
        ScpSkill::Science,
        ScpSkill::AnomalousScience,
        ScpSkill::Analysis,
    ];

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Stealth => "Скрытность",
            Self::Athletics => "Атлетика",
            Self::Acrobatics => "Акробатика",
            Self::Melee => "Рукопашный бой",
            Self::Dodge => "Уклонение",
            Self::Theft => "Кража",
            Self::Lockpicking => "Взлом замков",
            Self::SleightOfHand => "Ловкость рук",
            Self::Awareness => "Внимательность",
            Self::Tracking => "Выслеживание",
            Self::Driving => "Вождение",
            Self::Piloting => "Пилотирование",
            Self::AutomaticFire => "Автоматический огонь",
            Self::Pistols => "Пистолеты",
            Self::Rifles => "Винтовки",
            Self::Shotguns => "Дробовики",
            Self::HeavyWeapons => "Тяжёлое оружие",
            Self::TortureResistance => "Сопротивление пыткам/наркотикам",
            Self::Communication => "Общение",
            Self::Intimidation => "Запугивание",
            Self::Interrogation => "Допрос",
            Self::Persuasion => "Убеждение",
            Self::Trading => "Торговля",
            Self::Rapport => "Расположение к себе",
            Self::Leadership => "Лидерство",
            Self::TechKnowledge => "Знание техники",
            Self::Cybertech => "Кибертехника",
            Self::Explosives => "Взрывчатка",
            Self::Electronics => "Электроника",
            Self::AutoMechanics => "Автомеханика",
            Self::Gunsmithing => "Оружейник",
            Self::Armorsmithing => "Бронник",
            Self::Traps => "Ловушки",
            Self::FirstAid => "Первая помощь",
            Self::Pharmaceuticals => "Фармацевтика",
            Self::Surgery => "Хирургия",
            Self::Psychology => "Психология",
            Self::Deduction => "Дедукция",
            Self::Erudition => "Эрудиция",
            Self::Science => "Наука",
            Self::AnomalousScience => "Аномальная наука",
            Self::Analysis => "Анализ",
        }
    }

    #[must_use]
    pub fn category(self) -> SkillCategory {
        match self {
            Self::Stealth
            | Self::Athletics
            | Self::Acrobatics
            | Self::Melee
            | Self::Dodge
            | Self::Theft
            | Self::Lockpicking
            | Self::SleightOfHand => SkillCategory::Agility,
            Self::Awareness
            | Self::Tracking
            | Self::Driving
            | Self::Piloting
            | Self::AutomaticFire
            | Self::Pistols
            | Self::Rifles
            | Self::Shotguns
            | Self::HeavyWeapons => SkillCategory::Perception,
            Self::TortureResistance => SkillCategory::Endurance,
            Self::Communication
            | Self::Intimidation
            | Self::Interrogation
            | Self::Persuasion
            | Self::Trading
            | Self::Rapport
            | Self::Leadership => SkillCategory::Charisma,
            Self::TechKnowledge
            | Self::Cybertech
            | Self::Explosives
            | Self::Electronics
            | Self::AutoMechanics
            | Self::Gunsmithing
            | Self::Armorsmithing
            | Self::Traps
            | Self::FirstAid
            | Self::Pharmaceuticals
            | Self::Surgery
            | Self::Psychology => SkillCategory::Tech,
            Self::Deduction
            | Self::Erudition
            | Self::Science
            | Self::AnomalousScience
            | Self::Analysis => SkillCategory::Intellect,
        }
    }

    #[must_use]
    pub fn attr(self) -> ScpAttr {
        self.category().attr()
    }

    /// Медицинские навыки: на них действует бонус Врача.
    #[must_use]
    pub fn is_medical(self) -> bool {
        matches!(
            self,
            Self::FirstAid | Self::Pharmaceuticals | Self::Surgery | Self::Psychology
        )
    }
}

// ---------------------------------------------------------------------------
// Классы и уровни допуска
// ---------------------------------------------------------------------------

/// Девять классов персонажей.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScpClass {
    Soldier,
    Medic,
    Scientist,
    Engineer,
    Liquidator,
    Agent,
    Manager,
    Expendable,
    Anomaly,
}

impl ScpClass {
    pub const ALL: [ScpClass; 9] = [
        ScpClass::Soldier,
        ScpClass::Medic,
        ScpClass::Scientist,
        ScpClass::Engineer,
        ScpClass::Liquidator,
        ScpClass::Agent,
        ScpClass::Manager,
        ScpClass::Expendable,
        ScpClass::Anomaly,
    ];

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Soldier => "Солдат",
            Self::Medic => "Врач",
            Self::Scientist => "Учёный",
            Self::Engineer => "Инженер",
            Self::Liquidator => "Ликвидатор",
            Self::Agent => "Агент",
            Self::Manager => "Управленец",
            Self::Expendable => "Расходный материал",
            Self::Anomaly => "Аномальный объект",
        }
    }

    #[must_use]
    pub fn ability_ru(self) -> &'static str {
        match self {
            Self::Soldier => "Боевая готовность: +2 к точности всех атак, +2 к инициативе, доп. боевое действие",
            Self::Medic => "Профессиональная деформация: +4 к медицинским навыкам, психический урон вдвое меньше",
            Self::Scientist => "Докторская степень: +12 к одной науке или аномальной науке",
            Self::Engineer => "Мастер на все руки: +2 ко всем навыкам техники (кроме медицинских), +6 к одному",
            Self::Liquidator => "Приспособленец: +1 ВЫН и ПСИ, +4 к инициативе, +4 к внимательности",
            Self::Agent => "Галстук: +4 к дедукции и анализу",
            Self::Manager => "Прирождённый лидер: +4 к лидерству, убеждению и расположению к себе",
            Self::Expendable => "Компенсация: +5 очков характеристик, +26 очков навыков, допуск нельзя менять",
            Self::Anomaly => "Особенности задаёт ГМ",
        }
    }

    /// Бонус к точности всех атак.
    #[must_use]
    pub fn accuracy_bonus(self) -> i32 {
        match self {
            Self::Soldier => 2,
            _ => 0,
        }
    }

    /// Бонус к инициативе.
    #[must_use]
    pub fn initiative_bonus(self) -> i32 {
        match self {
            Self::Soldier => 2,
            Self::Liquidator => 4,
            _ => 0,
        }
    }

    /// Есть ли дополнительное боевое действие.
    #[must_use]
    pub fn extra_combat_action(self) -> bool {
        self == Self::Soldier
    }

    /// Множитель психического урона (в процентах).
    #[must_use]
    pub fn mental_damage_percent(self) -> i32 {
        match self {
            Self::Medic => 50,
            _ => 100,
        }
    }

    /// Бонус к характеристике (считается «эффективным», не тратит очки).
    #[must_use]
    pub fn attribute_bonus(self, attr: ScpAttr) -> u8 {
        match (self, attr) {
            (Self::Liquidator, ScpAttr::Endurance | ScpAttr::Psyche) => 1,
            _ => 0,
        }
    }

    /// Дополнительные очки создания.
    #[must_use]
    pub fn extra_attribute_points(self) -> u32 {
        match self {
            Self::Expendable => 5,
            _ => 0,
        }
    }

    #[must_use]
    pub fn extra_skill_points(self) -> u32 {
        match self {
            Self::Expendable => 26,
            _ => 0,
        }
    }

    /// Бонус класса к конкретному навыку (без учёта выбранной специализации).
    #[must_use]
    pub fn skill_bonus(self, skill: ScpSkill) -> i32 {
        match self {
            Self::Medic if skill.is_medical() => 4,
            Self::Engineer if skill.category() == SkillCategory::Tech && !skill.is_medical() => 2,
            Self::Liquidator if skill == ScpSkill::Awareness => 4,
            Self::Agent if matches!(skill, ScpSkill::Deduction | ScpSkill::Analysis) => 4,
            Self::Manager
                if matches!(
                    skill,
                    ScpSkill::Leadership | ScpSkill::Persuasion | ScpSkill::Rapport
                ) =>
            {
                4
            }
            _ => 0,
        }
    }
}

/// Уровень допуска в организации.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    #[default]
    First,
    Second,
    Third,
    Fourth,
    Fifth,
}

impl AccessLevel {
    pub const ALL: [AccessLevel; 5] = [
        AccessLevel::First,
        AccessLevel::Second,
        AccessLevel::Third,
        AccessLevel::Fourth,
        AccessLevel::Fifth,
    ];

    #[must_use]
    pub fn numeral(self) -> &'static str {
        match self {
            Self::First => "I",
            Self::Second => "II",
            Self::Third => "III",
            Self::Fourth => "IV",
            Self::Fifth => "V",
        }
    }

    /// Сколько очков характеристик стоит уровень при создании.
    #[must_use]
    pub fn cost(self) -> u32 {
        match self {
            Self::First => 0,
            Self::Second => 10,
            Self::Third => 30,
            Self::Fourth => 50,
            Self::Fifth => 150,
        }
    }

    #[must_use]
    pub fn title_ru(self) -> &'static str {
        match self {
            Self::First => "Рядовой сотрудник",
            Self::Second => "Управление группой",
            Self::Third => "Начальник подразделения",
            Self::Fourth => "Начальник отдела",
            Self::Fifth => "Член начальства",
        }
    }
}

// ---------------------------------------------------------------------------
// Здоровье, психоз, подъём
// ---------------------------------------------------------------------------

/// Состояние здоровья (физического или психического).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

impl Health {
    #[must_use]
    pub const fn new(max: i32) -> Self {
        Self { current: max, max }
    }

    pub fn damage(&mut self, amount: i32) {
        if amount > 0 {
            self.current = (self.current - amount).max(0);
        }
    }

    pub fn heal(&mut self, amount: i32) {
        if amount > 0 {
            self.current = (self.current + amount).min(self.max);
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current <= 0
    }

    /// Доля здоровья для полоски в UI.
    #[must_use]
    pub fn ratio(&self) -> f32 {
        if self.max <= 0 {
            0.0
        } else {
            (self.current as f32 / self.max as f32).clamp(0.0, 1.0)
        }
    }
}

/// Вид психоза (d10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Psychosis {
    Masochism,
    Cowardice,
    Hopelessness,
    Paranoia,
    Aggression,
}

impl Psychosis {
    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Masochism => "Мазохизм",
            Self::Cowardice => "Трусость",
            Self::Hopelessness => "Безнадёжность",
            Self::Paranoia => "Паранойя",
            Self::Aggression => "Агрессия",
        }
    }

    #[must_use]
    pub fn description_ru(self) -> &'static str {
        match self {
            Self::Masochism => {
                "Не принимает лечение, каждый ход наносит себе урон рукопашной атакой, снимает шлем"
            }
            Self::Cowardice => "Не может совершать боевые действия",
            Self::Hopelessness => "Не может перемещаться",
            Self::Paranoia => {
                "Не принимает лечение, не доверяет союзникам, только действие или перемещение"
            }
            Self::Aggression => "Не принимает лечение, оскорбляет союзников, вызывая стресс",
        }
    }
}

/// Вид душевного подъёма (d4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Elation {
    Concentration,
    Bravery,
    Energy,
    Fortitude,
}

impl Elation {
    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Concentration => "Концентрация",
            Self::Bravery => "Смелость",
            Self::Energy => "Энергичность",
            Self::Fortitude => "Непоколебимость",
        }
    }

    #[must_use]
    pub fn description_ru(self) -> &'static str {
        match self {
            Self::Concentration => "+4 к точности атак, игнорирование штрафов окружения",
            Self::Bravery => "Не получает психический урон, снимает стресс союзникам",
            Self::Energy => "+2 к СКО и ЛВК, +6 к уклонению",
            Self::Fortitude => "Не получает психический урон, +2 к ВЫН",
        }
    }
}

/// Результат броска психики: либо психоз, либо (на 1) душевный подъём.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanityOutcome {
    /// Что выпало на d10.
    pub die: u32,
    pub psychosis: Option<Psychosis>,
    pub elation: Option<Elation>,
    /// Что выпало на d4 (если был подъём).
    pub elation_die: Option<u32>,
}

impl SanityOutcome {
    #[must_use]
    pub fn describe(&self) -> String {
        match (self.psychosis, self.elation) {
            (Some(psychosis), _) => format!(
                "d10={}: психоз «{}» — {}",
                self.die,
                psychosis.name_ru(),
                psychosis.description_ru()
            ),
            (_, Some(elation)) => format!(
                "d10=1 → подъём (d4={}): «{}» — {}",
                self.elation_die.unwrap_or(0),
                elation.name_ru(),
                elation.description_ru()
            ),
            _ => format!("d10={}: без эффекта", self.die),
        }
    }
}

/// Бросок психики по таблицам правил.
///
/// 1 → душевный подъём (d4); 2–3 мазохизм; 4–5 трусость; 6–7 безнадёжность;
/// 8–9 паранойя; 10 — агрессия.
#[must_use]
pub fn roll_sanity(rng: &mut Rng) -> SanityOutcome {
    let die = rng.roll(10);
    match die {
        1 => {
            let elation_die = rng.roll(4);
            let elation = match elation_die {
                1 => Elation::Concentration,
                2 => Elation::Bravery,
                3 => Elation::Energy,
                _ => Elation::Fortitude,
            };
            SanityOutcome {
                die,
                psychosis: None,
                elation: Some(elation),
                elation_die: Some(elation_die),
            }
        }
        2..=3 => SanityOutcome {
            die,
            psychosis: Some(Psychosis::Masochism),
            elation: None,
            elation_die: None,
        },
        4..=5 => SanityOutcome {
            die,
            psychosis: Some(Psychosis::Cowardice),
            elation: None,
            elation_die: None,
        },
        6..=7 => SanityOutcome {
            die,
            psychosis: Some(Psychosis::Hopelessness),
            elation: None,
            elation_die: None,
        },
        8..=9 => SanityOutcome {
            die,
            psychosis: Some(Psychosis::Paranoia),
            elation: None,
            elation_die: None,
        },
        _ => SanityOutcome {
            die,
            psychosis: Some(Psychosis::Aggression),
            elation: None,
            elation_die: None,
        },
    }
}

// ---------------------------------------------------------------------------
// Проверки
// ---------------------------------------------------------------------------

/// Параметры проверок (домашнее правило, ГМ может менять СЛ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckConfig {
    /// По умолчанию 15 — «средне».
    pub default_dc: i32,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self { default_dc: 15 }
    }
}

/// Итог проверки навыка или характеристики.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub roll: RollResult,
    /// Бонус: характеристика + навык + класс.
    pub bonus: i32,
    pub total: i32,
    pub dc: i32,
    pub success: bool,
    pub critical_success: bool,
    pub critical_failure: bool,
}

impl CheckResult {
    #[must_use]
    pub fn describe(&self, what: &str) -> String {
        let verdict = if self.critical_success {
            "критический успех"
        } else if self.critical_failure {
            "критический провал"
        } else if self.success {
            "успех"
        } else {
            "провал"
        };
        format!(
            "{what}: d20 {}{:+} = {} против СЛ {} — {verdict}",
            self.roll.total, self.bonus, self.total, self.dc
        )
    }
}

// ---------------------------------------------------------------------------
// Персонаж
// ---------------------------------------------------------------------------

/// Пользовательское поле лора в листе персонажа.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoreField {
    pub name: String,
    pub value: String,
}

/// Лист персонажа SCP.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScpCharacter {
    pub id: EntityId,
    pub name: String,
    pub class: ScpClass,
    pub organization: String,
    pub access: AccessLevel,
    /// Пользовательские лорные поля (имя + значение).
    #[serde(default)]
    pub lore: Vec<LoreField>,
    pub attributes: Attributes,
    pub skills: BTreeMap<ScpSkill, u8>,
    pub physical: Health,
    pub mental: Health,
    pub xp: i32,
    /// Специализация класса (Учёный — наука, Инженер — навык техники).
    #[serde(default)]
    pub class_focus: Option<ScpSkill>,
    /// Личная особенность, придуманная игроком (по правилам — по согласованию с ГМ).
    #[serde(default)]
    pub quirk: String,
    pub notes: String,
    /// Кому принадлежит персонаж (для фильтрации снапшота).
    #[serde(default)]
    pub owner: Option<PlayerId>,
}

impl ScpCharacter {
    /// Новый персонаж: характеристики по 1, здоровье выведено, опыта 0.
    #[must_use]
    pub fn new(id: EntityId, name: impl Into<String>, class: ScpClass) -> Self {
        let attributes = Attributes::default();
        let focus = match class {
            ScpClass::Scientist => Some(ScpSkill::Science),
            ScpClass::Engineer => Some(ScpSkill::TechKnowledge),
            _ => None,
        };
        let mut character = Self {
            id,
            name: name.into(),
            class,
            organization: "Фонд SCP".to_string(),
            access: AccessLevel::First,
            attributes,
            skills: BTreeMap::new(),
            lore: Vec::new(),
            physical: Health::new(0),
            mental: Health::new(0),
            xp: 0,
            class_focus: focus,
            quirk: String::new(),
            notes: String::new(),
            owner: None,
        };
        character.sync_health_max();
        character.physical.current = character.physical.max;
        character.mental.current = character.mental.max;
        character
    }

    /// Эффективное значение характеристики (база + бонус класса).
    #[must_use]
    pub fn attr(&self, attr: ScpAttr) -> u8 {
        self.attributes
            .get(attr)
            .saturating_add(self.class.attribute_bonus(attr))
    }

    /// Значение навыка: вложенные очки + бонус класса + специализация.
    #[must_use]
    pub fn skill_value(&self, skill: ScpSkill) -> u8 {
        let base = self.skills.get(&skill).copied().unwrap_or(0);
        let class = self.class.skill_bonus(skill);
        let focus = if self.class_focus == Some(skill) {
            match self.class {
                ScpClass::Scientist
                    if matches!(skill, ScpSkill::Science | ScpSkill::AnomalousScience) =>
                {
                    12
                }
                ScpClass::Engineer => 6,
                _ => 0,
            }
        } else {
            0
        };
        (u32::from(base) + class as u32 + focus as u32).min(u32::from(u8::MAX)) as u8
    }

    /// Максимум физического здоровья: ВЫН × 15.
    #[must_use]
    pub fn physical_max(&self) -> i32 {
        i32::from(self.attr(ScpAttr::Endurance)) * 15
    }

    /// Максимум психического здоровья: ПСИ × 10.
    #[must_use]
    pub fn mental_max(&self) -> i32 {
        i32::from(self.attr(ScpAttr::Psyche)) * 10
    }

    /// Движение в квадратах: равно СКО.
    #[must_use]
    pub fn movement_squares(&self) -> u8 {
        self.attr(ScpAttr::Speed)
    }

    /// Инициатива: СКО + бонус класса.
    #[must_use]
    pub fn initiative(&self) -> i32 {
        i32::from(self.attr(ScpAttr::Speed)) + self.class.initiative_bonus()
    }

    /// Урон без оружия: `1 + СИЛ/5` кубов d6.
    #[must_use]
    pub fn unarmed_dice(&self) -> u32 {
        1 + u32::from(self.attr(ScpAttr::Strength)) / 5
    }

    /// Бонус к дальнобойной точности от восприятия: ВСП/5.
    #[must_use]
    pub fn ranged_accuracy_bonus(&self) -> i32 {
        i32::from(self.attr(ScpAttr::Perception)) / 5
    }

    /// Сколько ударов за ход лёгким/средним (`light`) или тяжёлым оружием.
    #[must_use]
    pub fn melee_hits(&self, light: bool) -> u32 {
        let steps = u32::from(self.attr(ScpAttr::Speed)) / 5;
        1 + if light { steps * 2 } else { steps }
    }

    /// Множитель опыта: 100% + 2% за пункт ИНТ.
    #[must_use]
    pub fn xp_percent(&self) -> i32 {
        100 + 2 * i32::from(self.attr(ScpAttr::Intellect))
    }

    /// Пересчитать максимумы здоровья и подрезать текущие значения.
    pub fn sync_health_max(&mut self) {
        self.physical.max = self.physical_max();
        self.mental.max = self.mental_max();
        self.physical.current = self.physical.current.min(self.physical.max);
        self.mental.current = self.mental.current.min(self.mental.max);
    }

    /// Бонус проверки по навыку: характеристика + навык + бонус класса.
    #[must_use]
    pub fn check_bonus(&self, skill: ScpSkill) -> i32 {
        i32::from(self.attr(skill.attr())) + i32::from(self.skill_value(skill))
    }

    /// Бонус проверки «просто характеристики» (без навыка).
    #[must_use]
    pub fn attr_check_bonus(&self, attr: ScpAttr) -> i32 {
        i32::from(self.attr(attr))
    }

    /// Бонус атаки: характеристика + навык + классовая точность.
    #[must_use]
    pub fn attack_bonus(&self, skill: ScpSkill) -> i32 {
        i32::from(self.attr(skill.attr()))
            + i32::from(self.skill_value(skill))
            + self.class.accuracy_bonus()
    }

    /// Бросок проверки навыка.
    #[must_use]
    pub fn roll_check(&self, skill: ScpSkill, dc: i32, rng: &mut Rng) -> CheckResult {
        let expr = DiceExpr::parse("1d20").expect("1d20 разбирается всегда");
        let roll = roll(&expr, RollMode::Normal, rng);
        self.finish_check(skill.name_ru(), roll, self.check_bonus(skill), dc)
    }

    /// Бросок проверки характеристики (без навыка).
    #[must_use]
    pub fn roll_attr_check(&self, attr: ScpAttr, dc: i32, rng: &mut Rng) -> CheckResult {
        let expr = DiceExpr::parse("1d20").expect("1d20 разбирается всегда");
        let roll = roll(&expr, RollMode::Normal, rng);
        self.finish_check(attr.name_ru(), roll, self.attr_check_bonus(attr), dc)
    }

    /// Бросок атаки против ОС цели.
    #[must_use]
    pub fn roll_attack(&self, skill: ScpSkill, target_os: i32, rng: &mut Rng) -> CheckResult {
        let expr = DiceExpr::parse("1d20").expect("1d20 разбирается всегда");
        let roll = roll(&expr, RollMode::Normal, rng);
        self.finish_check("атака", roll, self.attack_bonus(skill), target_os)
    }

    fn finish_check(&self, what: &str, roll: RollResult, bonus: i32, dc: i32) -> CheckResult {
        let natural = roll.natural_d20().unwrap_or(0);
        let critical_success = natural == 20;
        let critical_failure = natural == 1;
        let total = roll.total + bonus;
        let success = critical_success || (!critical_failure && total >= dc);
        let result = CheckResult {
            roll,
            bonus,
            total,
            dc,
            success,
            critical_success,
            critical_failure,
        };
        debug_assert!(!result.describe(what).is_empty());
        result
    }

    /// Получить урон. Психический урон у Врача вдвое меньше.
    pub fn take_damage(&mut self, amount: i32, mental: bool) {
        if amount <= 0 {
            return;
        }
        if mental {
            let scaled = amount * self.class.mental_damage_percent() / 100;
            self.mental.damage(scaled.max(1));
        } else {
            self.physical.damage(amount);
        }
    }

    pub fn heal(&mut self, amount: i32, mental: bool) {
        if mental {
            self.mental.heal(amount);
        } else {
            self.physical.heal(amount);
        }
    }

    /// Начислить сырой опыт с учётом ИНТ; возвращает начисленное.
    pub fn grant_xp(&mut self, raw: i32) -> i32 {
        if raw <= 0 {
            return 0;
        }
        let gained = raw * self.xp_percent() / 100;
        self.xp += gained;
        gained
    }

    /// Стоимость повышения характеристики на 1.
    #[must_use]
    pub fn level_up_cost(&self, attr: ScpAttr) -> i32 {
        if self.attributes.get(attr) < 10 {
            50
        } else {
            100
        }
    }

    /// Потратить опыт на характеристику. Возвращает `Err` с причиной.
    pub fn level_up(&mut self, attr: ScpAttr) -> Result<i32, String> {
        let cost = self.level_up_cost(attr);
        if self.xp < cost {
            return Err(format!("нужно {cost} опыта, а есть {}", self.xp));
        }
        self.xp -= cost;
        let current = self.attributes.get(attr);
        self.attributes.set(attr, current.saturating_add(1));
        self.sync_health_max();
        Ok(cost)
    }

    /// Проверка корректности стартового листа.
    #[must_use]
    pub fn validate_creation(&self) -> CreationReport {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for attr in ScpAttr::ALL {
            let value = self.attributes.get(attr);
            if value == 0 {
                errors.push(format!("{} не может быть 0 (минимум 1)", attr.name_ru()));
            }
            if value > 10 {
                errors.push(format!(
                    "{} = {value}: при создании максимум 10",
                    attr.name_ru()
                ));
            }
        }

        if self.class == ScpClass::Expendable && self.access != AccessLevel::First {
            errors.push("Расходный материал не может менять уровень допуска".to_string());
        }
        if self.access == AccessLevel::Fifth && self.class != ScpClass::Anomaly {
            warnings.push("V уровень допуска крайне не рекомендуется на старте".to_string());
        }

        let attr_budget = 70 + self.class.extra_attribute_points();
        let attr_spent = self.attributes.total().saturating_sub(10) + self.access.cost();
        if attr_spent > attr_budget {
            errors.push(format!(
                "характеристики и допуск: потрачено {attr_spent} из {attr_budget}"
            ));
        }

        let skill_budget = 84 + self.class.extra_skill_points();
        let skill_spent: u32 = self.skills.values().map(|value| u32::from(*value)).sum();
        if skill_spent > skill_budget {
            errors.push(format!("навыки: потрачено {skill_spent} из {skill_budget}"));
        }
        for (skill, value) in &self.skills {
            if *value > 8 {
                errors.push(format!(
                    "навык «{}» = {value}: при создании максимум 8",
                    skill.name_ru()
                ));
            }
        }

        CreationReport {
            attribute_budget: attr_budget,
            attribute_spent: attr_spent,
            skill_budget,
            skill_points: skill_spent,
            errors,
            warnings,
        }
    }

    /// Короткая сводка для журнала/UI.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} — {}, {}, допуск {}. Физ {}/{}, псих {} /{}, XP {}",
            self.name,
            self.class.name_ru(),
            self.organization,
            self.access.numeral(),
            self.physical.current,
            self.physical.max,
            self.mental.current,
            self.mental.max,
            self.xp
        )
    }
}

/// Отчёт о корректности стартового листа.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationReport {
    pub attribute_budget: u32,
    pub attribute_spent: u32,
    pub skill_budget: u32,
    pub skill_points: u32,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl CreationReport {
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    #[must_use]
    pub fn describe(&self) -> String {
        let mut text = format!(
            "характеристики {}/{} (с доп. допуском), навыки {}/{}",
            self.attribute_spent, self.attribute_budget, self.skill_points, self.skill_budget
        );
        for error in &self.errors {
            text.push_str(&format!("\n  ✖ {error}"));
        }
        for warning in &self.warnings {
            text.push_str(&format!("\n  ! {warning}"));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character() -> ScpCharacter {
        let mut ch = ScpCharacter::new(EntityId(1), "Агент Смит", ScpClass::Soldier);
        ch.attributes = Attributes {
            strength: 5,
            speed: 6,
            agility: 5,
            perception: 6,
            psyche: 5,
            endurance: 7,
            charisma: 4,
            appeal: 4,
            tech: 3,
            intellect: 5,
        };
        ch.skills.insert(ScpSkill::Pistols, 4);
        ch.skills.insert(ScpSkill::Awareness, 3);
        ch.sync_health_max();
        ch.physical.current = ch.physical.max;
        ch.mental.current = ch.mental.max;
        ch
    }

    #[test]
    fn derived_stats_follow_rules() {
        let ch = character();
        // ВЫН 7 → 7×15 = 105 физического здоровья.
        assert_eq!(ch.physical.max, 105);
        // ПСИ 5 → 5×10 = 50 психического.
        assert_eq!(ch.mental.max, 50);
        assert_eq!(ch.movement_squares(), 6);
        // СИЛ 5 → 1 + 1 = 2d6 без оружия.
        assert_eq!(ch.unarmed_dice(), 2);
        // ВСП 6 → +1 к дальнобойной точности.
        assert_eq!(ch.ranged_accuracy_bonus(), 1);
        // СКО 6 → +2 к лёгкому (1+2), +1 к тяжёлому (1+1).
        assert_eq!(ch.melee_hits(true), 3);
        assert_eq!(ch.melee_hits(false), 2);
    }

    #[test]
    fn xp_multiplier_uses_intellect() {
        let mut ch = character();
        assert_eq!(ch.xp_percent(), 110); // ИНТ 5 → +10%
        assert_eq!(ch.grant_xp(100), 110);
        assert_eq!(ch.xp, 110);
    }

    #[test]
    fn level_up_costs_fifty_before_ten() {
        let mut ch = character();
        ch.xp = 120;
        let spent = ch.level_up(ScpAttr::Strength).unwrap();
        assert_eq!(spent, 50);
        assert_eq!(ch.attributes.strength, 6);

        // Поднимаем до 10, дальше должно стоить 100.
        ch.attributes.strength = 10;
        ch.xp = 200;
        assert_eq!(ch.level_up_cost(ScpAttr::Strength), 100);
        assert_eq!(ch.level_up(ScpAttr::Strength).unwrap(), 100);
        assert_eq!(ch.attributes.strength, 11);
    }

    #[test]
    fn level_up_requires_xp() {
        let mut ch = character();
        ch.xp = 10;
        let err = ch.level_up(ScpAttr::Strength).unwrap_err();
        assert!(err.contains("нужно 50"), "{err}");
    }

    #[test]
    fn class_bonuses_are_applied() {
        // Солдат: +2 к атаке, +2 к инициативе, доп. действие.
        let soldier = character();
        assert_eq!(soldier.class.accuracy_bonus(), 2);
        assert_eq!(soldier.initiative(), 6 + 2);
        assert!(soldier.class.extra_combat_action());

        // Медик: +4 к медицине и половина психического урона.
        let mut medic = ScpCharacter::new(EntityId(2), "Док", ScpClass::Medic);
        medic.attributes.endurance = 10;
        medic.attributes.psyche = 10;
        medic.sync_health_max();
        medic.skills.insert(ScpSkill::Surgery, 4);
        assert_eq!(medic.skill_value(ScpSkill::Surgery), 8); // 4 + 4
        medic.mental.current = 100;
        medic.take_damage(20, true);
        assert_eq!(
            medic.mental.current, 90,
            "психический урон должен делиться на 2"
        );

        // Ликвидатор: +1 ВЫН/ПСИ, +4 инициативы, +4 внимательности.
        let mut liquidator = ScpCharacter::new(EntityId(3), "Чистильщик", ScpClass::Liquidator);
        liquidator.attributes.endurance = 5;
        assert_eq!(liquidator.attr(ScpAttr::Endurance), 6);
        assert_eq!(liquidator.physical_max(), 90);
        assert_eq!(liquidator.skill_value(ScpSkill::Awareness), 4);

        // Агент: +4 к дедукции и анализу.
        let agent = ScpCharacter::new(EntityId(4), "Агент", ScpClass::Agent);
        assert_eq!(agent.skill_value(ScpSkill::Deduction), 4);

        // Управленец: +4 к лидерству, убеждению, расположению.
        let manager = ScpCharacter::new(EntityId(5), "Босс", ScpClass::Manager);
        assert_eq!(manager.skill_value(ScpSkill::Leadership), 4);
        assert_eq!(manager.skill_value(ScpSkill::Rapport), 4);
        assert_eq!(manager.skill_value(ScpSkill::Persuasion), 4);
    }

    #[test]
    fn scientist_focus_gives_twelve() {
        let mut scientist = ScpCharacter::new(EntityId(6), "Учёный", ScpClass::Scientist);
        scientist.skills.insert(ScpSkill::Science, 3);
        assert_eq!(scientist.skill_value(ScpSkill::Science), 15);
        scientist.skills.insert(ScpSkill::AnomalousScience, 3);
        // Специализация — обычная наука, аномальная получает только вложенные 3.
        assert_eq!(scientist.skill_value(ScpSkill::AnomalousScience), 3);
    }

    #[test]
    fn engineer_gets_tech_bonuses() {
        let mut engineer = ScpCharacter::new(EntityId(7), "Инженер", ScpClass::Engineer);
        engineer.skills.insert(ScpSkill::Electronics, 3);
        // 3 + 2 (все техн. навыки) = 5.
        assert_eq!(engineer.skill_value(ScpSkill::Electronics), 5);
        // Специализация «Знание техники»: 0 + 2 + 6 = 8.
        assert_eq!(engineer.skill_value(ScpSkill::TechKnowledge), 8);
        // Медицинские не получают инженерный бонус.
        assert_eq!(engineer.skill_value(ScpSkill::Surgery), 0);
    }

    #[test]
    fn checks_are_deterministic_and_critical() {
        let ch = character();
        let mut rng = Rng::new(7);
        let first = ch.roll_check(ScpSkill::Pistols, 15, &mut rng);
        let mut same = Rng::new(7);
        let second = ch.roll_check(ScpSkill::Pistols, 15, &mut same);
        assert_eq!(first, second);
        // Бонус: ЛВК? Нет — пистолеты в категории Восприятия: ВСП 6 + навык 4 = 10.
        assert_eq!(first.bonus, 10);

        // Натуральная 20 — успех при любом СЛ.
        let mut crit_rng = Rng::new(5);
        let mut found = false;
        for _ in 0..500 {
            let result = ch.roll_check(ScpSkill::Pistols, 100, &mut crit_rng);
            if result.critical_success {
                assert!(result.success);
                found = true;
                break;
            }
        }
        assert!(found, "за 500 бросков должен встретиться крит");
    }

    #[test]
    fn attack_uses_class_accuracy() {
        let ch = character();
        let bonus = ch.attack_bonus(ScpSkill::Pistols);
        // ВСП 6 + навык 4 + солдат 2 = 12.
        assert_eq!(bonus, 12);
    }

    #[test]
    fn creation_validation_accepts_valid_sheet() {
        let mut ch = character();
        // Характеристики: 50 всего → 40 потрачено из 70. Навыки: 7 из 84.
        let report = ch.validate_creation();
        assert!(report.is_ok(), "{}", report.describe());
        assert_eq!(report.attribute_budget, 70);
        assert_eq!(report.attribute_spent, 40);
        assert_eq!(report.skill_points, 7);

        ch.access = AccessLevel::Second;
        let report = ch.validate_creation();
        assert!(report.is_ok());
        assert_eq!(report.attribute_spent, 50);
    }

    #[test]
    fn creation_validation_catches_overspending() {
        let mut ch = character();
        ch.attributes = Attributes {
            strength: 10,
            speed: 10,
            agility: 10,
            perception: 10,
            psyche: 10,
            endurance: 10,
            charisma: 10,
            appeal: 10,
            tech: 10,
            intellect: 10,
        };
        let report = ch.validate_creation();
        assert!(!report.is_ok());
        assert!(report.describe().contains("потрачено 90 из 70"));

        ch.skills.insert(ScpSkill::Piloting, 9);
        let report = ch.validate_creation();
        assert!(report.describe().contains("максимум 8"));
    }

    #[test]
    fn expendable_gets_extra_budgets_but_cannot_raise_access() {
        let mut ch = ScpCharacter::new(EntityId(8), "Расходник", ScpClass::Expendable);
        let report = ch.validate_creation();
        assert_eq!(report.attribute_budget, 75);
        assert_eq!(report.skill_budget, 110);

        ch.access = AccessLevel::Second;
        let report = ch.validate_creation();
        assert!(!report.is_ok());
        assert!(report
            .describe()
            .contains("не может менять уровень допуска"));
    }

    #[test]
    fn sanity_rolls_cover_all_ranges() {
        let mut rng = Rng::new(42);
        let mut psychoses = 0;
        let mut elations = 0;
        for _ in 0..1_000 {
            let outcome = roll_sanity(&mut rng);
            if outcome.psychosis.is_some() {
                psychoses += 1;
            }
            if outcome.elation.is_some() {
                elations += 1;
                assert_eq!(outcome.die, 1);
            }
            assert!(!outcome.describe().is_empty());
        }
        assert!(psychoses > 700, "психоз должен выпадать часто: {psychoses}");
        assert!(elations > 50, "подъём должен иногда выпадать: {elations}");
    }

    #[test]
    fn health_clamps_and_syncs() {
        let mut ch = character();
        ch.take_damage(1000, false);
        assert_eq!(ch.physical.current, 0);
        assert!(ch.physical.is_empty());
        ch.heal(1000, false);
        assert_eq!(ch.physical.current, ch.physical.max);

        ch.attributes.endurance = 2;
        ch.sync_health_max();
        assert_eq!(ch.physical.max, 30);
        assert_eq!(ch.physical.current, 30);
    }

    #[test]
    fn serde_roundtrip_with_skill_map() {
        let ch = character();
        let json = serde_json::to_string(&ch).unwrap();
        assert!(
            json.contains("\"pistols\""),
            "ключи карты навыков должны быть строками"
        );
        let back: ScpCharacter = serde_json::from_str(&json).unwrap();
        assert_eq!(ch, back);
    }

    #[test]
    fn all_skills_have_unique_names_and_categories() {
        let mut names = std::collections::BTreeSet::new();
        for skill in ScpSkill::ALL {
            assert!(
                names.insert(skill.name_ru()),
                "дубликат {}",
                skill.name_ru()
            );
            assert!(ScpSkill::ALL.contains(&skill));
        }
        assert_eq!(names.len(), 42);
        assert_eq!(ScpAttr::ALL.len(), 10);
        assert_eq!(ScpClass::ALL.len(), 9);
    }
}
