//! Компендиум: заклинания, предметы, монстры, пакеты контента.
//!
//! ## Лицензионная позиция
//!
//! В репозиторий **не кладётся** закрытый контент Wizards of the Coast.
//! Разрешено использовать только SRD 5.1 / SRD 5.2 (CC-BY-4.0) с указанием
//! источника, а также собственные и пользовательские данные.
//!
//! ## Как устроено
//!
//! Контент приходит **паками** ([`ContentPack`]) — JSON-файлами с манифестом
//! источника. Пак загружается в [`ContentRegistry`], который живёт в кампании
//! и сохраняется вместе с ней.
//!
//! Реестр намеренно простой: векторы и линейный поиск. Пока записей сотни,
//! а не десятки тысяч, это быстрее и понятнее любого индекса; FTS появится
//! вместе с SQLite.

use crate::entities::Abilities;
use crate::ids::EntityId;
use serde::{Deserialize, Serialize};

/// Откуда пришёл контент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentSource {
    /// Машинный идентификатор: `srd-5.2`, `homebrew`, `my-campaign`.
    pub id: String,
    /// Человекочитаемое имя.
    pub name: String,
    /// Лицензия/правовой статус: `CC-BY-4.0`, `private`, …
    pub license: String,
    /// Обязательная атрибуция, если требуется лицензией.
    #[serde(default)]
    pub attribution: String,
}

impl ContentSource {
    #[must_use]
    pub fn srd() -> Self {
        Self {
            id: "srd-5.2".to_string(),
            name: "SRD 5.2 (базовые правила)".to_string(),
            license: "CC-BY-4.0".to_string(),
            attribution: "Rules content based on the System Reference Document 5.2 \
                          by Wizards of the Coast LLC, licensed under CC-BY-4.0."
                .to_string(),
        }
    }

    #[must_use]
    pub fn private(name: impl Into<String>) -> Self {
        Self {
            id: "private".to_string(),
            name: name.into(),
            license: "private".to_string(),
            attribution: String::new(),
        }
    }
}

/// Заклинание.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spell {
    pub id: EntityId,
    pub name: String,
    /// 0 — заговор.
    pub level: u8,
    pub school: String,
    pub casting_time: String,
    pub range: String,
    pub components: String,
    pub duration: String,
    pub concentration: bool,
    pub ritual: bool,
    pub description: String,
    pub classes: Vec<String>,
    pub source: String,
}

/// Предмет.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: EntityId,
    pub name: String,
    pub kind: String,
    pub rarity: Option<String>,
    pub weight_lb: f32,
    pub cost_gp: f32,
    pub requires_attunement: bool,
    pub description: String,
    pub source: String,
}

/// Монстр/статблок.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Monster {
    pub id: EntityId,
    pub name: String,
    pub armor_class: i32,
    pub hit_points: i32,
    /// Формула хитов, например `2d6+2`.
    pub hit_dice: String,
    pub speed_ft: i32,
    pub abilities: Abilities,
    /// Показатель опасности (CR), например 0.25.
    pub challenge_rating: f32,
    /// Бонус мастерства.
    pub proficiency_bonus: i32,
    pub traits: Vec<String>,
    pub actions: Vec<String>,
    pub source: String,
}

// ---------------------------------------------------------------------------
// Паки контента (JSON)
// ---------------------------------------------------------------------------

/// Пак контента: манифест источника + записи без идентификаторов.
///
/// Идентификаторы выдаёт реестр при загрузке, поэтому одни и те же файлы можно
/// подключать в разные кампании.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentPack {
    pub source: ContentSource,
    #[serde(default)]
    pub monsters: Vec<MonsterDraft>,
    #[serde(default)]
    pub spells: Vec<SpellDraft>,
    #[serde(default)]
    pub items: Vec<ItemDraft>,
}

/// Монстр в паке (без `id` и `source`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonsterDraft {
    pub name: String,
    pub armor_class: i32,
    pub hit_points: i32,
    #[serde(default)]
    pub hit_dice: String,
    #[serde(default = "default_speed")]
    pub speed_ft: i32,
    pub abilities: Abilities,
    #[serde(default)]
    pub challenge_rating: f32,
    #[serde(default = "default_proficiency")]
    pub proficiency_bonus: i32,
    #[serde(default)]
    pub traits: Vec<String>,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// Заклинание в паке.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellDraft {
    pub name: String,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub school: String,
    #[serde(default)]
    pub casting_time: String,
    #[serde(default)]
    pub range: String,
    #[serde(default)]
    pub components: String,
    #[serde(default)]
    pub duration: String,
    #[serde(default)]
    pub concentration: bool,
    #[serde(default)]
    pub ritual: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub classes: Vec<String>,
}

/// Предмет в паке.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemDraft {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default)]
    pub weight_lb: f32,
    #[serde(default)]
    pub cost_gp: f32,
    #[serde(default)]
    pub requires_attunement: bool,
    #[serde(default)]
    pub description: String,
}

fn default_speed() -> i32 {
    30
}

fn default_proficiency() -> i32 {
    2
}

/// Сколько записей добавил пак.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackStats {
    pub monsters: usize,
    pub spells: usize,
    pub items: usize,
}

impl PackStats {
    #[must_use]
    pub fn total(&self) -> usize {
        self.monsters + self.spells + self.items
    }
}

/// Вид записи в результатах поиска.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Spell,
    Item,
    Monster,
}

/// Результат поиска по компендиуму.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: EntityId,
    pub kind: ContentKind,
    pub name: String,
    /// Короткая подпись: «заклинание 3 уровня» или «CR 0.25».
    pub subtitle: String,
}

/// Реестр контента кампании.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContentRegistry {
    /// Следующий свободный идентификатор записи.
    #[serde(default = "next_start")]
    pub next_entity_id: u64,
    #[serde(default)]
    pub sources: Vec<ContentSource>,
    #[serde(default)]
    pub spells: Vec<Spell>,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub monsters: Vec<Monster>,
}

fn next_start() -> u64 {
    1
}

impl ContentRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id.max(1));
        self.next_entity_id = id.0 + 1;
        id
    }

    /// Загрузить пак: выдаёт идентификаторы, проставляет `source` и атрибуцию.
    pub fn add_pack(&mut self, pack: ContentPack) -> PackStats {
        let source_id = pack.source.id.clone();
        let mut stats = PackStats::default();

        if !self
            .sources
            .iter()
            .any(|existing| existing.id == pack.source.id)
        {
            self.sources.push(pack.source);
        }

        for draft in pack.monsters {
            let id = self.next_id();
            self.monsters.push(Monster {
                id,
                name: draft.name,
                armor_class: draft.armor_class,
                hit_points: draft.hit_points,
                hit_dice: draft.hit_dice,
                speed_ft: draft.speed_ft,
                abilities: draft.abilities,
                challenge_rating: draft.challenge_rating,
                proficiency_bonus: draft.proficiency_bonus,
                traits: draft.traits,
                actions: draft.actions,
                source: source_id.clone(),
            });
            stats.monsters += 1;
        }

        for draft in pack.spells {
            let id = self.next_id();
            self.spells.push(Spell {
                id,
                name: draft.name,
                level: draft.level,
                school: draft.school,
                casting_time: draft.casting_time,
                range: draft.range,
                components: draft.components,
                duration: draft.duration,
                concentration: draft.concentration,
                ritual: draft.ritual,
                description: draft.description,
                classes: draft.classes,
                source: source_id.clone(),
            });
            stats.spells += 1;
        }

        for draft in pack.items {
            let id = self.next_id();
            self.items.push(Item {
                id,
                name: draft.name,
                kind: draft.kind,
                rarity: draft.rarity,
                weight_lb: draft.weight_lb,
                cost_gp: draft.cost_gp,
                requires_attunement: draft.requires_attunement,
                description: draft.description,
                source: source_id.clone(),
            });
            stats.items += 1;
        }

        stats
    }

    #[must_use]
    pub fn spell(&self, id: EntityId) -> Option<&Spell> {
        self.spells.iter().find(|s| s.id == id)
    }

    #[must_use]
    pub fn item(&self, id: EntityId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    #[must_use]
    pub fn monster(&self, id: EntityId) -> Option<&Monster> {
        self.monsters.iter().find(|m| m.id == id)
    }

    /// Поиск монстра по точному имени (для команд вроде `spawn гоблин`).
    #[must_use]
    pub fn monster_by_name(&self, name: &str) -> Option<&Monster> {
        let needle = name.trim().to_lowercase();
        self.monsters
            .iter()
            .find(|m| m.name.to_lowercase() == needle)
    }

    /// Поиск по подстроке имени без учёта регистра (работает и для кириллицы).
    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }

        let mut hits = Vec::new();
        for spell in &self.spells {
            if spell.name.to_lowercase().contains(&needle) {
                hits.push(SearchHit {
                    id: spell.id,
                    kind: ContentKind::Spell,
                    name: spell.name.clone(),
                    subtitle: format!("заклинание {} уровня", spell.level),
                });
            }
        }
        for item in &self.items {
            if item.name.to_lowercase().contains(&needle) {
                hits.push(SearchHit {
                    id: item.id,
                    kind: ContentKind::Item,
                    name: item.name.clone(),
                    subtitle: item.kind.clone(),
                });
            }
        }
        for monster in &self.monsters {
            if monster.name.to_lowercase().contains(&needle) {
                hits.push(SearchHit {
                    id: monster.id,
                    kind: ContentKind::Monster,
                    name: monster.name.clone(),
                    subtitle: format!("CR {:.2}", monster.challenge_rating),
                });
            }
        }

        hits.sort_by(|a, b| a.name.cmp(&b.name));
        hits.truncate(limit);
        hits
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spells.is_empty() && self.items.is_empty() && self.monsters.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.spells.len() + self.items.len() + self.monsters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_pack() -> ContentPack {
        ContentPack {
            source: ContentSource::srd(),
            monsters: vec![MonsterDraft {
                name: "Гоблин".into(),
                armor_class: 15,
                hit_points: 7,
                hit_dice: "2d6".into(),
                speed_ft: 30,
                abilities: Abilities::new(8, 14, 10, 10, 8, 8),
                challenge_rating: 0.25,
                proficiency_bonus: 2,
                traits: vec!["Трусливое бегство".into()],
                actions: vec!["Скимитар".into()],
            }],
            spells: vec![SpellDraft {
                name: "Огненный снаряд".into(),
                level: 1,
                school: "воплощение".into(),
                casting_time: "1 действие".into(),
                range: "120 футов".into(),
                components: "В, С".into(),
                duration: "Мгновенно".into(),
                concentration: false,
                ritual: false,
                description: String::new(),
                classes: vec!["волшебник".into()],
            }],
            items: vec![ItemDraft {
                name: "Верёвка".into(),
                kind: "снаряжение".into(),
                rarity: None,
                weight_lb: 10.0,
                cost_gp: 1.0,
                requires_attunement: false,
                description: String::new(),
            }],
        }
    }

    #[test]
    fn pack_loads_and_assigns_ids() {
        let mut registry = ContentRegistry::new();
        let stats = registry.add_pack(demo_pack());
        assert_eq!(stats.total(), 3);
        assert_eq!(stats.monsters, 1);
        assert_eq!(registry.len(), 3);
        assert_eq!(registry.sources.len(), 1);

        let monster = &registry.monsters[0];
        assert!(!monster.id.is_none());
        assert_eq!(monster.source, "srd-5.2");
        assert_eq!(monster.hit_dice, "2d6");
    }

    #[test]
    fn ids_are_unique_across_packs() {
        let mut registry = ContentRegistry::new();
        registry.add_pack(demo_pack());
        registry.add_pack(demo_pack());

        let mut ids: Vec<u64> = Vec::new();
        ids.extend(registry.spells.iter().map(|s| s.id.0));
        ids.extend(registry.items.iter().map(|i| i.id.0));
        ids.extend(registry.monsters.iter().map(|m| m.id.0));
        let unique: std::collections::BTreeSet<u64> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
        assert!(ids.iter().all(|id| *id > 0));
    }

    #[test]
    fn duplicate_source_not_added_twice() {
        let mut registry = ContentRegistry::new();
        registry.add_pack(demo_pack());
        registry.add_pack(demo_pack());
        assert_eq!(registry.sources.len(), 1);
        assert_eq!(registry.len(), 6);
    }

    #[test]
    fn search_is_case_insensitive_and_finds_all_kinds() {
        let mut registry = ContentRegistry::new();
        registry.add_pack(demo_pack());
        assert_eq!(registry.search("огн", 10).len(), 1);
        assert_eq!(registry.search("ГОБ", 10).len(), 1);
        assert_eq!(registry.search("вер", 10).len(), 1);
        assert!(registry.search("нет-такого", 10).is_empty());
        assert!(registry.search("   ", 10).is_empty());
    }

    #[test]
    fn monster_lookup_by_name() {
        let mut registry = ContentRegistry::new();
        registry.add_pack(demo_pack());
        assert!(registry.monster_by_name("гоблин").is_some());
        assert!(registry.monster_by_name("  Гоблин  ").is_some());
        assert!(registry.monster_by_name("дракон").is_none());
    }

    #[test]
    fn registry_json_roundtrip() {
        let mut registry = ContentRegistry::new();
        registry.add_pack(demo_pack());
        let json = serde_json::to_string(&registry).unwrap();
        let back: ContentRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(registry, back);
        // После загрузки нумерация продолжается, а не начинается заново.
        let mut back = back;
        let before = back.next_entity_id;
        let _ = back.next_id();
        assert_eq!(back.next_entity_id, before + 1);
    }

    #[test]
    fn empty_pack_is_harmless() {
        let mut registry = ContentRegistry::new();
        let stats = registry.add_pack(ContentPack {
            source: ContentSource::private("Моя кампания"),
            monsters: Vec::new(),
            spells: Vec::new(),
            items: Vec::new(),
        });
        assert_eq!(stats.total(), 0);
        assert!(registry.is_empty());
        assert_eq!(registry.sources.len(), 1);
    }
}
