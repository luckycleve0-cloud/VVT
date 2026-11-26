//! Авторитарный сервер: единственное место, где меняется состояние игры.
//!
//! Схема работы:
//!
//! 1. Клиент присылает [`Intent`] — «хочу подвинуть токен».
//! 2. Сервер проверяет права ([`vtt_core::permissions`]) и лимиты.
//! 3. Применяет изменение к [`Campaign`].
//! 4. Возвращает список [`Outgoing`] — кому и что отправить.
//!
//! Транспорт (`vtt-net`) только доставляет эти сообщения; он ничего не решает.

use std::collections::BTreeMap;

use vtt_core::dice::{self, DiceExpr, RollMode};
use vtt_core::entities::{Bar, Token, Vision};
use vtt_core::ids::{EntityId, PlayerId};
use vtt_core::permissions::{can, Access, Capability, Role, Target};
use vtt_core::rng::Rng;
use vtt_core::scene::{DoorState, FogMode, FogShape, Wall};
use vtt_core::scp::{roll_sanity, ScpAttr, ScpCharacter};
use vtt_proto::{
    check_protocol_version, CharacterPatch, ClientId, ClientMsg, DeathOutcome, Event, Intent,
    PlayerInfo, ServerMsg, Snapshot, Welcome, PROTOCOL_VERSION,
};

use crate::campaign::Campaign;

/// Идентификатор локального клиента ГМа.
pub const HOST_CLIENT: ClientId = 0;

/// Максимальная длина сообщения чата (символов).
pub const MAX_CHAT_LEN: usize = 2_000;
/// Максимум токенов на сцену — защита от случайного «размножения».
pub const MAX_TOKENS_PER_SCENE: usize = 500;

/// Алфавит кода комнаты без похожих символов (0/O, 1/I/L).
const ROOM_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Сгенерировать код комнаты из 6 символов.
#[must_use]
pub fn room_code(rng: &mut Rng) -> String {
    let len = ROOM_CODE_ALPHABET.len() as u32;
    (0..6)
        .map(|_| char::from(ROOM_CODE_ALPHABET[(rng.roll(len) - 1) as usize]))
        .collect()
}

/// Подключённый участник (может быть временно отключён и вернуться).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientSession {
    pub client: ClientId,
    pub player: PlayerId,
    pub name: String,
    pub role: Role,
    /// Сейчас на связи.
    pub connected: bool,
}

/// Кому и что отправить. `to: None` — всем.
#[derive(Debug, Clone)]
pub struct Outgoing {
    pub to: Option<ClientId>,
    pub msg: ServerMsg,
}

impl Outgoing {
    #[must_use]
    pub fn broadcast(msg: ServerMsg) -> Self {
        Self { to: None, msg }
    }

    #[must_use]
    pub fn to(client: ClientId, msg: ServerMsg) -> Self {
        Self {
            to: Some(client),
            msg,
        }
    }
}

/// Сервер одной комнаты.
#[derive(Debug)]
pub struct GameServer {
    pub campaign: Campaign,
    pub room_code: String,
    clients: BTreeMap<ClientId, ClientSession>,
    next_player: u64,
    /// Генератор для бросков. Хранится на сервере — значит результат нельзя подделать.
    pub rng: Rng,
}

impl GameServer {
    /// Новый сервер со случайным кодом комнаты.
    #[must_use]
    pub fn new(campaign: Campaign) -> Self {
        let mut rng = Rng::from_entropy();
        let code = room_code(&mut rng);
        Self::with_room_code_and_rng(campaign, code, rng)
    }

    /// Новый сервер с заданным кодом (нужно для тестов и повторного запуска).
    #[must_use]
    pub fn with_room_code(campaign: Campaign, code: impl Into<String>) -> Self {
        Self::with_room_code_and_rng(campaign, code, Rng::from_entropy())
    }

    fn with_room_code_and_rng(campaign: Campaign, code: impl Into<String>, rng: Rng) -> Self {
        Self {
            campaign,
            room_code: code.into(),
            clients: BTreeMap::new(),
            next_player: 1,
            rng,
        }
    }

    #[must_use]
    pub fn room_code(&self) -> &str {
        &self.room_code
    }

    #[must_use]
    pub fn client_count(&self) -> usize {
        self.clients.values().filter(|s| s.connected).count()
    }

    #[must_use]
    pub fn session(&self, client: ClientId) -> Option<&ClientSession> {
        self.clients.get(&client)
    }

    /// Соединение участника по `PlayerId` (для кика/уведомлений).
    #[must_use]
    pub fn client_of(&self, player: PlayerId) -> Option<ClientId> {
        self.clients
            .values()
            .find(|session| session.player == player)
            .map(|session| session.client)
    }

    /// Подключить локального ГМа (сервер и клиент в одном процессе).
    pub fn add_host(&mut self, name: &str) -> Welcome {
        self.join(HOST_CLIENT, name, Role::Gm)
    }

    /// Подключить участника. Повторный вызов обновляет имя, но не роль.
    pub fn join(&mut self, client: ClientId, name: &str, role: Role) -> Welcome {
        if let Some(existing) = self.clients.get_mut(&client) {
            existing.name = name.to_string();
            return Welcome {
                client,
                player: existing.player,
                role: existing.role,
                campaign: self.campaign.name.clone(),
                protocol: PROTOCOL_VERSION,
            };
        }

        let player = PlayerId(self.next_player);
        self.next_player += 1;
        self.clients.insert(
            client,
            ClientSession {
                client,
                player,
                name: name.to_string(),
                role,
                connected: true,
            },
        );

        Welcome {
            client,
            player,
            role,
            campaign: self.campaign.name.clone(),
            protocol: PROTOCOL_VERSION,
        }
    }

    /// Участник отключился: сессия остаётся, чтобы можно было вернуться.
    pub fn leave(&mut self, client: ClientId) -> Option<PlayerId> {
        match self.clients.get_mut(&client) {
            Some(session) => {
                session.connected = false;
                Some(session.player)
            }
            None => None,
        }
    }

    /// Вернуть участника по его `PlayerId` (переподключение).
    ///
    /// Возвращает `None`, если такой игрок не найден.
    pub fn resume_session(&mut self, client: ClientId, player: PlayerId) -> Option<Welcome> {
        let old_client = self
            .clients
            .values()
            .find(|s| s.player == player)
            .map(|s| s.client)?;
        let mut session = self.clients.remove(&old_client)?;
        session.client = client;
        session.connected = true;
        let welcome = Welcome {
            client,
            player: session.player,
            role: session.role,
            campaign: self.campaign.name.clone(),
            protocol: PROTOCOL_VERSION,
        };
        self.clients.insert(client, session);
        Some(welcome)
    }

    #[must_use]
    pub fn players(&self) -> Vec<PlayerInfo> {
        self.clients
            .values()
            .map(|s| PlayerInfo {
                player: s.player,
                name: s.name.clone(),
                role: s.role,
                connected: s.connected,
            })
            .collect()
    }

    /// Срез состояния для конкретного клиента: скрытые токены не попадают игрокам.
    #[must_use]
    pub fn snapshot(&self, client: ClientId) -> Option<Snapshot> {
        let session = self.clients.get(&client)?;
        let is_gm = session.role.is_gm_like();

        let mut scene = self.campaign.scene().clone();
        if !is_gm {
            scene.tokens.retain(|token| !token.hidden);
        }

        let player = session.player;
        let characters = self
            .campaign
            .characters
            .iter()
            .filter(|character| is_gm || character.owner == Some(player))
            .cloned()
            .collect();

        Some(Snapshot {
            campaign: self.campaign.name.clone(),
            scene,
            combat: self.campaign.combat.clone(),
            players: self.players(),
            characters,
            you: PlayerInfo {
                player: session.player,
                name: session.name.clone(),
                role: session.role,
                connected: true,
            },
        })
    }

    // -----------------------------------------------------------------------
    // Обработка входящих сообщений
    // -----------------------------------------------------------------------

    /// Обработать сообщение клиента. Возвращает то, что нужно разослать.
    pub fn handle(&mut self, client: ClientId, msg: ClientMsg) -> Vec<Outgoing> {
        match msg {
            ClientMsg::Hello {
                name,
                code,
                protocol,
                resume,
            } => {
                if let Err(reason) = check_protocol_version(protocol) {
                    return vec![Outgoing::to(client, ServerMsg::Rejected { reason })];
                }
                if code != self.room_code {
                    return vec![Outgoing::to(
                        client,
                        ServerMsg::Rejected {
                            reason: "неверный код комнаты".to_string(),
                        },
                    )];
                }
                let welcome = match resume {
                    Some(player) if self.clients.values().any(|s| s.player == player) => self
                        .resume_session(client, player)
                        .unwrap_or_else(|| self.join(client, &name, Role::Player)),
                    _ => self.join(client, &name, Role::Player),
                };
                let returning = resume.is_some();
                let mut out = vec![Outgoing::to(client, ServerMsg::Welcome(welcome))];
                if let Some(snapshot) = self.snapshot(client) {
                    out.push(Outgoing::to(
                        client,
                        ServerMsg::Snapshot(Box::new(snapshot)),
                    ));
                }
                if returning {
                    out.push(Outgoing::broadcast(ServerMsg::Notice {
                        text: format!("{name} вернулся в игру"),
                    }));
                }
                out
            }
            ClientMsg::Intent(intent) => self.apply_intent(client, intent),
            ClientMsg::Ping { nonce } => {
                vec![Outgoing::to(client, ServerMsg::Pong { nonce })]
            }
            ClientMsg::Bye => {
                self.leave(client);
                Vec::new()
            }
        }
    }

    fn apply_intent(&mut self, client: ClientId, intent: Intent) -> Vec<Outgoing> {
        let Some(access) = self.access_of(client) else {
            return deny(client, "сначала подключитесь к комнате");
        };

        match intent {
            Intent::Chat { text, to_gm_only } => {
                let text = text.trim().to_string();
                if text.is_empty() {
                    return deny(client, "пустое сообщение");
                }
                if text.chars().count() > MAX_CHAT_LEN {
                    return deny(client, "слишком длинное сообщение");
                }
                let event = Event::ChatPosted {
                    from: access.player,
                    from_name: self.name_of(client),
                    text,
                    to_gm_only,
                };
                if to_gm_only {
                    self.whisper(client, event)
                } else {
                    vec![Outgoing::broadcast(ServerMsg::Events(vec![event]))]
                }
            }

            Intent::Roll {
                expr,
                mode,
                to_gm_only,
            } => {
                let parsed = match DiceExpr::parse(&expr) {
                    Ok(parsed) => parsed,
                    Err(err) => return deny(client, format!("ошибка в формуле броска: {err}")),
                };
                let result = dice::roll(&parsed, mode, &mut self.rng);
                let event = Event::Rolled {
                    from: access.player,
                    from_name: self.name_of(client),
                    result,
                    to_gm_only,
                };
                if to_gm_only {
                    self.whisper(client, event)
                } else {
                    vec![Outgoing::broadcast(ServerMsg::Events(vec![event]))]
                }
            }

            Intent::MoveToken { token, to } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Move, Target::Token(token), owner) {
                    return deny(client, "нет прав на перемещение этого токена");
                }

                let Some(current) = self.campaign.scene().token(token) else {
                    return deny(client, "токен не найден");
                };
                if current.locked && !access.role.is_gm_like() {
                    return deny(client, "токен закреплён ГМом");
                }

                let grid = self.campaign.scene().grid;
                let snapped = grid.snap(to);
                if !self.campaign.scene_mut().move_token(token, snapped) {
                    return deny(client, "токен не найден");
                }
                let hidden = self.campaign.scene().token(token).is_some_and(|t| t.hidden);

                self.broadcast_event(Event::TokenMoved { token, to: snapped }, !hidden)
            }

            Intent::CreateToken { name, pos, size } => {
                if !access.role.is_gm_like() {
                    return deny(client, "только ГМ может создавать токены");
                }
                if self.campaign.scene().tokens.len() >= MAX_TOKENS_PER_SCENE {
                    return deny(client, "на сцене слишком много токенов");
                }
                let name = name.trim();
                if name.is_empty() {
                    return deny(client, "у токена должно быть имя");
                }

                let id = self.campaign.next_id();
                let grid = self.campaign.scene().grid;
                let mut token = Token::new(id, name, grid.snap(pos));
                token.size = size;
                self.campaign.scene_mut().add_token(token.clone());
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::TokenCreated { token },
                ]))]
            }

            Intent::DeleteToken { token } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Delete, Target::Token(token), owner) {
                    return deny(client, "нет прав на удаление этого токена");
                }
                if self.campaign.scene_mut().remove_token(token).is_none() {
                    return deny(client, "токен не найден");
                }
                self.campaign.combat.remove(token);
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::TokenRemoved { token },
                ]))]
            }

            Intent::SetTokenHp { token, value } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять этот токен");
                }
                let Some(current) = self.campaign.scene().token(token) else {
                    return deny(client, "токен не найден");
                };
                let Some(mut bar) = current.hp else {
                    return deny(client, "у токена нет полоски хитов");
                };
                bar.value = value.clamp(0, bar.max.max(0));
                if let Some(t) = self.campaign.scene_mut().token_mut(token) {
                    t.hp = Some(bar);
                }
                self.broadcast_token_change(token)
            }

            Intent::DamageToken { token, amount } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять этот токен");
                }
                if amount <= 0 {
                    return deny(client, "урон должен быть положительным");
                }
                let Some(token_before) = self.campaign.scene().token(token).cloned() else {
                    return deny(client, "токен не найден");
                };
                let Some(mut bar) = token_before.hp else {
                    return deny(client, "у токена нет полоски хитов");
                };
                bar.value = (bar.value - amount).max(0);
                if let Some(t) = self.campaign.scene_mut().token_mut(token) {
                    t.hp = Some(bar);
                }

                let mut events = Vec::new();
                if let Some(token_after) = self.campaign.scene().token(token) {
                    events.push(Event::TokenChanged {
                        token: token_after.clone(),
                    });
                }
                // Концентрация: СЛ = max(10, половина урона).
                if let Some(effect) = token_before.concentration {
                    events.push(Event::ConcentrationCheck {
                        token,
                        dc: (amount / 2).max(10),
                        effect,
                    });
                }

                let hidden = self.campaign.scene().token(token).is_some_and(|t| t.hidden);
                self.broadcast_events(events, !hidden)
            }

            Intent::SetTokenHidden { token, hidden } => {
                if !access.role.is_gm_like() {
                    return deny(client, "скрывать токены может только ГМ");
                }
                let Some(t) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                t.hidden = hidden;
                self.broadcast_token_change_visible(token, !hidden)
            }

            Intent::SetInitiative { token, value } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять инициативу");
                }
                let Some(token_data) = self.campaign.scene().token(token) else {
                    return deny(client, "токен не найден");
                };
                let name = token_data.name.clone();
                if !self.campaign.combat.set_initiative(token, value) {
                    self.campaign.combat.add(vtt_core::combat::Combatant {
                        token,
                        name: name.clone(),
                        initiative: value,
                    });
                }
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::InitiativeSet { token, value, name },
                ]))]
            }

            Intent::NextTurn => {
                if !access.role.is_gm_like() {
                    return deny(client, "ход передаёт ГМ");
                }
                self.campaign.combat.start();
                let current = self
                    .campaign
                    .combat
                    .next_turn()
                    .map(|c| (c.token, c.name.clone()));
                let (active, active_name) = match current {
                    Some((token, name)) => (Some(token), Some(name)),
                    None => (None, None),
                };
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::TurnChanged {
                        round: self.campaign.combat.round,
                        active,
                        active_name,
                    },
                ]))]
            }

            Intent::ChangeScene { scene } => {
                if !can(&access, Capability::ManageRoom, Target::Settings, None) {
                    return deny(client, "менять сцену может только ГМ");
                }
                let Some(target) = self.campaign.scene_by_id(scene) else {
                    return deny(client, "сцена не найдена");
                };
                let name = target.name.clone();
                self.campaign.set_active_scene(scene);

                // После смены сцены каждому нужен свой снапшот (скрытые токены!).
                let mut out: Vec<Outgoing> = self
                    .clients
                    .keys()
                    .filter_map(|&id| {
                        self.snapshot(id)
                            .map(|snap| Outgoing::to(id, ServerMsg::Snapshot(Box::new(snap))))
                    })
                    .collect();
                out.push(Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::SceneChanged { scene, name },
                ])));
                out
            }

            Intent::CreateMonsterToken { monster, pos } => {
                if !access.role.is_gm_like() {
                    return deny(client, "только ГМ может создавать токены");
                }
                let Some(monster_data) = self.campaign.content.monster(monster).cloned() else {
                    return deny(client, "монстр не найден в компендиуме");
                };
                if self.campaign.scene().tokens.len() >= MAX_TOKENS_PER_SCENE {
                    return deny(client, "на сцене слишком много токенов");
                }

                let id = self.campaign.next_id();
                let grid = self.campaign.scene().grid;
                let mut token = Token::new(id, monster_data.name.clone(), grid.snap(pos));
                token.hp = Some(Bar::new(monster_data.hit_points, monster_data.hit_points));
                token.ac = Some(Bar::new(monster_data.armor_class, monster_data.armor_class));
                self.campaign.scene_mut().add_token(token.clone());

                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::TokenCreated { token },
                ]))]
            }

            Intent::SpawnMonster { name, pos } => {
                let Some(monster) = self.campaign.content.monster_by_name(&name) else {
                    return deny(client, format!("в компендиуме нет монстра «{name}»"));
                };
                let monster_id = monster.id;
                self.apply_intent(
                    client,
                    Intent::CreateMonsterToken {
                        monster: monster_id,
                        pos,
                    },
                )
            }

            Intent::Rest { token, long } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав на отдых этого токена");
                }
                let Some(existing) = self.campaign.scene().token(token).cloned() else {
                    return deny(client, "токен не найден");
                };
                if existing.dead {
                    return deny(client, "мёртвому отдых не поможет");
                }

                if let Some(tok) = self.campaign.scene_mut().token_mut(token) {
                    if long {
                        if let Some(hp) = tok.hp.as_mut() {
                            hp.value = hp.max;
                        }
                        tok.death_saves.reset();
                        tok.dead = false;
                    }
                }

                let mut events = Vec::new();
                if let Some(tok) = self.campaign.scene().token(token) {
                    events.push(Event::TokenChanged { token: tok.clone() });
                }
                let mut out = self.broadcast_events(events, true);
                out.push(Outgoing::broadcast(ServerMsg::Notice {
                    text: format!(
                        "{}: {} отдых",
                        existing.name,
                        if long {
                            "длинный"
                        } else {
                            "короткий"
                        }
                    ),
                }));
                out
            }

            Intent::DeathSave { token } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав на этот бросок");
                }
                let Some(existing) = self.campaign.scene().token(token).cloned() else {
                    return deny(client, "токен не найден");
                };
                if existing.dead {
                    return deny(client, "персонаж уже мёртв");
                }
                let Some(hp) = existing.hp else {
                    return deny(client, "у токена нет полоски хитов");
                };
                if hp.value > 0 {
                    return deny(client, "спасбросок от смерти нужен только при 0 хитов");
                }

                let expr = DiceExpr::parse("1d20").expect("1d20 разбирается всегда");
                let roll = dice::roll(&expr, RollMode::Normal, &mut self.rng);
                let natural = roll.natural_d20().unwrap_or(0);
                let mut outcome = DeathOutcome::Continue;

                if let Some(tok) = self.campaign.scene_mut().token_mut(token) {
                    match natural {
                        20 => {
                            if let Some(hp) = tok.hp.as_mut() {
                                hp.value = 1;
                            }
                            tok.death_saves.reset();
                            tok.dead = false;
                            outcome = DeathOutcome::Revived;
                        }
                        1 => {
                            tok.death_saves.failures = (tok.death_saves.failures + 2).min(3);
                        }
                        10..=19 => {
                            tok.death_saves.successes = (tok.death_saves.successes + 1).min(3);
                        }
                        _ => {
                            tok.death_saves.failures = (tok.death_saves.failures + 1).min(3);
                        }
                    }
                    if tok.death_saves.failures >= 3 {
                        tok.dead = true;
                        outcome = DeathOutcome::Dead;
                    } else if tok.death_saves.successes >= 3 {
                        outcome = DeathOutcome::Stabilized;
                    }
                }

                let (successes, failures) =
                    self.campaign.scene().token(token).map_or((0, 0), |t| {
                        (t.death_saves.successes, t.death_saves.failures)
                    });
                let hidden = self.campaign.scene().token(token).is_some_and(|t| t.hidden);

                self.broadcast_events(
                    vec![Event::DeathSaveResult {
                        token,
                        roll,
                        successes,
                        failures,
                        outcome,
                    }],
                    !hidden,
                )
            }

            Intent::SetConcentration { token, effect } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять концентрацию");
                }
                let effect = effect
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty());
                let Some(tok) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                tok.concentration = effect;
                self.broadcast_token_change(token)
            }

            Intent::PostHandout { title, text } => {
                if !access.role.is_gm_like() {
                    return deny(client, "хендауты показывает только ГМ");
                }
                let title = title.trim().to_string();
                let text = text.trim().to_string();
                if title.is_empty() || text.is_empty() {
                    return deny(client, "нужны и заголовок, и текст");
                }
                self.campaign.add_handout(title.clone(), text.clone());
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::HandoutPosted { title, text },
                ]))]
            }

            Intent::AddFog { center, radius } => {
                if !access.role.is_gm_like() {
                    return deny(client, "туман рисует только ГМ");
                }
                let radius = radius.clamp(5.0, 4000.0);
                let scene = self.campaign.scene_mut();
                scene.fog.mode = FogMode::Manual;
                scene.fog.add_shape(FogShape::Circle { center, radius });
                let shapes = scene.fog.shapes.clone();
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::FogChanged { shapes },
                ]))]
            }

            Intent::ClearFog => {
                if !access.role.is_gm_like() {
                    return deny(client, "туман очищает только ГМ");
                }
                self.campaign.scene_mut().fog.clear();
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::FogChanged { shapes: Vec::new() },
                ]))]
            }

            // --- SCP-лист персонажа ---
            Intent::CreateCharacter {
                name,
                class,
                organization,
                access: requested_access,
            } => {
                let name = name.trim().to_string();
                if name.is_empty() {
                    return deny(client, "у персонажа должно быть имя");
                }
                let id = self.campaign.next_id();
                let mut character = ScpCharacter::new(id, name, class);
                character.organization = organization.trim().to_string();
                character.access = requested_access;
                character.owner = Some(access.player);
                character.physical.current = character.physical.max;
                character.mental.current = character.mental.max;
                self.campaign.characters.push(character.clone());
                self.notify_character(client, Event::CharacterUpdated { character })
            }

            Intent::UpdateCharacter { character, patch } => {
                if !self.can_edit_character(&access, character) {
                    return deny(client, "нет прав менять этого персонажа");
                }
                let Some(target) = self.campaign.character_mut(character) else {
                    return deny(client, "персонаж не найден");
                };
                apply_character_patch(target, &patch);
                let updated = target.clone();
                self.notify_character(client, Event::CharacterUpdated { character: updated })
            }

            Intent::RollCharacterCheck {
                character,
                skill,
                attribute,
                dc,
            } => {
                let Some(target) = self.campaign.character(character).cloned() else {
                    return deny(client, "персонаж не найден");
                };
                if !self.can_edit_character(&access, character) {
                    return deny(client, "нельзя бросать за чужого персонажа");
                }
                let dc = dc.clamp(1, 40);
                let result = match (skill, attribute) {
                    (Some(skill), _) => target.roll_check(skill, dc, &mut self.rng),
                    (None, Some(attr)) => target.roll_attr_check(attr, dc, &mut self.rng),
                    (None, None) => {
                        return deny(client, "нужен навык или характеристика");
                    }
                };
                self.notify_character(
                    client,
                    Event::CharacterCheck {
                        character,
                        name: target.name,
                        skill,
                        result,
                    },
                )
            }

            Intent::RollSanity { character } => {
                let Some(target) = self.campaign.character(character).cloned() else {
                    return deny(client, "персонаж не найден");
                };
                if !self.can_edit_character(&access, character) {
                    return deny(client, "нельзя бросать за чужого персонажа");
                }
                let outcome = roll_sanity(&mut self.rng);
                self.notify_character(
                    client,
                    Event::SanityRolled {
                        character,
                        name: target.name,
                        outcome,
                    },
                )
            }

            Intent::GrantXp { character, amount } => {
                if !access.role.is_gm_like() {
                    return deny(client, "опыт выдаёт ГМ");
                }
                if amount <= 0 {
                    return deny(client, "опыт должен быть положительным");
                }
                let Some(target) = self.campaign.character_mut(character) else {
                    return deny(client, "персонаж не найден");
                };
                let gained = target.grant_xp(amount);
                let event = Event::XpGranted {
                    character,
                    name: target.name.clone(),
                    gained,
                    total: target.xp,
                };
                self.notify_character(client, event)
            }

            Intent::LevelUpAttribute {
                character,
                attribute,
            } => {
                if !self.can_edit_character(&access, character) {
                    return deny(client, "нет прав менять этого персонажа");
                }
                let Some(target) = self.campaign.character_mut(character) else {
                    return deny(client, "персонаж не найден");
                };
                match target.level_up(attribute) {
                    Ok(_) => {
                        let updated = target.clone();
                        self.notify_character(
                            client,
                            Event::CharacterUpdated { character: updated },
                        )
                    }
                    Err(reason) => deny(client, reason),
                }
            }

            Intent::SpawnCharacterToken { character, pos } => {
                if !access.role.is_gm_like() {
                    return deny(client, "токен персонажа ставит ГМ");
                }
                let Some(target) = self.campaign.character(character).cloned() else {
                    return deny(client, "персонаж не найден");
                };
                if self.campaign.scene().tokens.len() >= MAX_TOKENS_PER_SCENE {
                    return deny(client, "на сцене слишком много токенов");
                }
                let id = self.campaign.next_id();
                let grid = self.campaign.scene().grid;
                let mut token = Token::new(id, target.name.clone(), grid.snap(pos));
                token.character = Some(character);
                token.hp = Some(Bar::new(target.physical.current, target.physical.max));
                token.vision = Some(Vision {
                    range_ft: 30.0,
                    darkvision_ft: 0.0,
                });
                self.campaign.scene_mut().add_token(token.clone());
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::TokenCreated { token },
                ]))]
            }

            // --- M4: стены и свет ---
            Intent::AddWall { a, b, door } => {
                if !access.role.is_gm_like() {
                    return deny(client, "стены рисует только ГМ");
                }
                if a.distance(b) < 5.0 {
                    return deny(client, "стена слишком короткая");
                }
                let wall = if door {
                    Wall::door(a, b, DoorState::Closed)
                } else {
                    Wall::solid(a, b)
                };
                self.campaign.scene_mut().walls.push(wall);
                let walls = self.campaign.scene().walls.clone();
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::WallsChanged { walls },
                ]))]
            }

            Intent::ToggleDoor { index } => {
                if !access.role.is_gm_like() {
                    return deny(client, "двери открывает только ГМ");
                }
                if !self.campaign.scene_mut().toggle_door(index) {
                    return deny(client, "стена не найдена");
                }
                let walls = self.campaign.scene().walls.clone();
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::WallsChanged { walls },
                ]))]
            }

            Intent::RemoveWall { index } => {
                if !access.role.is_gm_like() {
                    return deny(client, "стены удаляет только ГМ");
                }
                let scene = self.campaign.scene_mut();
                if index >= scene.walls.len() {
                    return deny(client, "стена не найдена");
                }
                scene.walls.remove(index);
                let walls = scene.walls.clone();
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::WallsChanged { walls },
                ]))]
            }

            Intent::SetTokenVision { token, range_ft } => {
                if !access.role.is_gm_like() {
                    return deny(client, "обзор задаёт ГМ");
                }
                let range = range_ft.clamp(0.0, 240.0);
                let Some(target) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                target.vision = Some(Vision {
                    range_ft: range,
                    darkvision_ft: 0.0,
                });
                self.broadcast_token_change(token)
            }

            Intent::SetTokenSize { token, size } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять этот токен");
                }
                let Some(target) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                target.size = size;
                self.broadcast_token_change(token)
            }

            Intent::SetTokenImage { token, image } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять этот токен");
                }
                let image = image.filter(|path| !path.trim().is_empty());
                if let Some(path) = &image {
                    if path.chars().count() > 512 {
                        return deny(client, "слишком длинный путь к картинке");
                    }
                }
                let Some(target) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                target.image = image;
                self.broadcast_token_change(token)
            }

            Intent::SetSceneBounds {
                width_px,
                height_px,
            } => {
                if !access.role.is_gm_like() {
                    return deny(client, "границы карты задаёт ГМ");
                }
                let width = width_px.clamp(200.0, 20_000.0);
                let height = height_px.clamp(200.0, 20_000.0);
                let scene = self.campaign.scene_mut();
                scene.width_px = width;
                scene.height_px = height;
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::SceneBoundsChanged {
                        width_px: width,
                        height_px: height,
                    },
                ]))]
            }

            Intent::SetTokenFrame {
                token,
                frame,
                color,
            } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять этот токен");
                }
                let Some(target) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                target.frame = frame;
                target.frame_color = color;
                self.broadcast_token_change(token)
            }

            Intent::SetTokenOpacity { token, opacity } => {
                let owner = self.owner_of(token);
                if !can(&access, Capability::Edit, Target::Token(token), owner) {
                    return deny(client, "нет прав менять этот токен");
                }
                let value = opacity.clamp(0.05, 1.0);
                let Some(target) = self.campaign.scene_mut().token_mut(token) else {
                    return deny(client, "токен не найден");
                };
                target.opacity = value;
                self.broadcast_token_change(token)
            }

            Intent::SetPlayerRole { player, role } => {
                if !access.role.is_gm_like() {
                    return deny(client, "роли выдаёт только ГМ");
                }
                let Some(session) = self.clients.values_mut().find(|s| s.player == player) else {
                    return deny(client, "участник не найден");
                };
                if session.client == HOST_CLIENT {
                    return deny(client, "нельзя менять роль самого ГМа");
                }
                session.role = role;
                let name = session.name.clone();
                vec![Outgoing::broadcast(ServerMsg::Notice {
                    text: format!("{name}: роль {}", role.name_ru()),
                })]
            }

            Intent::SetGrid {
                kind,
                cell_px,
                feet_per_cell,
            } => {
                if !access.role.is_gm_like() {
                    return deny(client, "сетку настраивает ГМ");
                }
                let cell = cell_px.clamp(20.0, 200.0);
                let feet = feet_per_cell.clamp(0.5, 50.0);
                let scene = self.campaign.scene_mut();
                scene.grid.kind = kind;
                scene.grid.cell_px = cell;
                scene.grid.feet_per_cell = feet;
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::GridChanged {
                        kind,
                        cell_px: cell,
                        feet_per_cell: feet,
                    },
                ]))]
            }

            Intent::SetSceneBackground { path } => {
                if !access.role.is_gm_like() {
                    return deny(client, "фон меняет ГМ");
                }
                let path = path.filter(|value| !value.trim().is_empty());
                if let Some(value) = &path {
                    if value.chars().count() > 512 {
                        return deny(client, "слишком длинный путь");
                    }
                }
                self.campaign.scene_mut().background = path.clone();
                vec![Outgoing::broadcast(ServerMsg::Events(vec![
                    Event::SceneBackgroundChanged { path },
                ]))]
            }
        }
    }

    // -----------------------------------------------------------------------
    // Вспомогательное
    // -----------------------------------------------------------------------

    fn access_of(&self, client: ClientId) -> Option<Access> {
        self.clients
            .get(&client)
            .map(|s| Access::new(s.player, s.role))
    }

    fn name_of(&self, client: ClientId) -> String {
        self.clients
            .get(&client)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "кто-то".to_string())
    }

    fn owner_of(&self, token: EntityId) -> Option<PlayerId> {
        self.campaign.scene().token(token).and_then(|t| t.owner)
    }

    fn gm_clients(&self) -> Vec<ClientId> {
        self.clients
            .values()
            .filter(|s| s.role.is_gm_like())
            .map(|s| s.client)
            .collect()
    }

    /// Событие, видимое всем (или только ГМам, если скрыто).
    fn broadcast_event(&self, event: Event, visible_to_players: bool) -> Vec<Outgoing> {
        if visible_to_players {
            vec![Outgoing::broadcast(ServerMsg::Events(vec![event]))]
        } else {
            self.gm_clients()
                .into_iter()
                .map(|c| Outgoing::to(c, ServerMsg::Events(vec![event.clone()])))
                .collect()
        }
    }

    /// Несколько событий с одним правилом видимости.
    fn broadcast_events(&self, events: Vec<Event>, visible_to_players: bool) -> Vec<Outgoing> {
        if events.is_empty() {
            return Vec::new();
        }
        if visible_to_players {
            vec![Outgoing::broadcast(ServerMsg::Events(events))]
        } else {
            self.gm_clients()
                .into_iter()
                .map(|c| Outgoing::to(c, ServerMsg::Events(events.clone())))
                .collect()
        }
    }

    fn broadcast_token_change(&self, token: EntityId) -> Vec<Outgoing> {
        let hidden = self.campaign.scene().token(token).is_some_and(|t| t.hidden);
        self.broadcast_token_change_visible(token, !hidden)
    }

    fn broadcast_token_change_visible(
        &self,
        token: EntityId,
        visible_to_players: bool,
    ) -> Vec<Outgoing> {
        let Some(token_data) = self.campaign.scene().token(token) else {
            return Vec::new();
        };
        self.broadcast_event(
            Event::TokenChanged {
                token: token_data.clone(),
            },
            visible_to_players,
        )
    }

    /// Отправить событие автору и всем ГМам.
    fn whisper(&self, client: ClientId, event: Event) -> Vec<Outgoing> {
        let mut out = vec![Outgoing::to(client, ServerMsg::Events(vec![event.clone()]))];
        for id in self.gm_clients() {
            if id != client {
                out.push(Outgoing::to(id, ServerMsg::Events(vec![event.clone()])));
            }
        }
        out
    }

    /// Событие листа персонажа: владельцу и ГМам.
    fn notify_character(&self, client: ClientId, event: Event) -> Vec<Outgoing> {
        self.whisper(client, event)
    }

    /// Может ли участник менять лист персонажа (владелец или ГМ).
    fn can_edit_character(&self, access: &Access, character: EntityId) -> bool {
        if access.role.is_gm_like() {
            return true;
        }
        self.campaign
            .character(character)
            .is_some_and(|target| target.owner == Some(access.player))
    }
}

/// Применить частичное изменение листа.
///
/// Значения зажимаются в разумные границы, чтобы клиент не мог прислать
/// «характеристику 999» и сломать производные показатели.
fn apply_character_patch(character: &mut ScpCharacter, patch: &CharacterPatch) {
    if let Some(name) = &patch.name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            character.name = trimmed.to_string();
        }
    }
    if let Some(organization) = &patch.organization {
        character.organization = organization.trim().to_string();
    }
    if let Some(access) = patch.access {
        character.access = access;
    }
    if let Some(class) = patch.class {
        character.class = class;
    }
    if let Some(attributes) = patch.attributes {
        for attr in ScpAttr::ALL {
            let value = attributes.get(attr).clamp(1, 30);
            character.attributes.set(attr, value);
        }
    }
    if let Some(lore) = &patch.lore {
        character.lore = lore
            .iter()
            .filter(|field| !field.name.trim().is_empty())
            .cloned()
            .collect();
    }
    if let Some(skills) = &patch.skills {
        for (skill, value) in skills {
            let clamped = (*value).min(20);
            if clamped == 0 {
                character.skills.remove(skill);
            } else {
                character.skills.insert(*skill, clamped);
            }
        }
    }
    character.sync_health_max();
    if let Some(physical) = patch.physical {
        character.physical.current = physical.clamp(0, character.physical.max);
    }
    if let Some(mental) = patch.mental {
        character.mental.current = mental.clamp(0, character.mental.max);
    }
    if let Some(xp) = patch.xp {
        character.xp = xp.max(0);
    }
    if let Some(focus) = patch.class_focus {
        character.class_focus = Some(focus);
    }
    if let Some(quirk) = &patch.quirk {
        character.quirk = quirk.clone();
    }
    if let Some(notes) = &patch.notes {
        character.notes = notes.clone();
    }
    character.sync_health_max();
}

fn deny(client: ClientId, reason: impl Into<String>) -> Vec<Outgoing> {
    vec![Outgoing::to(
        client,
        ServerMsg::Denied {
            reason: reason.into(),
        },
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtt_core::content::{ContentPack, ContentSource, MonsterDraft};
    use vtt_core::entities::{TokenFrame, TokenSize};
    use vtt_core::geom::Vec2;
    use vtt_core::scp::{ScpAttr, ScpCharacter, ScpSkill};

    const PLAYER: ClientId = 10;
    const OTHER_CLIENT: ClientId = 11;

    fn test_server() -> (GameServer, ClientId) {
        let mut campaign = Campaign::new("Тест");
        let id = campaign.next_id();
        let mut token = Token::new(id, "Гоблин", Vec2::new(35.0, 35.0));
        token.hp = Some(vtt_core::entities::Bar::new(7, 7));
        campaign.scene_mut().add_token(token);

        let mut server = GameServer::with_room_code(campaign, "TEST42");
        let host = server.add_host("ГМ");
        (server, host.client)
    }

    fn events(out: &[Outgoing]) -> Vec<Event> {
        out.iter()
            .filter_map(|o| match &o.msg {
                ServerMsg::Events(list) => Some(list.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    #[test]
    fn room_code_has_expected_shape() {
        let mut rng = Rng::new(1);
        for _ in 0..100 {
            let code = room_code(&mut rng);
            assert_eq!(code.len(), 6);
            assert!(code
                .chars()
                .all(|c| ROOM_CODE_ALPHABET.contains(&(c as u8))));
        }
        assert_ne!(room_code(&mut rng), room_code(&mut rng));
    }

    #[test]
    fn hello_requires_correct_code_and_version() {
        let (mut server, _host) = test_server();

        let wrong_code = server.handle(
            PLAYER,
            ClientMsg::Hello {
                name: "Алиса".into(),
                code: "WRONG1".into(),
                protocol: PROTOCOL_VERSION,
                resume: None,
            },
        );
        assert!(matches!(wrong_code[0].msg, ServerMsg::Rejected { .. }));

        let wrong_version = server.handle(
            PLAYER,
            ClientMsg::Hello {
                name: "Алиса".into(),
                code: "TEST42".into(),
                protocol: PROTOCOL_VERSION + 1,
                resume: None,
            },
        );
        assert!(matches!(wrong_version[0].msg, ServerMsg::Rejected { .. }));

        let ok = server.handle(
            PLAYER,
            ClientMsg::Hello {
                name: "Алиса".into(),
                code: "TEST42".into(),
                protocol: PROTOCOL_VERSION,
                resume: None,
            },
        );
        assert!(matches!(ok[0].msg, ServerMsg::Welcome(_)));
        assert!(matches!(ok[1].msg, ServerMsg::Snapshot(_)));
    }

    #[test]
    fn player_cannot_move_gm_token() {
        let (mut server, _host) = test_server();
        let welcome = server.join(PLAYER, "Алиса", Role::Player);

        let token = server.campaign.scene().tokens[0].id;
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::MoveToken {
                token,
                to: Vec2::new(500.0, 500.0),
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));

        // Токен не сдвинулся.
        assert_eq!(server.campaign.scene().tokens[0].pos, Vec2::new(35.0, 35.0));
        assert_eq!(welcome.role, Role::Player);
    }

    #[test]
    fn player_moves_own_token_and_everyone_sees_it() {
        let (mut server, _host) = test_server();
        let welcome = server.join(PLAYER, "Алиса", Role::Player);
        let token = server.campaign.scene().tokens[0].id;
        server.campaign.scene_mut().token_mut(token).unwrap().owner = Some(welcome.player);

        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::MoveToken {
                token,
                to: Vec2::new(105.0, 105.0),
            }),
        );
        let evs = events(&out);
        assert_eq!(evs.len(), 1);
        assert!(matches!(evs[0], Event::TokenMoved { .. }));
        // Снап к сетке: (105,105) → центр клетки (105,105).
        assert_eq!(
            server.campaign.scene().tokens[0].pos,
            Vec2::new(105.0, 105.0)
        );
    }

    #[test]
    fn locked_token_cannot_be_moved_by_player() {
        let (mut server, _host) = test_server();
        let welcome = server.join(PLAYER, "Алиса", Role::Player);
        let token = server.campaign.scene().tokens[0].id;
        {
            let t = server.campaign.scene_mut().token_mut(token).unwrap();
            t.owner = Some(welcome.player);
            t.locked = true;
        }
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::MoveToken {
                token,
                to: Vec2::new(200.0, 200.0),
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn hidden_token_is_invisible_to_players() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let token = server.campaign.scene().tokens[0].id;
        let gm_snapshot = server.snapshot(HOST_CLIENT).unwrap();
        let player_snapshot = server.snapshot(PLAYER).unwrap();
        assert_eq!(gm_snapshot.scene.tokens.len(), 1);
        assert_eq!(player_snapshot.scene.tokens.len(), 1);

        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetTokenHidden {
                token,
                hidden: true,
            }),
        );

        assert_eq!(server.snapshot(HOST_CLIENT).unwrap().scene.tokens.len(), 1);
        assert_eq!(server.snapshot(PLAYER).unwrap().scene.tokens.len(), 0);
    }

    #[test]
    fn hidden_token_events_go_only_to_gm() {
        let (mut server, host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let token = server.campaign.scene().tokens[0].id;
        server.campaign.scene_mut().token_mut(token).unwrap().hidden = true;

        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 3 }),
        );
        assert!(!out.is_empty());
        assert!(
            out.iter().all(|o| o.to == Some(host)),
            "утечка скрытого события игроку"
        );
    }

    #[test]
    fn only_gm_creates_tokens() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);

        let denied = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::CreateToken {
                name: "Крыса".into(),
                pos: Vec2::new(70.0, 70.0),
                size: TokenSize::Small,
            }),
        );
        assert!(matches!(denied[0].msg, ServerMsg::Denied { .. }));

        let created = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::CreateToken {
                name: "Крыса".into(),
                pos: Vec2::new(70.0, 70.0),
                size: TokenSize::Small,
            }),
        );
        assert!(events(&created)
            .iter()
            .any(|e| matches!(e, Event::TokenCreated { .. })));
        assert_eq!(server.campaign.scene().tokens.len(), 2);
    }

    #[test]
    fn damage_clamps_at_zero() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 100 }),
        );
        assert_eq!(server.campaign.scene().tokens[0].hp.unwrap().value, 0);
    }

    #[test]
    fn rolls_are_server_side_and_validated() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);

        let ok = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::Roll {
                expr: "2d6+3".into(),
                mode: vtt_core::dice::RollMode::Normal,
                to_gm_only: false,
            }),
        );
        let evs = events(&ok);
        assert!(evs.iter().any(|e| matches!(e, Event::Rolled { .. })));

        let bad = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::Roll {
                expr: "2d6+".into(),
                mode: vtt_core::dice::RollMode::Normal,
                to_gm_only: false,
            }),
        );
        assert!(matches!(bad[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn whisper_goes_to_author_and_gm() {
        let (mut server, host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);

        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::Chat {
                text: "только для ГМа".into(),
                to_gm_only: true,
            }),
        );
        assert!(out.iter().any(|o| o.to == Some(PLAYER)));
        assert!(out.iter().any(|o| o.to == Some(host)));
        assert!(out.iter().all(|o| o.to.is_some()));
    }

    #[test]
    fn empty_chat_is_rejected() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::Chat {
                text: "   ".into(),
                to_gm_only: false,
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn initiative_and_turn_order() {
        let (mut server, _host) = test_server();
        let first = server.campaign.scene().tokens[0].id;
        let created = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::CreateToken {
                name: "Второй".into(),
                pos: Vec2::new(140.0, 70.0),
                size: TokenSize::Medium,
            }),
        );
        let second = match events(&created).iter().find_map(|e| match e {
            Event::TokenCreated { token } => Some(token.id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("токен не создан"),
        };

        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetInitiative {
                token: first,
                value: 20,
            }),
        );
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetInitiative {
                token: second,
                value: 5,
            }),
        );
        assert_eq!(server.campaign.combat.order.len(), 2);
        assert_eq!(server.campaign.combat.order[0].token, first);

        let out = server.handle(HOST_CLIENT, ClientMsg::Intent(Intent::NextTurn));
        let evs = events(&out);
        assert!(matches!(
            evs[0],
            Event::TurnChanged {
                round: 1,
                active: Some(id),
                ..
            } if id == second
        ));
    }

    #[test]
    fn players_cannot_change_scene() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::ChangeScene {
                scene: server.campaign.active_scene,
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn unknown_client_is_denied() {
        let (mut server, _host) = test_server();
        let out = server.handle(
            999,
            ClientMsg::Intent(Intent::Chat {
                text: "привет".into(),
                to_gm_only: false,
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    // --- M1: компендиум, отдых, смерть, концентрация, туман, хендауты ---

    fn add_bestiary(server: &mut GameServer) {
        server.campaign.content.add_pack(ContentPack {
            source: ContentSource::srd(),
            monsters: vec![MonsterDraft {
                name: "Гоблин".into(),
                armor_class: 15,
                hit_points: 7,
                hit_dice: "2d6".into(),
                speed_ft: 30,
                abilities: vtt_core::entities::Abilities::new(8, 14, 10, 10, 8, 8),
                challenge_rating: 0.25,
                proficiency_bonus: 2,
                traits: Vec::new(),
                actions: Vec::new(),
            }],
            spells: Vec::new(),
            items: Vec::new(),
        });
    }

    #[test]
    fn spawn_monster_from_compendium() {
        let (mut server, _host) = test_server();
        add_bestiary(&mut server);

        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SpawnMonster {
                name: "гоблин".into(),
                pos: Vec2::new(70.0, 70.0),
            }),
        );
        let created = events(&out)
            .into_iter()
            .find_map(|event| match event {
                Event::TokenCreated { token } => Some(token),
                _ => None,
            })
            .expect("токен должен создаться");
        assert_eq!(created.name, "Гоблин");
        assert_eq!(created.hp.expect("хиты").max, 7);
        assert_eq!(created.ac.expect("КД").value, 15);
    }

    #[test]
    fn spawn_unknown_monster_is_denied() {
        let (mut server, _host) = test_server();
        add_bestiary(&mut server);
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SpawnMonster {
                name: "дракон".into(),
                pos: Vec2::ZERO,
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn long_rest_heals_to_max() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 4 }),
        );
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::Rest { token, long: true }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::TokenChanged { .. })));
        let hp = server.campaign.scene().token(token).unwrap().hp.unwrap();
        assert_eq!(hp.value, hp.max);
    }

    #[test]
    fn short_rest_does_not_heal() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 4 }),
        );
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::Rest { token, long: false }),
        );
        assert_eq!(
            server
                .campaign
                .scene()
                .token(token)
                .unwrap()
                .hp
                .unwrap()
                .value,
            3
        );
    }

    #[test]
    fn death_save_requires_zero_hp() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        let out = server.handle(HOST_CLIENT, ClientMsg::Intent(Intent::DeathSave { token }));
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn death_saves_resolve_to_an_outcome() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 100 }),
        );

        let mut last = DeathOutcome::Continue;
        for _ in 0..10 {
            let out = server.handle(HOST_CLIENT, ClientMsg::Intent(Intent::DeathSave { token }));
            if let Some(outcome) = events(&out).into_iter().find_map(|event| match event {
                Event::DeathSaveResult { outcome, .. } => Some(outcome),
                _ => None,
            }) {
                last = outcome;
                if outcome != DeathOutcome::Continue {
                    break;
                }
            }
        }
        assert!(
            matches!(
                last,
                DeathOutcome::Dead | DeathOutcome::Stabilized | DeathOutcome::Revived
            ),
            "спасброски должны закончиться исходом, получили {last:?}"
        );
    }

    #[test]
    fn damage_while_concentrating_emits_check() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetConcentration {
                token,
                effect: Some("Благословение".into()),
            }),
        );
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 5 }),
        );
        let check = events(&out).into_iter().find_map(|event| match event {
            Event::ConcentrationCheck { dc, effect, .. } => Some((dc, effect)),
            _ => None,
        });
        let (dc, effect) = check.expect("должен быть спасбросок на концентрацию");
        assert_eq!(dc, 10, "СЛ не может быть ниже 10");
        assert_eq!(effect, "Благословение");
    }

    #[test]
    fn big_damage_raises_concentration_dc() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetTokenHp { token, value: 100 }),
        );
        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetConcentration {
                token,
                effect: Some("Полёт".into()),
            }),
        );
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DamageToken { token, amount: 44 }),
        );
        let dc = events(&out).into_iter().find_map(|event| match event {
            Event::ConcentrationCheck { dc, .. } => Some(dc),
            _ => None,
        });
        assert_eq!(dc, Some(22), "СЛ = половина урона, если она больше 10");
    }

    #[test]
    fn fog_can_be_added_and_cleared() {
        let (mut server, _host) = test_server();
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::AddFog {
                center: Vec2::new(70.0, 70.0),
                radius: 40.0,
            }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::FogChanged { shapes } if shapes.len() == 1)));
        assert_eq!(server.campaign.scene().fog.shapes.len(), 1);

        let out = server.handle(HOST_CLIENT, ClientMsg::Intent(Intent::ClearFog));
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::FogChanged { shapes } if shapes.is_empty())));
        assert!(server.campaign.scene().fog.shapes.is_empty());
    }

    #[test]
    fn players_cannot_paint_fog() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::AddFog {
                center: Vec2::ZERO,
                radius: 10.0,
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn handout_is_stored_and_broadcast() {
        let (mut server, _host) = test_server();
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::PostHandout {
                title: "Письмо".into(),
                text: "Текст письма".into(),
            }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::HandoutPosted { .. })));
        assert_eq!(server.campaign.handouts.len(), 1);
        assert_eq!(server.campaign.handouts[0].title, "Письмо");
    }

    #[test]
    fn players_cannot_post_handouts() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::PostHandout {
                title: "x".into(),
                text: "y".into(),
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn resume_keeps_player_identity() {
        let (mut server, _host) = test_server();
        let welcome = server.join(PLAYER, "Алиса", Role::Player);
        let player = welcome.player;
        let token = server.campaign.scene().tokens[0].id;
        server.campaign.scene_mut().token_mut(token).unwrap().owner = Some(player);

        let departed = server.leave(PLAYER);
        assert_eq!(departed, Some(player));
        assert!(
            !server
                .players()
                .iter()
                .find(|p| p.player == player)
                .unwrap()
                .connected
        );

        let out = server.handle(
            OTHER_CLIENT,
            ClientMsg::Hello {
                name: "Алиса".into(),
                code: "TEST42".into(),
                protocol: PROTOCOL_VERSION,
                resume: Some(player),
            },
        );
        assert!(matches!(out[0].msg, ServerMsg::Welcome(_)));
        assert_eq!(
            server.session(OTHER_CLIENT).map(|s| s.player),
            Some(player),
            "resume должен вернуть того же игрока"
        );
        assert_eq!(
            server.campaign.scene().token(token).unwrap().owner,
            Some(player),
            "владение токеном должно сохраниться"
        );
    }

    #[test]
    fn resume_for_unknown_player_creates_new_session() {
        let (mut server, _host) = test_server();
        let out = server.handle(
            PLAYER,
            ClientMsg::Hello {
                name: "Новый".into(),
                code: "TEST42".into(),
                protocol: PROTOCOL_VERSION,
                resume: Some(PlayerId(999)),
            },
        );
        assert!(matches!(out[0].msg, ServerMsg::Welcome(_)));
        let player = server.session(PLAYER).unwrap().player;
        assert_ne!(player, PlayerId(999));
    }

    // --- M3: SCP-лист персонажа ---

    fn newest_character(out: &[Outgoing]) -> ScpCharacter {
        events(out)
            .into_iter()
            .find_map(|event| match event {
                Event::CharacterUpdated { character } => Some(character),
                _ => None,
            })
            .expect("событие CharacterUpdated")
    }

    fn create_character(server: &mut GameServer, client: ClientId, name: &str) -> ScpCharacter {
        let out = server.handle(
            client,
            ClientMsg::Intent(Intent::CreateCharacter {
                name: name.into(),
                class: vtt_core::scp::ScpClass::Agent,
                organization: "Фонд SCP".into(),
                access: vtt_core::scp::AccessLevel::First,
            }),
        );
        newest_character(&out)
    }

    #[test]
    fn character_creation_sets_owner_and_derived_health() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let player = server.session(PLAYER).unwrap().player;
        let character = create_character(&mut server, PLAYER, "Смит");

        assert_eq!(character.owner, Some(player));
        // ВЫН 1 → 15 физического, ПСИ 1 → 10 психического.
        assert_eq!(character.physical.max, 15);
        assert_eq!(character.mental.max, 10);
        assert_eq!(character.xp, 0);
        assert_eq!(server.campaign.characters.len(), 1);
    }

    #[test]
    fn character_is_visible_only_to_owner_and_gm() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        server.join(OTHER_CLIENT, "Боб", Role::Player);
        create_character(&mut server, PLAYER, "Смит");

        assert_eq!(server.snapshot(PLAYER).unwrap().characters.len(), 1);
        assert_eq!(server.snapshot(OTHER_CLIENT).unwrap().characters.len(), 0);
        assert_eq!(server.snapshot(HOST_CLIENT).unwrap().characters.len(), 1);
    }

    #[test]
    fn other_player_cannot_edit_character() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        server.join(OTHER_CLIENT, "Боб", Role::Player);
        let character = create_character(&mut server, PLAYER, "Смит");

        let out = server.handle(
            OTHER_CLIENT,
            ClientMsg::Intent(Intent::UpdateCharacter {
                character: character.id,
                patch: CharacterPatch {
                    name: Some("Угнан".into()),
                    ..CharacterPatch::default()
                },
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
        assert_eq!(
            server.campaign.character(character.id).unwrap().name,
            "Смит"
        );
    }

    #[test]
    fn character_update_clamps_values() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let character = create_character(&mut server, PLAYER, "Смит");

        let mut attributes = character.attributes;
        attributes.strength = 99;
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::UpdateCharacter {
                character: character.id,
                patch: CharacterPatch {
                    attributes: Some(attributes),
                    physical: Some(9999),
                    ..CharacterPatch::default()
                },
            }),
        );
        let updated = newest_character(&out);
        assert_eq!(updated.attributes.strength, 30, "характеристика зажимается");
        assert_eq!(updated.physical.current, updated.physical.max);
    }

    #[test]
    fn character_check_and_sanity_are_rolled_on_server() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let character = create_character(&mut server, PLAYER, "Смит");

        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::RollCharacterCheck {
                character: character.id,
                skill: Some(ScpSkill::Pistols),
                attribute: None,
                dc: 15,
            }),
        );
        let checked = events(&out).into_iter().find_map(|event| match event {
            Event::CharacterCheck { result, .. } => Some(result),
            _ => None,
        });
        let result = checked.expect("проверка навыка");
        // ВСП 1 + навык 0 = бонус 1.
        assert_eq!(result.bonus, 1);
        assert_eq!(result.dc, 15);

        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::RollSanity {
                character: character.id,
            }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::SanityRolled { .. })));
    }

    #[test]
    fn xp_and_level_up_rules() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let character = create_character(&mut server, PLAYER, "Смит");

        // Игрок не может выдать себе опыт.
        let denied = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::GrantXp {
                character: character.id,
                amount: 100,
            }),
        );
        assert!(matches!(denied[0].msg, ServerMsg::Denied { .. }));

        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::GrantXp {
                character: character.id,
                amount: 100,
            }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::XpGranted { .. })));
        assert!(server.campaign.character(character.id).unwrap().xp >= 100);

        // Повышение характеристики тратит 50 опыта.
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::LevelUpAttribute {
                character: character.id,
                attribute: ScpAttr::Strength,
            }),
        );
        let updated = newest_character(&out);
        assert_eq!(updated.attributes.strength, 2);
    }

    #[test]
    fn character_token_links_to_sheet() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let character = create_character(&mut server, PLAYER, "Смит");

        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SpawnCharacterToken {
                character: character.id,
                pos: Vec2::new(140.0, 70.0),
            }),
        );
        let token = events(&out).into_iter().find_map(|event| match event {
            Event::TokenCreated { token } => Some(token),
            _ => None,
        });
        let token = token.expect("токен создан");
        assert_eq!(token.character, Some(character.id));
        assert_eq!(token.name, "Смит");
        assert_eq!(token.hp.expect("хиты").max, 15);
    }

    // --- M4: стены и двери ---

    #[test]
    fn walls_can_be_added_toggled_and_removed() {
        let (mut server, _host) = test_server();
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::AddWall {
                a: Vec2::new(210.0, 0.0),
                b: Vec2::new(210.0, 400.0),
                door: true,
            }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::WallsChanged { walls } if walls.len() == 1)));
        assert_eq!(server.campaign.scene().walls.len(), 1);

        // Дверь закрыта — не видно; открыли — видно.
        let origin = Vec2::new(70.0, 70.0);
        let target = Vec2::new(400.0, 70.0);
        assert!(server.campaign.scene().los_blocked(origin, target));

        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::ToggleDoor { index: 0 }),
        );
        assert!(!server.campaign.scene().los_blocked(origin, target));

        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::RemoveWall { index: 0 }),
        );
        assert!(server.campaign.scene().walls.is_empty());
    }

    #[test]
    fn players_cannot_edit_walls() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);
        let out = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::AddWall {
                a: Vec2::ZERO,
                b: Vec2::new(100.0, 0.0),
                door: false,
            }),
        );
        assert!(matches!(out[0].msg, ServerMsg::Denied { .. }));
    }

    #[test]
    fn token_size_image_and_frame_can_be_changed() {
        let (mut server, _host) = test_server();
        let token = server.campaign.scene().tokens[0].id;

        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetTokenSize {
                token,
                size: TokenSize::Large,
            }),
        );
        assert!(matches!(
            events(&out).as_slice(),
            [Event::TokenChanged { .. }]
        ));
        assert_eq!(
            server.campaign.scene().token(token).unwrap().size,
            TokenSize::Large
        );

        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetTokenImage {
                token,
                image: Some("tokens/hero.png".into()),
            }),
        );
        assert_eq!(
            server
                .campaign
                .scene()
                .token(token)
                .unwrap()
                .image
                .as_deref(),
            Some("tokens/hero.png")
        );

        server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetTokenFrame {
                token,
                frame: TokenFrame::Double,
                color: [230, 90, 90],
            }),
        );
        let token_data = server.campaign.scene().token(token).unwrap();
        assert_eq!(token_data.frame, TokenFrame::Double);
        assert_eq!(token_data.frame_color, [230, 90, 90]);
    }

    #[test]
    fn scene_bounds_are_gm_only_and_clamped() {
        let (mut server, _host) = test_server();
        server.join(PLAYER, "Алиса", Role::Player);

        let denied = server.handle(
            PLAYER,
            ClientMsg::Intent(Intent::SetSceneBounds {
                width_px: 1000.0,
                height_px: 1000.0,
            }),
        );
        assert!(matches!(denied[0].msg, ServerMsg::Denied { .. }));

        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::SetSceneBounds {
                width_px: 99_999.0,
                height_px: 100.0,
            }),
        );
        assert!(events(&out)
            .iter()
            .any(|event| matches!(event, Event::SceneBoundsChanged { .. })));
        let scene = server.campaign.scene();
        assert_eq!(scene.width_px, 20_000.0);
        assert_eq!(scene.height_px, 200.0);
    }
}
