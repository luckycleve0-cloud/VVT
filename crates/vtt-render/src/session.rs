//! Локальная игровая сессия для окна: сервер + клиент ГМа в одном процессе.
//!
//! Окно не «владеет» состоянием: оно показывает `server.campaign.scene()` и
//! применяет действия через `server.handle(HOST_CLIENT, …)`. Ровно так же будет
//! работать сетевой ГМ — только сообщения поедут через транспорт.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use vtt_core::entities::{TokenFrame, TokenSize};
use vtt_core::geom::Vec2;
use vtt_core::ids::{EntityId, PlayerId};
use vtt_core::scene::Scene;
use vtt_core::scp::{AccessLevel, Attributes, ScpAttr, ScpCharacter, ScpClass, ScpSkill};
use vtt_core::visibility::{visible_cells, vision_radius_px};
use vtt_proto::{CharacterPatch, ClientId, ClientMsg, Event, Intent, ServerMsg};
use vtt_server::{
    campaign_exists, load_campaign, save_campaign, Campaign, GameServer, Outgoing, HOST_CLIENT,
};

use crate::interact::nearest_wall;

/// Время жизни пинга в секундах.
pub const PING_LIFETIME: f64 = 2.5;
/// Максимальная длина журнала в памяти.
pub const LOG_CAPACITY: usize = 300;

/// Пинг на карте.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ping {
    pub at: Vec2,
    pub born: f64,
}

/// Строка чата.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLine {
    pub from: String,
    pub text: String,
    pub to_gm: bool,
}

/// Карточка броска в ленте (как в Roll20).
#[derive(Debug, Clone, PartialEq)]
pub struct RollCard {
    pub actor: String,
    pub title: String,
    pub formula: String,
    pub total: i32,
    pub gm_only: bool,
    pub success: Option<bool>,
    pub note: Option<String>,
}

/// Элемент правой ленты.
#[derive(Debug, Clone, PartialEq)]
pub enum FeedItem {
    Text(String),
    Chat {
        from: String,
        text: String,
        to_gm: bool,
    },
    Roll(RollCard),
}

impl Ping {
    /// Прогресс 0..1 (для анимации).
    #[must_use]
    pub fn progress(&self, now: f64) -> f32 {
        let age = (now - self.born).max(0.0);
        (age / PING_LIFETIME).clamp(0.0, 1.0) as f32
    }
}

/// Кампания, открытая в окне.
#[derive(Debug)]
pub struct LocalSession {
    pub server: GameServer,
    pub dir: PathBuf,
    /// Путь к картинке карты (для загрузки текстуры).
    pub map_path: Option<PathBuf>,
    pub log: Vec<String>,
    pub pings: Vec<Ping>,
    /// Есть несохранённые изменения.
    pub dirty: bool,
    /// «Память исследования»: клетки, которые группа уже видела.
    pub explored: BTreeSet<(i32, i32)>,
    /// Клетки, видимые прямо сейчас (обновляется каждый кадр).
    pub visible: BTreeSet<(i32, i32)>,
    /// Есть ли у кого-то на сцене зрение (иначе туман не рисуем).
    pub fog_active: bool,
    /// Выбранный токен.
    pub selected_token: Option<EntityId>,
    /// Все выбранные токены (Ctrl+клик и рамка выделения).
    pub selection: BTreeSet<EntityId>,
    /// Открытый лист персонажа.
    pub selected_character: Option<EntityId>,
    /// Показывать панель листа.
    pub show_sheet: bool,
    /// Туман по клеткам (по умолчанию выключен — не мешает карте).
    pub show_fog: bool,
    /// История чата.
    pub chat: Vec<ChatLine>,
    /// Лента карточек: броски, чат, события.
    pub feed: Vec<FeedItem>,
}

impl LocalSession {
    /// Открыть (или создать) кампанию. `map` — картинка карты.
    pub fn open(dir: impl AsRef<Path>, map: Option<PathBuf>, name: &str) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let mut campaign = if campaign_exists(&dir) {
            load_campaign(&dir).with_context(|| format!("не удалось открыть {}", dir.display()))?
        } else {
            Campaign::new(name)
        };

        if let Some(map_path) = &map {
            // Пока ассеты — просто путь в JSON; хэши и копирование появятся в M6.
            campaign.scene_mut().background = Some(map_path.to_string_lossy().into_owned());
        }

        // Демонстрационный бестиарий: если кампания пустая — подключаем встроенный пак.
        if campaign.content.is_empty() {
            if let Ok(pack) = serde_json::from_str::<vtt_core::content::ContentPack>(include_str!(
                "../../../content/example-bestiary.json"
            )) {
                campaign.content.add_pack(pack);
            }
        }

        let mut server = GameServer::new(campaign);
        server.add_host("ГМ");

        Ok(Self {
            server,
            dir,
            map_path: map,
            log: Vec::new(),
            pings: Vec::new(),
            dirty: false,
            explored: BTreeSet::new(),
            visible: BTreeSet::new(),
            fog_active: false,
            selected_token: None,
            selection: BTreeSet::new(),
            selected_character: None,
            show_sheet: true,
            show_fog: false,
            chat: Vec::new(),
            feed: Vec::new(),
        })
    }

    #[must_use]
    pub fn scene(&self) -> &Scene {
        self.server.campaign.scene()
    }

    /// Центр сцены в мировых координатах.
    #[must_use]
    pub fn scene_center(&self) -> Vec2 {
        let scene = self.server.campaign.scene();
        Vec2::new(scene.width_px * 0.5, scene.height_px * 0.5)
    }

    /// Применить намерение ГМа; вернуть события для возможной логики окна.
    pub fn apply(&mut self, intent: Intent) -> Vec<Event> {
        let outgoing = self.server.handle(HOST_CLIENT, ClientMsg::Intent(intent));
        let mut events = Vec::new();
        for out in outgoing {
            match out.msg {
                ServerMsg::Events(list) => {
                    for event in &list {
                        self.record_event(event);
                    }
                    events.extend(list);
                }
                ServerMsg::Notice { text } => self.push_log(text),
                ServerMsg::Denied { reason } => self.push_log(format!("отказано: {reason}")),
                _ => {}
            }
        }
        self.dirty = true;
        events
    }

    /// Создать токен по имени монстра из компендиума.
    pub fn spawn_monster(&mut self, name: &str, pos: Vec2) -> bool {
        let Some(monster) = self.server.campaign.content.monster_by_name(name) else {
            return false;
        };
        let monster_id = monster.id;
        self.apply(Intent::CreateMonsterToken {
            monster: monster_id,
            pos,
        });
        true
    }

    /// Создать пустой токен.
    pub fn create_token(&mut self, name: &str, pos: Vec2) {
        self.apply(Intent::CreateToken {
            name: name.to_string(),
            pos,
            size: TokenSize::Medium,
        });
    }

    /// Удалить токен.
    pub fn delete_token(&mut self, token: EntityId) {
        self.apply(Intent::DeleteToken { token });
    }

    pub fn ping(&mut self, at: Vec2, now: f64) {
        self.pings.push(Ping { at, born: now });
        self.push_log(format!("пинг ({:.0}, {:.0})", at.x, at.y));
    }

    /// Убрать отжившие пинги.
    pub fn tick(&mut self, now: f64) {
        self.pings.retain(|ping| now - ping.born < PING_LIFETIME);
        self.update_visibility();
    }

    /// Пересчитать видимость и пополнить «память исследования».
    pub fn update_visibility(&mut self) {
        let (visible, any) = {
            let scene = self.server.campaign.scene();
            let mut visible = BTreeSet::new();
            let mut any_vision = false;
            for token in &scene.tokens {
                let Some(vision) = token.vision else {
                    continue;
                };
                if vision.range_ft <= 0.0 {
                    continue;
                }
                any_vision = true;
                let radius = vision_radius_px(&scene.grid, vision.range_ft);
                for cell in visible_cells(scene, token.pos, radius) {
                    visible.insert(cell);
                }
            }
            (visible, any_vision)
        };

        self.fog_active = any;
        self.visible = visible;
        if any {
            let newly: Vec<(i32, i32)> = self.visible.iter().copied().collect();
            self.explored.extend(newly);
        }
    }

    /// Выбрать один токен (сбрасывает остальное выделение).
    pub fn select_only(&mut self, token: EntityId) {
        self.selection.clear();
        self.selection.insert(token);
        self.selected_token = Some(token);
        self.refresh_sheet_from_selection();
    }

    /// Ctrl+клик: добавить/убрать токен из выделения.
    pub fn toggle_selected(&mut self, token: EntityId) {
        if !self.selection.remove(&token) {
            self.selection.insert(token);
        }
        self.selected_token = self.selection.iter().next().copied();
        self.refresh_sheet_from_selection();
    }

    /// Выделить токены, попавшие в рамку.
    pub fn select_in_rect(&mut self, min: Vec2, max: Vec2, add: bool) {
        if !add {
            self.selection.clear();
        }
        let (min_x, max_x) = (min.x.min(max.x), min.x.max(max.x));
        let (min_y, max_y) = (min.y.min(max.y), min.y.max(max.y));
        let inside: Vec<EntityId> = self
            .server
            .campaign
            .scene()
            .tokens
            .iter()
            .filter(|token| {
                token.pos.x >= min_x
                    && token.pos.x <= max_x
                    && token.pos.y >= min_y
                    && token.pos.y <= max_y
            })
            .map(|token| token.id)
            .collect();
        self.selection.extend(inside);
        self.selected_token = self.selection.iter().next().copied();
        self.refresh_sheet_from_selection();
    }

    #[must_use]
    pub fn is_selected(&self, token: EntityId) -> bool {
        self.selection.contains(&token)
    }

    #[must_use]
    pub fn selection_len(&self) -> usize {
        self.selection.len()
    }

    #[must_use]
    pub fn selected_ids(&self) -> Vec<EntityId> {
        self.selection.iter().copied().collect()
    }

    /// Снять выделение.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.selected_token = None;
        self.selected_character = None;
    }

    /// Если выбран ровно один токен с листом — открыть этот лист.
    fn refresh_sheet_from_selection(&mut self) {
        if self.selection.len() == 1 {
            let token = *self.selection.iter().next().expect("ровно один");
            if let Some(character) = self
                .server
                .campaign
                .scene()
                .token(token)
                .and_then(|token| token.character)
            {
                self.selected_character = Some(character);
                self.show_sheet = true;
                return;
            }
        }
        if self.selection.is_empty() {
            self.selected_character = None;
        }
    }

    #[must_use]
    pub fn character(&self, id: EntityId) -> Option<&ScpCharacter> {
        self.server.campaign.character(id)
    }

    /// Копия открытого листа (для отрисовки без заимствований).
    #[must_use]
    pub fn selected_sheet(&self) -> Option<ScpCharacter> {
        self.selected_character
            .and_then(|id| self.server.campaign.character(id).cloned())
    }

    #[must_use]
    pub fn selected_sheet_has_token(&self) -> bool {
        let Some(character) = self.selected_character else {
            return false;
        };
        self.server
            .campaign
            .scene()
            .tokens
            .iter()
            .any(|token| token.character == Some(character))
    }

    /// Применить изменение листа.
    pub fn update_character(&mut self, id: EntityId, patch: CharacterPatch) {
        self.apply(Intent::UpdateCharacter {
            character: id,
            patch,
        });
    }

    pub fn roll_check(&mut self, id: EntityId, skill: ScpSkill, dc: i32) {
        self.apply(Intent::RollCharacterCheck {
            character: id,
            skill: Some(skill),
            attribute: None,
            dc,
        });
    }

    pub fn roll_attr(&mut self, id: EntityId, attr: ScpAttr, dc: i32) {
        self.apply(Intent::RollCharacterCheck {
            character: id,
            skill: None,
            attribute: Some(attr),
            dc,
        });
    }

    pub fn roll_sanity(&mut self, id: EntityId) {
        self.apply(Intent::RollSanity { character: id });
    }

    pub fn grant_xp(&mut self, id: EntityId, amount: i32) {
        self.apply(Intent::GrantXp {
            character: id,
            amount,
        });
    }

    pub fn level_up(&mut self, id: EntityId, attr: ScpAttr) {
        self.apply(Intent::LevelUpAttribute {
            character: id,
            attribute: attr,
        });
    }

    pub fn spawn_character_token(&mut self, id: EntityId, pos: Vec2) {
        self.apply(Intent::SpawnCharacterToken { character: id, pos });
    }

    pub fn create_character(&mut self, name: &str, class: vtt_core::scp::ScpClass) {
        self.apply(Intent::CreateCharacter {
            name: name.to_string(),
            class,
            organization: "Фонд SCP".to_string(),
            access: vtt_core::scp::AccessLevel::First,
        });
    }

    /// Создать персонажа и сразу поставить его токен на сцену.
    pub fn create_and_place_character(
        &mut self,
        name: &str,
        class: vtt_core::scp::ScpClass,
        pos: Vec2,
    ) {
        self.create_character_full(
            name,
            class,
            AccessLevel::First,
            Attributes::default(),
            BTreeMap::new(),
            None,
            TokenFrame::Ring,
            [240, 200, 80],
            pos,
        );
    }

    /// Создать персонажа из данных мастера: лист, затем токен с картинкой и рамкой.
    #[allow(clippy::too_many_arguments)]
    pub fn create_character_full(
        &mut self,
        name: &str,
        class: ScpClass,
        access: AccessLevel,
        attributes: Attributes,
        skills: BTreeMap<ScpSkill, u8>,
        image: Option<String>,
        frame: TokenFrame,
        color: [u8; 3],
        pos: Vec2,
    ) -> Option<EntityId> {
        let events = self.apply(Intent::CreateCharacter {
            name: name.to_string(),
            class,
            organization: "Фонд SCP".to_string(),
            access,
        });
        let character = events.iter().find_map(|event| match event {
            Event::CharacterUpdated { character } => Some(character.id),
            _ => None,
        })?;

        self.apply(Intent::UpdateCharacter {
            character,
            patch: CharacterPatch {
                attributes: Some(attributes),
                skills: Some(skills),
                ..CharacterPatch::default()
            },
        });

        let spawn = self.apply(Intent::SpawnCharacterToken { character, pos });
        let token = spawn.iter().find_map(|event| match event {
            Event::TokenCreated { token } => Some(token.id),
            _ => None,
        });
        if let Some(token) = token {
            if let Some(path) = image {
                self.apply(Intent::SetTokenImage {
                    token,
                    image: Some(path),
                });
            }
            self.apply(Intent::SetTokenFrame {
                token,
                frame,
                color,
            });
            self.selection.clear();
            self.selection.insert(token);
            self.selected_token = Some(token);
        }
        self.selected_character = Some(character);
        self.show_sheet = true;
        Some(character)
    }

    /// Добавить стену.
    pub fn add_wall(&mut self, a: Vec2, b: Vec2, door: bool) {
        self.apply(Intent::AddWall { a, b, door });
    }

    /// Открыть/закрыть дверь рядом с точкой.
    pub fn toggle_door_near(&mut self, at: Vec2) -> bool {
        let index = {
            let scene = self.server.campaign.scene();
            nearest_wall(scene, at, 18.0)
        };
        match index {
            Some(index) => {
                self.apply(Intent::ToggleDoor { index });
                true
            }
            None => false,
        }
    }

    /// Изменить размер выбранных токенов.
    pub fn set_size_for_selection(&mut self, size: vtt_core::entities::TokenSize) {
        let tokens: Vec<EntityId> = self.selection.iter().copied().collect();
        for token in tokens {
            self.apply(Intent::SetTokenSize { token, size });
        }
    }

    /// Задать картинку токену.
    pub fn set_token_image(&mut self, token: EntityId, image: Option<String>) {
        self.apply(Intent::SetTokenImage { token, image });
    }

    /// Рамка и цвет для всех выбранных токенов.
    pub fn set_frame_for_selection(
        &mut self,
        frame: vtt_core::entities::TokenFrame,
        color: [u8; 3],
    ) {
        let tokens = self.selected_ids();
        for token in tokens {
            self.apply(Intent::SetTokenFrame {
                token,
                frame,
                color,
            });
        }
    }

    /// Изменить границы сцены (только ГМ).
    pub fn set_scene_bounds(&mut self, width_px: f32, height_px: f32) {
        self.apply(Intent::SetSceneBounds {
            width_px,
            height_px,
        });
    }

    /// Обработать сообщение сетевого игрока и вернуть исходящие.
    pub fn handle_remote(&mut self, client: ClientId, msg: ClientMsg) -> Vec<Outgoing> {
        let outgoing = self.server.handle(client, msg);
        for item in &outgoing {
            if let ServerMsg::Events(list) = &item.msg {
                for event in list {
                    self.record_event(event);
                }
            }
        }
        outgoing
    }

    /// Записать событие в журнал и ленту карточек.
    fn record_event(&mut self, event: &Event) {
        self.push_log(describe_event(event));
        let item = match event {
            Event::ChatPosted {
                from_name,
                text,
                to_gm_only,
                ..
            } => {
                self.push_chat(from_name.clone(), text.clone(), *to_gm_only);
                FeedItem::Chat {
                    from: from_name.clone(),
                    text: text.clone(),
                    to_gm: *to_gm_only,
                }
            }
            Event::Rolled {
                from_name,
                result,
                to_gm_only,
                ..
            } => FeedItem::Roll(RollCard {
                actor: from_name.clone(),
                title: "Кубы".to_string(),
                formula: result.notation.clone(),
                total: result.total,
                gm_only: *to_gm_only,
                success: None,
                note: result
                    .natural_d20()
                    .map(|natural| format!("d20: {natural}")),
            }),
            Event::CharacterCheck {
                name,
                skill,
                result,
                ..
            } => FeedItem::Roll(RollCard {
                actor: name.clone(),
                title: skill.map_or_else(
                    || "Проверка".to_string(),
                    |skill| skill.name_ru().to_string(),
                ),
                formula: format!("{} {:+}", result.roll.notation, result.bonus),
                total: result.total,
                gm_only: false,
                success: Some(result.success),
                note: Some(format!("СЛ {}", result.dc)),
            }),
            other => FeedItem::Text(describe_event(other)),
        };
        if self.feed.len() >= 200 {
            self.feed.remove(0);
        }
        self.feed.push(item);
    }

    /// Добавить строку чата.
    pub fn push_chat(&mut self, from: String, text: String, to_gm: bool) {
        if self.chat.len() >= 300 {
            self.chat.remove(0);
        }
        self.chat.push(ChatLine { from, text, to_gm });
    }

    /// Прозрачность выбранных токенов.
    pub fn set_opacity_for_selection(&mut self, opacity: f32) {
        let tokens = self.selected_ids();
        for token in tokens {
            self.apply(Intent::SetTokenOpacity { token, opacity });
        }
    }

    /// Скрыть/показать выбранные токены.
    pub fn set_hidden_for_selection(&mut self, hidden: bool) {
        let tokens = self.selected_ids();
        for token in tokens {
            self.apply(Intent::SetTokenHidden { token, hidden });
        }
    }

    /// Настройки сетки (ГМ).
    pub fn set_grid(&mut self, kind: vtt_core::grid::GridKind, cell_px: f32, feet: f32) {
        self.apply(Intent::SetGrid {
            kind,
            cell_px,
            feet_per_cell: feet,
        });
    }

    /// Фон сцены (ГМ).
    pub fn set_background(&mut self, path: Option<String>) {
        self.apply(Intent::SetSceneBackground { path });
    }

    /// Выдать роль участнику (ГМ).
    pub fn set_player_role(&mut self, player: PlayerId, role: vtt_core::permissions::Role) {
        self.apply(Intent::SetPlayerRole { player, role });
    }

    /// Найти соединение участника (для кика).
    #[must_use]
    pub fn client_of(&self, player: PlayerId) -> Option<ClientId> {
        self.server.client_of(player)
    }

    /// Сохранить кампанию и журнал сессии.
    pub fn save(&mut self) -> Result<PathBuf> {
        let path = save_campaign(&self.server.campaign, &self.dir)?;
        let log_path = self.dir.join("session.log");
        let text = self.log.join("\n");
        // Журнал не критичен: не сохранился — не повод падать.
        let _ = std::fs::write(log_path, text);
        self.dirty = false;
        Ok(path)
    }

    fn push_log(&mut self, line: String) {
        if self.log.len() >= LOG_CAPACITY {
            self.log.remove(0);
        }
        self.log.push(line);
    }
}

/// Человекочитаемое описание события для журнала окна.
#[must_use]
pub fn describe_event(event: &Event) -> String {
    match event {
        Event::TokenMoved { token, to } => format!("{token} → ({:.0}, {:.0})", to.x, to.y),
        Event::TokenCreated { token } => format!("создан {} «{}»", token.id, token.name),
        Event::TokenRemoved { token } => format!("удалён {token}"),
        Event::TokenChanged { token } => {
            let hp = token
                .hp
                .map(|bar| format!(" HP {}/{}", bar.value, bar.max))
                .unwrap_or_default();
            format!("{} «{}»{hp}", token.id, token.name)
        }
        Event::ChatPosted {
            from_name,
            text,
            to_gm_only,
            ..
        } => format!(
            "💬 {from_name}: {text}{}",
            if *to_gm_only { " (шёпот)" } else { "" }
        ),
        Event::Rolled {
            from_name, result, ..
        } => format!("🎲 {from_name}: {}", result.describe()),
        Event::InitiativeSet { name, value, .. } => format!("инициатива «{name}»: {value}"),
        Event::TurnChanged {
            round, active_name, ..
        } => match active_name {
            Some(name) => format!("раунд {round}, ход: {name}"),
            None => format!("раунд {round}"),
        },
        Event::SceneChanged { name, .. } => format!("сцена «{name}»"),
        Event::ConcentrationCheck { dc, effect, .. } => {
            format!("концентрация «{effect}»: спасбросок СЛ {dc}")
        }
        Event::DeathSaveResult {
            successes,
            failures,
            outcome,
            ..
        } => format!(
            "спасбросок от смерти: успехи {successes}, провалы {failures}, итог {outcome:?}"
        ),
        Event::HandoutPosted { title, .. } => format!("хендаут «{title}»"),
        Event::FogChanged { shapes } => format!("туман: областей {}", shapes.len()),
        Event::CharacterUpdated { character } => format!("лист: {}", character.summary()),
        Event::CharacterCheck {
            name,
            skill,
            result,
            ..
        } => {
            let what = skill.map_or("проверка", ScpSkill::name_ru);
            format!("{name}: {}", result.describe(what))
        }
        Event::SanityRolled { name, outcome, .. } => {
            format!("{name}: {}", outcome.describe())
        }
        Event::XpGranted {
            name,
            gained,
            total,
            ..
        } => format!("{name}: +{gained} опыта (всего {total})"),
        Event::WallsChanged { walls } => format!("стены: {}", walls.len()),
        Event::SceneBoundsChanged {
            width_px,
            height_px,
        } => format!("границы карты: {width_px:.0}×{height_px:.0}"),
        Event::GridChanged {
            kind,
            cell_px,
            feet_per_cell,
        } => format!("сетка: {kind:?}, {cell_px:.0} px, {feet_per_cell:.0} фт"),
        Event::SceneBackgroundChanged { path } => {
            format!("фон: {}", path.clone().unwrap_or_else(|| "нет".to_string()))
        }
    }
}
