//! Сущности стола: персонажи, эффекты, состояния, токены.
//!
//! Здесь только данные и «тонкие» вычисления. Формулы правил живут в
//! [`crate::rules`], а отрисовка и сеть — в других крейтах.
//!
//! Главная идея — [`Probe`]: любой расчёт возвращает не просто число, а
//! **расшифровку** («+5 = DEX +3, мастерство +2»). Это нужно, чтобы ГМ видел,
//! откуда взялся бонус, и не спорил с приложением.

use crate::geom::Vec2;
use crate::ids::{EntityId, PlayerId};
use crate::rules;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Характеристики и навыки
// ---------------------------------------------------------------------------

/// Шесть характеристик.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ability {
    Str,
    Dex,
    Con,
    Int,
    Wis,
    Cha,
}

impl Ability {
    pub const ALL: [Ability; 6] = [
        Ability::Str,
        Ability::Dex,
        Ability::Con,
        Ability::Int,
        Ability::Wis,
        Ability::Cha,
    ];

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Str => "Сила",
            Self::Dex => "Ловкость",
            Self::Con => "Телосложение",
            Self::Int => "Интеллект",
            Self::Wis => "Мудрость",
            Self::Cha => "Харизма",
        }
    }
}

/// Восемнадцать навыков 5e.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
}

impl Skill {
    pub const ALL: [Skill; 18] = [
        Skill::Acrobatics,
        Skill::AnimalHandling,
        Skill::Arcana,
        Skill::Athletics,
        Skill::Deception,
        Skill::History,
        Skill::Insight,
        Skill::Intimidation,
        Skill::Investigation,
        Skill::Medicine,
        Skill::Nature,
        Skill::Perception,
        Skill::Performance,
        Skill::Persuasion,
        Skill::Religion,
        Skill::SleightOfHand,
        Skill::Stealth,
        Skill::Survival,
    ];

    /// К какой характеристике привязан навык.
    #[must_use]
    pub fn ability(self) -> Ability {
        match self {
            Self::Acrobatics | Self::SleightOfHand | Self::Stealth => Ability::Dex,
            Self::AnimalHandling
            | Self::Insight
            | Self::Medicine
            | Self::Perception
            | Self::Survival => Ability::Wis,
            Self::Arcana | Self::History | Self::Investigation | Self::Nature | Self::Religion => {
                Ability::Int
            }
            Self::Athletics => Ability::Str,
            Self::Deception | Self::Intimidation | Self::Performance | Self::Persuasion => {
                Ability::Cha
            }
        }
    }

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Acrobatics => "Акробатика",
            Self::AnimalHandling => "Обращение с животными",
            Self::Arcana => "Магия",
            Self::Athletics => "Атлетика",
            Self::Deception => "Обман",
            Self::History => "История",
            Self::Insight => "Проницательность",
            Self::Intimidation => "Запугивание",
            Self::Investigation => "Анализ",
            Self::Medicine => "Медицина",
            Self::Nature => "Природа",
            Self::Perception => "Внимательность",
            Self::Performance => "Выступление",
            Self::Persuasion => "Убеждение",
            Self::Religion => "Религия",
            Self::SleightOfHand => "Ловкость рук",
            Self::Stealth => "Скрытность",
            Self::Survival => "Выживание",
        }
    }
}

/// Уровень владения: ничего, владение, экспертиза.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfLevel {
    #[default]
    None,
    Proficient,
    Expertise,
}

impl ProfLevel {
    /// Во сколько раз бонус мастерства умножается.
    #[must_use]
    pub fn multiplier(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Proficient => 1,
            Self::Expertise => 2,
        }
    }
}

/// Значения характеристик (10 — «средний человек»).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Abilities {
    #[serde(rename = "str")]
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub int: i32,
    pub wis: i32,
    pub cha: i32,
}

impl Default for Abilities {
    fn default() -> Self {
        Self {
            str_: 10,
            dex: 10,
            con: 10,
            int: 10,
            wis: 10,
            cha: 10,
        }
    }
}

impl Abilities {
    #[must_use]
    pub const fn new(str_: i32, dex: i32, con: i32, int: i32, wis: i32, cha: i32) -> Self {
        Self {
            str_,
            dex,
            con,
            int,
            wis,
            cha,
        }
    }

    #[must_use]
    pub fn get(self, ability: Ability) -> i32 {
        match ability {
            Ability::Str => self.str_,
            Ability::Dex => self.dex,
            Ability::Con => self.con,
            Ability::Int => self.int,
            Ability::Wis => self.wis,
            Ability::Cha => self.cha,
        }
    }

    pub fn set(&mut self, ability: Ability, value: i32) {
        match ability {
            Ability::Str => self.str_ = value,
            Ability::Dex => self.dex = value,
            Ability::Con => self.con = value,
            Ability::Int => self.int = value,
            Ability::Wis => self.wis = value,
            Ability::Cha => self.cha = value,
        }
    }

    /// Модификатор характеристики: `(значение - 10) / 2` с округлением вниз.
    #[must_use]
    pub fn modifier(self, ability: Ability) -> i32 {
        rules::ability_modifier(self.get(ability))
    }
}

/// Владения: спасброски и навыки.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Proficiency {
    pub saves: BTreeSet<Ability>,
    pub skills: BTreeMap<Skill, ProfLevel>,
}

impl Proficiency {
    #[must_use]
    pub fn skill_level(&self, skill: Skill) -> ProfLevel {
        self.skills.get(&skill).copied().unwrap_or_default()
    }

    #[must_use]
    pub fn save_proficient(&self, ability: Ability) -> bool {
        self.saves.contains(&ability)
    }

    pub fn set_skill(&mut self, skill: Skill, level: ProfLevel) {
        if level == ProfLevel::None {
            self.skills.remove(&skill);
        } else {
            self.skills.insert(skill, level);
        }
    }
}

// ---------------------------------------------------------------------------
// Здоровье и состояния
// ---------------------------------------------------------------------------

/// Хиты: текущие, максимум и временные.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hp {
    pub current: i32,
    pub max: i32,
    pub temp: i32,
}

impl Hp {
    #[must_use]
    pub const fn new(max: i32) -> Self {
        Self {
            current: max,
            max,
            temp: 0,
        }
    }

    /// Урон: сначала съедает временные хиты (5e 2024).
    pub fn damage(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        let absorbed = self.temp.min(amount);
        self.temp -= absorbed;
        self.current = (self.current - (amount - absorbed)).max(0);
    }

    pub fn heal(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        self.current = (self.current + amount).min(self.max);
    }

    #[must_use]
    pub fn is_down(&self) -> bool {
        self.current <= 0
    }

    /// «Половина хитов или меньше» — условие Bloodied (5e 2024).
    #[must_use]
    pub fn is_bloodied(&self) -> bool {
        self.max > 0 && self.current > 0 && self.current * 2 <= self.max
    }
}

/// Счётчики спасбросков от смерти (5e: 3 успеха — стабилен, 3 провала — мёртв).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeathSaves {
    pub successes: u8,
    pub failures: u8,
}

impl DeathSaves {
    /// Стабилизировался (три успеха).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.successes >= 3
    }

    /// Умер (три провала).
    #[must_use]
    pub fn is_dead(&self) -> bool {
        self.failures >= 3
    }

    pub fn reset(&mut self) {
        self.successes = 0;
        self.failures = 0;
    }
}

/// Состояния 5e 2024 (включая Bloodied как удобный статус).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    Blinded,
    Charmed,
    Deafened,
    /// Уровень истощения 1–6 (6 — смерть).
    Exhaustion(u8),
    Frightened,
    Grappled,
    Incapacitated,
    Invisible,
    Paralyzed,
    Petrified,
    Poisoned,
    Prone,
    Restrained,
    Stunned,
    Unconscious,
    /// Не состояние правил, а удобная метка «хиты ≤ половины».
    Bloodied,
}

impl Condition {
    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Blinded => "Ослеплён",
            Self::Charmed => "Очарован",
            Self::Deafened => "Оглох",
            Self::Exhaustion(_) => "Истощение",
            Self::Frightened => "Испуган",
            Self::Grappled => "Схвачен",
            Self::Incapacitated => "Недееспособен",
            Self::Invisible => "Невидим",
            Self::Paralyzed => "Парализован",
            Self::Petrified => "Окаменел",
            Self::Poisoned => "Отравлен",
            Self::Prone => "Сбит с ног",
            Self::Restrained => "Опутан",
            Self::Stunned => "Ошеломлён",
            Self::Unconscious => "Без сознания",
            Self::Bloodied => "Изранен",
        }
    }

    /// Состояния, при которых нельзя действовать.
    #[must_use]
    pub fn is_incapacitating(self) -> bool {
        matches!(
            self,
            Self::Incapacitated
                | Self::Paralyzed
                | Self::Petrified
                | Self::Stunned
                | Self::Unconscious
        )
    }

    /// По таким целям атаки идут с преимуществом.
    #[must_use]
    pub fn grants_advantage_to_attackers(self) -> bool {
        matches!(
            self,
            Self::Blinded
                | Self::Paralyzed
                | Self::Petrified
                | Self::Prone
                | Self::Restrained
                | Self::Stunned
                | Self::Unconscious
        )
    }

    /// Провал спасбросков Силы и Ловкости.
    #[must_use]
    pub fn auto_fails_str_dex_saves(self) -> bool {
        matches!(
            self,
            Self::Paralyzed
                | Self::Petrified
                | Self::Restrained
                | Self::Stunned
                | Self::Unconscious
        )
    }

    /// Уровень истощения, если это оно.
    #[must_use]
    pub fn exhaustion_level(self) -> Option<u8> {
        match self {
            Self::Exhaustion(level) => Some(level),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Эффекты и модификаторы
// ---------------------------------------------------------------------------

/// К чему применяется модификатор.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModTarget {
    Ac,
    Initiative,
    Speed,
    /// Броски атаки.
    Attack,
    /// Броски урона.
    Damage,
    Check(Ability),
    Save(Ability),
    Skill(Skill),
}

/// Как модификатор влияет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModOp {
    Add,
    /// Устанавливает значение вместо сложения (например, AC = 13).
    Set,
    Advantage,
    Disadvantage,
}

/// Один модификатор из эффекта/предмета/состояния.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifier {
    pub target: ModTarget,
    pub op: ModOp,
    pub value: i16,
    /// Откуда взялось — попадает в расшифровку.
    pub source: String,
}

/// Набор модификаторов (эффекты, предметы, баффы).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ModifierSet {
    pub items: Vec<Modifier>,
}

impl ModifierSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, modifier: Modifier) {
        self.items.push(modifier);
    }

    /// Суммарное числовое изменение (складывает `Add`, учитывает `Set`).
    #[must_use]
    pub fn total(&self, target: ModTarget) -> i32 {
        let mut total = 0i32;
        for item in &self.items {
            if item.target != target {
                continue;
            }
            match item.op {
                ModOp::Add => total += i32::from(item.value),
                ModOp::Set => total = i32::from(item.value),
                ModOp::Advantage | ModOp::Disadvantage => {}
            }
        }
        total
    }

    /// +1 — преимущество, -1 — помеха, 0 — ничего.
    #[must_use]
    pub fn advantage(&self, target: ModTarget) -> i8 {
        let mut score = 0i8;
        for item in &self.items {
            if item.target != target {
                continue;
            }
            match item.op {
                ModOp::Advantage => score = score.saturating_add(1),
                ModOp::Disadvantage => score = score.saturating_sub(1),
                ModOp::Add | ModOp::Set => {}
            }
        }
        score.signum()
    }

    /// Расшифровка: `[("Плащ эльфов", 2), ("Усталость", -1)]`.
    #[must_use]
    pub fn breakdown(&self, target: ModTarget) -> Vec<(String, i32)> {
        self.items
            .iter()
            .filter(|item| item.target == target)
            .filter(|item| matches!(item.op, ModOp::Add | ModOp::Set))
            .map(|item| (item.source.clone(), i32::from(item.value)))
            .collect()
    }
}

/// Сколько длится эффект.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Duration {
    UntilEndOfTurn,
    Rounds(u16),
    Minutes(u16),
    Hours(u16),
    Concentration,
    WhileEquipped,
    Permanent,
}

/// Активный эффект на персонаже.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectInstance {
    pub id: EntityId,
    pub name: String,
    pub source: String,
    pub mods: ModifierSet,
    pub duration: Duration,
    pub concentration: bool,
    pub description: String,
}

// ---------------------------------------------------------------------------
// Расчёт с расшифровкой
// ---------------------------------------------------------------------------

/// Результат проверки: итог, из чего сложился и режим d20.
///
/// Именно это показывает UI в тултипе, чтобы ГМ видел «откуда +7».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    pub total: i32,
    pub parts: Vec<(String, i32)>,
    /// +1 преимущество, -1 помеха, 0 обычный бросок.
    pub advantage: i8,
}

impl Probe {
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            parts: Vec::new(),
            advantage: 0,
        }
    }

    pub fn add(&mut self, source: impl Into<String>, value: i32) {
        if value != 0 {
            self.parts.push((source.into(), value));
        }
        self.total += value;
    }

    #[must_use]
    pub fn describe(&self) -> String {
        let detail = self
            .parts
            .iter()
            .map(|(source, value)| format!("{source} {value:+}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mode = match self.advantage {
            1 => " [преимущество]",
            -1 => " [помеха]",
            _ => "",
        };
        if detail.is_empty() {
            format!("{:+}{mode}", self.total)
        } else {
            format!("{:+} ({detail}){mode}", self.total)
        }
    }
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Персонаж
// ---------------------------------------------------------------------------

/// Кто это: игровой персонаж, NPC или монстр.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    #[default]
    PlayerCharacter,
    Npc,
    Monster,
}

/// Персонаж или монстр: всё, что нужно для бросков и боя.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub id: EntityId,
    pub name: String,
    pub kind: ActorKind,
    pub level: u8,
    pub abilities: Abilities,
    pub prof: Proficiency,
    pub hp: Hp,
    /// Базовый КД без эффектов.
    pub base_ac: i32,
    pub speed_ft: i32,
    pub conditions: BTreeSet<Condition>,
    pub effects: Vec<EffectInstance>,
    /// Ресурсы с счётчиком: «Ярость», «Ки», «Вдохновение».
    pub resources: BTreeMap<String, i32>,
    /// Ссылка на ассет портрета (позже станет хэшем).
    pub image: Option<String>,
    pub notes: String,
}

impl Actor {
    #[must_use]
    pub fn new(id: EntityId, name: impl Into<String>, level: u8) -> Self {
        Self {
            id,
            name: name.into(),
            kind: ActorKind::PlayerCharacter,
            level,
            abilities: Abilities::default(),
            prof: Proficiency::default(),
            hp: Hp::new(10),
            base_ac: 10,
            speed_ft: 30,
            conditions: BTreeSet::new(),
            effects: Vec::new(),
            resources: BTreeMap::new(),
            image: None,
            notes: String::new(),
        }
    }

    #[must_use]
    pub fn proficiency_bonus(&self) -> i32 {
        rules::proficiency_bonus(self.level)
    }

    /// Уровень истощения (0 — нет).
    #[must_use]
    pub fn exhaustion(&self) -> u8 {
        self.conditions
            .iter()
            .filter_map(|c| c.exhaustion_level())
            .max()
            .unwrap_or(0)
    }

    /// Все модификаторы от активных эффектов.
    #[must_use]
    pub fn modifiers(&self) -> ModifierSet {
        let mut set = ModifierSet::new();
        for effect in &self.effects {
            set.items.extend(effect.mods.items.iter().cloned());
        }
        set
    }

    fn apply_effect_mods(&self, probe: &mut Probe, target: ModTarget) {
        let mods = self.modifiers();
        for (source, value) in mods.breakdown(target) {
            probe.add(source, value);
        }
        probe.advantage = mods.advantage(target);
    }

    fn apply_exhaustion(&self, probe: &mut Probe) {
        let level = self.exhaustion();
        if level > 0 {
            probe.add("Истощение", rules::exhaustion_penalty(level));
        }
    }

    /// Спасбросок по характеристике.
    #[must_use]
    pub fn save(&self, ability: Ability) -> Probe {
        let mut probe = Probe::new();
        probe.add(ability.name_ru(), self.abilities.modifier(ability));
        if self.prof.save_proficient(ability) {
            probe.add("Мастерство", self.proficiency_bonus());
        }
        self.apply_exhaustion(&mut probe);
        self.apply_effect_mods(&mut probe, ModTarget::Save(ability));
        probe
    }

    /// Проверка навыка.
    #[must_use]
    pub fn skill(&self, skill: Skill) -> Probe {
        let ability = skill.ability();
        let mut probe = Probe::new();
        probe.add(ability.name_ru(), self.abilities.modifier(ability));
        let level = self.prof.skill_level(skill);
        if level != ProfLevel::None {
            probe.add(
                if level == ProfLevel::Expertise {
                    "Экспертиза"
                } else {
                    "Мастерство"
                },
                self.proficiency_bonus() * level.multiplier(),
            );
        }
        self.apply_exhaustion(&mut probe);
        self.apply_effect_mods(&mut probe, ModTarget::Skill(skill));
        probe
    }

    /// Пассивное значение (10 + бонус) — пассивная внимательность и т.п.
    #[must_use]
    pub fn passive(&self, skill: Skill) -> i32 {
        rules::passive_score(self.skill(skill).total)
    }

    #[must_use]
    pub fn initiative(&self) -> Probe {
        let mut probe = Probe::new();
        probe.add("Ловкость", self.abilities.modifier(Ability::Dex));
        self.apply_effect_mods(&mut probe, ModTarget::Initiative);
        probe
    }

    /// Бонус атаки оружием/заклинанием, привязанным к характеристике.
    #[must_use]
    pub fn attack_bonus(&self, ability: Ability) -> Probe {
        let mut probe = Probe::new();
        probe.add(ability.name_ru(), self.abilities.modifier(ability));
        probe.add("Мастерство", self.proficiency_bonus());
        self.apply_effect_mods(&mut probe, ModTarget::Attack);
        probe
    }

    #[must_use]
    pub fn spell_save_dc(&self, ability: Ability) -> i32 {
        rules::spell_save_dc(self.proficiency_bonus(), self.abilities.modifier(ability))
    }

    #[must_use]
    pub fn armor_class(&self) -> Probe {
        let mut probe = Probe::new();
        probe.total = self.base_ac;
        if self.base_ac != 0 {
            probe.parts.push(("Базовый КД".to_string(), self.base_ac));
        }
        self.apply_effect_mods(&mut probe, ModTarget::Ac);
        probe
    }

    #[must_use]
    pub fn has_condition(&self, condition: Condition) -> bool {
        self.conditions.contains(&condition)
    }

    #[must_use]
    pub fn is_incapacitated(&self) -> bool {
        self.conditions.iter().any(|c| c.is_incapacitating())
    }

    /// Добавить/убрать уровень истощения (0 — убрать).
    pub fn set_exhaustion(&mut self, level: u8) {
        self.conditions.retain(|c| c.exhaustion_level().is_none());
        if level > 0 {
            self.conditions.insert(Condition::Exhaustion(level.min(6)));
        }
    }
}

// ---------------------------------------------------------------------------
// Токены
// ---------------------------------------------------------------------------

/// Размер токена (5e).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSize {
    Tiny,
    Small,
    #[default]
    Medium,
    Large,
    Huge,
    Gargantuan,
}

impl TokenSize {
    /// Сколько клеток занимает сторона токена.
    #[must_use]
    pub fn cells(self) -> f32 {
        match self {
            Self::Tiny => 0.5,
            Self::Small | Self::Medium => 1.0,
            Self::Large => 2.0,
            Self::Huge => 3.0,
            Self::Gargantuan => 4.0,
        }
    }

    /// Размер в футах (ширина).
    #[must_use]
    pub fn feet(self) -> f32 {
        self.cells() * 5.0
    }

    /// Следующий размер по кругу (для кнопки «увеличить»).
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Tiny => Self::Small,
            Self::Small => Self::Medium,
            Self::Medium => Self::Large,
            Self::Large => Self::Huge,
            Self::Huge | Self::Gargantuan => Self::Gargantuan,
        }
    }

    /// Предыдущий размер по кругу.
    #[must_use]
    pub fn previous(self) -> Self {
        match self {
            Self::Tiny | Self::Small => Self::Tiny,
            Self::Medium => Self::Small,
            Self::Large => Self::Medium,
            Self::Huge => Self::Large,
            Self::Gargantuan => Self::Huge,
        }
    }
}

/// Слой сцены — определяет порядок отрисовки и доступность игрокам.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    #[default]
    Tokens,
    /// Декор под токенами.
    Map,
    /// Видно только ГМу.
    Gm,
    /// Объекты над токенами (кроны, крыши).
    Foreground,
}

/// Полоска над токеном (хиты, КД и что угодно ещё).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub value: i32,
    pub max: i32,
    pub color: [u8; 3],
}

impl Bar {
    #[must_use]
    pub const fn new(value: i32, max: i32) -> Self {
        Self {
            value,
            max,
            color: [200, 40, 40],
        }
    }
}

/// Зрение токена.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vision {
    pub range_ft: f32,
    pub darkvision_ft: f32,
}

/// Источник света на токене.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Light {
    /// Радиус яркого света.
    pub bright_ft: f32,
    /// Радиус тусклого света (включая яркий).
    pub dim_ft: f32,
    pub color: [u8; 3],
}

/// Рамка токена (как в «конструкторе токенов»: без рамки, кольцо, двойное, пунктир).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenFrame {
    #[default]
    None,
    Ring,
    Double,
    Dashed,
}

impl TokenFrame {
    pub const ALL: [TokenFrame; 4] = [
        TokenFrame::None,
        TokenFrame::Ring,
        TokenFrame::Double,
        TokenFrame::Dashed,
    ];

    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::None => "без рамки",
            Self::Ring => "кольцо",
            Self::Double => "двойное",
            Self::Dashed => "пунктир",
        }
    }
}

/// Цвет рамки по умолчанию (светло-серый).
fn default_frame_color() -> [u8; 3] {
    [235, 235, 240]
}

/// Прозрачность токена по умолчанию.
fn default_opacity() -> f32 {
    1.0
}

/// Токен — то, что стоит на карте.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Token {
    pub id: EntityId,
    pub name: String,
    pub pos: Vec2,
    pub size: TokenSize,
    pub rotation: f32,
    pub layer: Layer,
    /// Ссылка на изображение (позже — хэш ассета).
    pub image: Option<String>,
    /// Рамка вокруг токена.
    #[serde(default)]
    pub frame: TokenFrame,
    /// Цвет рамки.
    #[serde(default = "default_frame_color")]
    pub frame_color: [u8; 3],
    /// Прозрачность токена (0.05..1.0).
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Кому принадлежит (кто может двигать).
    pub owner: Option<PlayerId>,
    /// Привязка к персонажу.
    pub actor: Option<EntityId>,
    /// Привязка к листу SCP-персонажа (см. [`crate::scp`]).
    #[serde(default)]
    pub character: Option<EntityId>,
    pub hp: Option<Bar>,
    pub ac: Option<Bar>,
    pub conditions: BTreeSet<Condition>,
    /// Название эффекта/заклинания, на котором держится концентрация.
    #[serde(default)]
    pub concentration: Option<String>,
    /// Спасброски от смерти (актуальны при 0 хитов).
    #[serde(default)]
    pub death_saves: DeathSaves,
    /// Мёртв (три провала спасбросков от смерти).
    #[serde(default)]
    pub dead: bool,
    /// Скрыт от игроков.
    pub hidden: bool,
    /// Запрещён к перемещению игроками.
    pub locked: bool,
    pub vision: Option<Vision>,
    pub light: Option<Light>,
    pub note: Option<String>,
}

impl Token {
    #[must_use]
    pub fn new(id: EntityId, name: impl Into<String>, pos: Vec2) -> Self {
        Self {
            id,
            name: name.into(),
            pos,
            size: TokenSize::Medium,
            rotation: 0.0,
            layer: Layer::Tokens,
            image: None,
            frame: TokenFrame::None,
            frame_color: default_frame_color(),
            opacity: default_opacity(),
            owner: None,
            actor: None,
            character: None,
            hp: None,
            ac: None,
            conditions: BTreeSet::new(),
            concentration: None,
            death_saves: DeathSaves::default(),
            dead: false,
            hidden: false,
            locked: false,
            vision: None,
            light: None,
            note: None,
        }
    }

    /// Синхронизировать полоски с персонажем (HP, КД, состояния).
    pub fn sync_from_actor(&mut self, actor: &Actor) {
        self.hp = Some(Bar::new(actor.hp.current, actor.hp.max));
        self.ac = Some(Bar::new(actor.base_ac, actor.base_ac));
        self.conditions = actor.conditions.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fighter() -> Actor {
        let mut actor = Actor::new(EntityId(1), "Торин", 5);
        actor.abilities = Abilities::new(16, 14, 14, 10, 12, 8);
        actor.prof.saves.insert(Ability::Str);
        actor.prof.saves.insert(Ability::Con);
        actor
            .prof
            .set_skill(Skill::Athletics, ProfLevel::Proficient);
        actor
            .prof
            .set_skill(Skill::Perception, ProfLevel::Expertise);
        actor
    }

    #[test]
    fn saves_include_proficiency() {
        let actor = fighter();
        // STR 16 → +3, мастерство 5 уровня → +3.
        assert_eq!(actor.save(Ability::Str).total, 6);
        // DEX 14 → +2 и без мастерства.
        assert_eq!(actor.save(Ability::Dex).total, 2);
    }

    #[test]
    fn skills_include_expertise() {
        let actor = fighter();
        // WIS 12 → +1, экспертиза → +3×2 = +6.
        assert_eq!(actor.skill(Skill::Perception).total, 7);
        // STR 16 → +3, мастерство → +3.
        assert_eq!(actor.skill(Skill::Athletics).total, 6);
        assert_eq!(actor.passive(Skill::Perception), 17);
    }

    #[test]
    fn exhaustion_penalises_checks() {
        let mut actor = fighter();
        actor.set_exhaustion(2);
        // STR +3, мастерство +3, истощение −2.
        assert_eq!(actor.skill(Skill::Athletics).total, 4);
        assert_eq!(actor.exhaustion(), 2);
    }

    #[test]
    fn effects_add_and_explain() {
        let mut actor = fighter();
        actor.effects.push(EffectInstance {
            id: EntityId(99),
            name: "Плащ эльфов".into(),
            source: "Плащ эльфов".into(),
            mods: {
                let mut set = ModifierSet::new();
                set.push(Modifier {
                    target: ModTarget::Skill(Skill::Stealth),
                    op: ModOp::Add,
                    value: 2,
                    source: "Плащ эльфов".into(),
                });
                set.push(Modifier {
                    target: ModTarget::Skill(Skill::Stealth),
                    op: ModOp::Advantage,
                    value: 0,
                    source: "Плащ эльфов".into(),
                });
                set
            },
            duration: Duration::WhileEquipped,
            concentration: false,
            description: String::new(),
        });

        let stealth = actor.skill(Skill::Stealth);
        // DEX 14 → +2, плащ +2.
        assert_eq!(stealth.total, 4);
        assert_eq!(stealth.advantage, 1);
        assert!(stealth.describe().contains("Плащ эльфов"));
    }

    #[test]
    fn modifier_set_handles_set_and_advantage() {
        let mut set = ModifierSet::new();
        set.push(Modifier {
            target: ModTarget::Ac,
            op: ModOp::Add,
            value: 2,
            source: "Щит".into(),
        });
        set.push(Modifier {
            target: ModTarget::Ac,
            op: ModOp::Set,
            value: 18,
            source: "Доспех".into(),
        });
        // Set перекрывает накопленную сумму.
        assert_eq!(set.total(ModTarget::Ac), 18);
        assert_eq!(set.advantage(ModTarget::Ac), 0);
    }

    #[test]
    fn hp_damage_uses_temp_first() {
        let mut hp = Hp::new(20);
        hp.temp = 5;
        hp.damage(8);
        assert_eq!(hp.temp, 0);
        assert_eq!(hp.current, 17);
        hp.heal(100);
        assert_eq!(hp.current, 20, "нельзя вылечиться выше максимума");
        hp.damage(100);
        assert_eq!(hp.current, 0);
        assert!(hp.is_down());
    }

    #[test]
    fn bloodied_flag() {
        let mut hp = Hp::new(20);
        assert!(!hp.is_bloodied());
        hp.damage(10);
        assert!(hp.is_bloodied());
    }

    #[test]
    fn conditions_have_expected_flags() {
        assert!(Condition::Paralyzed.is_incapacitating());
        assert!(Condition::Prone.grants_advantage_to_attackers());
        assert!(Condition::Unconscious.auto_fails_str_dex_saves());
        assert_eq!(Condition::Exhaustion(3).exhaustion_level(), Some(3));
        assert!(!Condition::Poisoned.is_incapacitating());
    }

    #[test]
    fn armor_class_and_attack_bonus() {
        let mut actor = fighter();
        actor.base_ac = 16;
        assert_eq!(actor.armor_class().total, 16);
        // STR 16 → +3, мастерство 5 → +3.
        assert_eq!(actor.attack_bonus(Ability::Str).total, 6);
        // Мастерство 5, INT +0 → СЛ 8 + 3 + 0 = 11.
        assert_eq!(actor.spell_save_dc(Ability::Int), 11);
    }

    #[test]
    fn token_sizes_in_cells() {
        assert_eq!(TokenSize::Tiny.cells(), 0.5);
        assert_eq!(TokenSize::Medium.cells(), 1.0);
        assert_eq!(TokenSize::Large.cells(), 2.0);
        assert_eq!(TokenSize::Gargantuan.cells(), 4.0);
        assert_eq!(TokenSize::Huge.feet(), 15.0);
    }

    #[test]
    fn token_syncs_from_actor() {
        let mut actor = fighter();
        actor.hp.damage(5);
        actor.conditions.insert(Condition::Prone);
        let mut token = Token::new(EntityId(2), "Торин", Vec2::ZERO);
        token.sync_from_actor(&actor);
        assert_eq!(token.hp.unwrap().value, 5);
        assert!(token.conditions.contains(&Condition::Prone));
    }

    #[test]
    fn death_save_counters_work() {
        let mut saves = DeathSaves::default();
        assert!(!saves.is_stable() && !saves.is_dead());
        saves.successes = 3;
        assert!(saves.is_stable());
        saves.reset();
        saves.failures = 3;
        assert!(saves.is_dead());
        saves.reset();
        assert_eq!(saves, DeathSaves::default());
    }

    #[test]
    fn token_json_roundtrip_keeps_new_fields() {
        let mut token = Token::new(EntityId(2), "Торин", Vec2::ZERO);
        token.concentration = Some("Благословение".into());
        token.death_saves.failures = 1;
        token.dead = false;
        let json = serde_json::to_string(&token).unwrap();
        let back: Token = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
        assert_eq!(back.concentration.as_deref(), Some("Благословение"));
        assert_eq!(back.death_saves.failures, 1);
    }

    #[test]
    fn old_token_json_loads_with_defaults() {
        // Совместимость: старая кампания без новых полей должна читаться.
        let json = r#"{
            "id": 5, "name": "Старый", "pos": {"x": 0.0, "y": 0.0},
            "size": "medium", "rotation": 0.0, "layer": "tokens",
            "image": null, "owner": null, "actor": null, "hp": null, "ac": null,
            "conditions": [], "hidden": false, "locked": false,
            "vision": null, "light": null, "note": null
        }"#;
        let token: Token = serde_json::from_str(json).expect("старый формат должен читаться");
        assert!(token.concentration.is_none());
        assert!(!token.dead);
        assert_eq!(token.death_saves, DeathSaves::default());
    }

    #[test]
    fn actor_json_roundtrip() {
        let actor = fighter();
        let json = serde_json::to_string(&actor).unwrap();
        let back: Actor = serde_json::from_str(&json).unwrap();
        assert_eq!(actor, back);
    }

    #[test]
    fn abilities_json_uses_str_key() {
        let abilities = Abilities::new(1, 2, 3, 4, 5, 6);
        let json = serde_json::to_string(&abilities).unwrap();
        assert!(json.contains("\"str\":1"), "{json}");
    }
}
