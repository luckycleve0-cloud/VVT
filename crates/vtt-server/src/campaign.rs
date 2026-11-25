//! Кампания: сохраняемый набор сцен, персонажей и состояния боя.
//!
//! Кампания — это **данные**. Всё поведение — в [`crate::server::GameServer`].
//! Такой файл легко сохранить в JSON, отправить другу и положить в git.

use serde::{Deserialize, Serialize};
use vtt_core::combat::Combat;
use vtt_core::content::ContentRegistry;
use vtt_core::entities::Actor;
use vtt_core::ids::{EntityId, PlayerId};
use vtt_core::scene::Scene;
use vtt_core::scp::ScpCharacter;

/// Хендаут — материал, который ГМ показывает игрокам.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handout {
    pub title: String,
    pub text: String,
}

/// Одна кампания (может содержать несколько сцен).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub name: String,
    pub scenes: Vec<Scene>,
    pub active_scene: EntityId,
    pub actors: Vec<Actor>,
    pub combat: Combat,
    /// Следующий свободный идентификатор (чтобы не переиспользовать удалённые).
    pub next_entity_id: u64,
    /// Подключённые паки контента (монстры, заклинания, предметы).
    #[serde(default)]
    pub content: ContentRegistry,
    /// Выданные хендауты.
    #[serde(default)]
    pub handouts: Vec<Handout>,
    /// Персонажи по системе SCP (лист персонажа).
    #[serde(default)]
    pub characters: Vec<ScpCharacter>,
}

impl Default for Campaign {
    fn default() -> Self {
        Self::new("Новая кампания")
    }
}

impl Campaign {
    /// Новая кампания с одной пустой сценой.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let mut campaign = Self {
            name: name.into(),
            scenes: Vec::new(),
            active_scene: EntityId::NONE,
            actors: Vec::new(),
            combat: Combat::default(),
            next_entity_id: 1,
            content: ContentRegistry::new(),
            handouts: Vec::new(),
            characters: Vec::new(),
        };
        let scene_id = campaign.next_id();
        campaign.scenes.push(Scene::new(scene_id, "Сцена 1"));
        campaign.active_scene = scene_id;
        campaign
    }

    /// Выдать новый идентификатор сущности.
    pub fn next_id(&mut self) -> EntityId {
        let id = EntityId(self.next_entity_id);
        self.next_entity_id += 1;
        id
    }

    /// Активная сцена.
    ///
    /// # Panics
    /// Паникует, если кампания битая: сцен нет или `active_scene` не найден.
    /// Инвариант поддерживается конструктором и [`Self::set_active_scene`].
    #[must_use]
    pub fn scene(&self) -> &Scene {
        self.scene_by_id(self.active_scene)
            .expect("инвариант: active_scene существует")
    }

    /// Активная сцена для изменения.
    ///
    /// # Panics
    /// См. [`Self::scene`].
    pub fn scene_mut(&mut self) -> &mut Scene {
        let id = self.active_scene;
        self.scenes
            .iter_mut()
            .find(|s| s.id == id)
            .expect("инвариант: active_scene существует")
    }

    #[must_use]
    pub fn scene_by_id(&self, id: EntityId) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == id)
    }

    pub fn set_active_scene(&mut self, id: EntityId) -> bool {
        if self.scene_by_id(id).is_some() {
            self.active_scene = id;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn actor(&self, id: EntityId) -> Option<&Actor> {
        self.actors.iter().find(|a| a.id == id)
    }

    pub fn actor_mut(&mut self, id: EntityId) -> Option<&mut Actor> {
        self.actors.iter_mut().find(|a| a.id == id)
    }

    /// Добавить персонажа; `id` выдаётся кампанией.
    pub fn add_actor(&mut self, mut actor: Actor) -> EntityId {
        actor.id = self.next_id();
        let id = actor.id;
        self.actors.push(actor);
        id
    }

    /// Добавить сцену.
    pub fn add_scene(&mut self, name: impl Into<String>) -> EntityId {
        let id = self.next_id();
        self.scenes.push(Scene::new(id, name));
        id
    }

    #[must_use]
    pub fn tokens_count(&self) -> usize {
        self.scenes.iter().map(|s| s.tokens.len()).sum()
    }

    /// Добавить хендаут в историю кампании.
    pub fn add_handout(&mut self, title: impl Into<String>, text: impl Into<String>) -> &Handout {
        self.handouts.push(Handout {
            title: title.into(),
            text: text.into(),
        });
        self.handouts.last().expect("только что добавлен")
    }

    #[must_use]
    pub fn character(&self, id: EntityId) -> Option<&ScpCharacter> {
        self.characters.iter().find(|character| character.id == id)
    }

    pub fn character_mut(&mut self, id: EntityId) -> Option<&mut ScpCharacter> {
        self.characters
            .iter_mut()
            .find(|character| character.id == id)
    }

    /// Добавить персонажа; `id` выдаётся кампанией.
    pub fn add_character(&mut self, mut character: ScpCharacter) -> EntityId {
        character.id = self.next_id();
        let id = character.id;
        self.characters.push(character);
        id
    }

    /// Персонажи, принадлежащие игроку.
    #[must_use]
    pub fn characters_of(&self, player: PlayerId) -> Vec<&ScpCharacter> {
        self.characters
            .iter()
            .filter(|character| character.owner == Some(player))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtt_core::entities::Actor;

    #[test]
    fn new_campaign_has_one_scene() {
        let campaign = Campaign::new("Тест");
        assert_eq!(campaign.scenes.len(), 1);
        assert_eq!(campaign.scene().name, "Сцена 1");
        assert_eq!(campaign.active_scene, campaign.scene().id);
    }

    #[test]
    fn ids_are_monotonic() {
        let mut campaign = Campaign::new("Тест");
        let a = campaign.next_id();
        let b = campaign.next_id();
        assert!(b.0 > a.0);
        assert_eq!(campaign.next_entity_id, b.0 + 1);
    }

    #[test]
    fn add_actor_assigns_id() {
        let mut campaign = Campaign::new("Тест");
        let id = campaign.add_actor(Actor::new(EntityId::NONE, "Герой", 1));
        assert_eq!(campaign.actor(id).unwrap().name, "Герой");
    }

    #[test]
    fn switching_scene_only_to_existing() {
        let mut campaign = Campaign::new("Тест");
        let second = campaign.add_scene("Подземелье");
        assert!(campaign.set_active_scene(second));
        assert_eq!(campaign.scene().name, "Подземелье");
        assert!(!campaign.set_active_scene(EntityId(9999)));
        assert_eq!(campaign.scene().name, "Подземелье");
    }

    #[test]
    fn campaign_json_roundtrip() {
        let mut campaign = Campaign::new("Кампания");
        campaign.add_actor(Actor::new(EntityId::NONE, "Маг", 3));
        let json = serde_json::to_string_pretty(&campaign).unwrap();
        let back: Campaign = serde_json::from_str(&json).unwrap();
        assert_eq!(campaign, back);
    }
}
