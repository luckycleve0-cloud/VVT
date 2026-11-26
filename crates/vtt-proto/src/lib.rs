//! Протокол между клиентом и сервером.
//!
//! Модель простая и намеренно асимметричная:
//!
//! - клиент шлёт **намерения** ([`Intent`]) — «хочу подвинуть токен»,
//!   «бросить кубик», «написать в чат»;
//! - сервер валидирует права, применяет и рассылает **события** ([`Event`]);
//! - при подключении клиент получает [`Snapshot`] — срез состояния.
//!
//! Клиент **никогда** не меняет состояние сам. Это даёт бесплатно автосохранение,
//! undo и невозможность «подкрутить» результат кубика.
//!
//! Транспорт и кодирование живут в `vtt-net`; этот крейт отвечает только за
//! форму сообщений и версию протокола.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vtt_core::combat::Combat;
use vtt_core::dice::{RollMode, RollResult};
use vtt_core::entities::{Token, TokenFrame, TokenSize};
use vtt_core::geom::Vec2;
use vtt_core::grid::GridKind;
use vtt_core::ids::EntityId;
use vtt_core::scene::Scene;
use vtt_core::scp::{
    AccessLevel, Attributes, CheckResult, SanityOutcome, ScpAttr, ScpCharacter, ScpClass, ScpSkill,
};

// Удобные ре-экспорты: клиенту обычно нужны именно эти типы вместе с протоколом.
pub use vtt_core::ids::PlayerId;
pub use vtt_core::permissions::Role;

/// Версия протокола. Несовпадение — понятная ошибка, а не «непонятный краш».
pub const PROTOCOL_VERSION: u16 = 1;

/// Идентификатор соединения (выдаётся транспортом/сервером).
pub type ClientId = u64;

/// Сообщения от клиента к серверу.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientMsg {
    /// Первое сообщение: имя и код комнаты.
    Hello {
        name: String,
        code: String,
        protocol: u16,
        /// Переподключение: идентификатор прошлой сессии (если сохранили).
        #[serde(default)]
        resume: Option<PlayerId>,
    },
    /// Намерение что-то сделать.
    Intent(Intent),
    /// Проверка задержки.
    Ping { nonce: u64 },
    /// Корректное отключение.
    Bye,
}

/// Что клиент просит сделать. Только намерения — без прямых мутаций.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    MoveToken {
        token: EntityId,
        to: Vec2,
    },
    CreateToken {
        name: String,
        pos: Vec2,
        size: TokenSize,
    },
    DeleteToken {
        token: EntityId,
    },
    /// Установить текущие хиты.
    SetTokenHp {
        token: EntityId,
        value: i32,
    },
    /// Нанести урон (с учётом временных хитов на сервере).
    DamageToken {
        token: EntityId,
        amount: i32,
    },
    SetTokenHidden {
        token: EntityId,
        hidden: bool,
    },
    SetInitiative {
        token: EntityId,
        value: i32,
    },
    NextTurn,
    ChangeScene {
        scene: EntityId,
    },
    Chat {
        text: String,
        /// Видно только ГМу (шёпот).
        to_gm_only: bool,
    },
    Roll {
        expr: String,
        mode: RollMode,
        to_gm_only: bool,
    },
    /// Создать токен из монстра компендиума (только ГМ).
    CreateMonsterToken {
        monster: EntityId,
        pos: Vec2,
    },
    /// Создать токен из монстра по имени (удобно из консоли).
    SpawnMonster {
        name: String,
        pos: Vec2,
    },
    /// Отдых: `long = false` — короткий, `true` — длинный.
    Rest {
        token: EntityId,
        long: bool,
    },
    /// Бросок спасброска от смерти (только при 0 хитов).
    DeathSave {
        token: EntityId,
    },
    /// Установить/снять концентрацию (название эффекта).
    SetConcentration {
        token: EntityId,
        effect: Option<String>,
    },
    /// Показать игрокам хендаут (только ГМ).
    PostHandout {
        title: String,
        text: String,
    },
    /// Закрасить круг тумана (только ГМ).
    AddFog {
        center: Vec2,
        radius: f32,
    },
    /// Открыть весь туман сцены (только ГМ).
    ClearFog,

    // --- SCP-лист персонажа ---
    /// Создать своего персонажа по системе SCP.
    CreateCharacter {
        name: String,
        class: ScpClass,
        organization: String,
        access: AccessLevel,
    },
    /// Изменить лист (владелец или ГМ).
    UpdateCharacter {
        character: EntityId,
        patch: CharacterPatch,
    },
    /// Проверка навыка или характеристики (владелец или ГМ).
    RollCharacterCheck {
        character: EntityId,
        /// Навык; если не задан — проверяется характеристика.
        skill: Option<ScpSkill>,
        attribute: Option<ScpAttr>,
        dc: i32,
    },
    /// Бросок психики (d10 → психоз или d4 → подъём).
    RollSanity {
        character: EntityId,
    },
    /// Начислить опыт (только ГМ).
    GrantXp {
        character: EntityId,
        amount: i32,
    },
    /// Потратить опыт на характеристику (владелец или ГМ).
    LevelUpAttribute {
        character: EntityId,
        attribute: ScpAttr,
    },
    /// Поставить токен персонажа на сцену (только ГМ).
    SpawnCharacterToken {
        character: EntityId,
        pos: Vec2,
    },

    // --- M4: стены и свет ---
    /// Добавить стену (только ГМ).
    AddWall {
        a: Vec2,
        b: Vec2,
        /// `true` — дверь (можно открыть), иначе глухая стена.
        door: bool,
    },
    /// Открыть/закрыть дверь по индексу стены (только ГМ).
    ToggleDoor {
        index: usize,
    },
    /// Удалить стену по индексу (только ГМ).
    RemoveWall {
        index: usize,
    },
    /// Задать радиус обзора токена в футах (только ГМ).
    SetTokenVision {
        token: EntityId,
        range_ft: f32,
    },
    /// Изменить размер токена (владелец или ГМ).
    SetTokenSize {
        token: EntityId,
        size: TokenSize,
    },
    /// Задать картинку токена (путь к файлу) или убрать её.
    SetTokenImage {
        token: EntityId,
        image: Option<String>,
    },
    /// Изменить границы сцены (только ГМ).
    SetSceneBounds {
        width_px: f32,
        height_px: f32,
    },
    /// Рамка и цвет токена.
    SetTokenFrame {
        token: EntityId,
        frame: TokenFrame,
        color: [u8; 3],
    },
    /// Прозрачность токена (0.05..1.0).
    SetTokenOpacity {
        token: EntityId,
        opacity: f32,
    },
    /// Назначить роль участнику (только ГМ).
    SetPlayerRole {
        player: PlayerId,
        role: Role,
    },
    /// Настройки сетки сцены (только ГМ).
    SetGrid {
        kind: GridKind,
        cell_px: f32,
        feet_per_cell: f32,
    },
    /// Фон сцены: путь к картинке (только ГМ).
    SetSceneBackground {
        path: Option<String>,
    },
}

/// Изменение листа персонажа: всё опционально, применяется только заданное.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CharacterPatch {
    pub name: Option<String>,
    pub organization: Option<String>,
    pub access: Option<AccessLevel>,
    pub class: Option<ScpClass>,
    pub attributes: Option<Attributes>,
    pub skills: Option<BTreeMap<ScpSkill, u8>>,
    /// Текущее физическое здоровье.
    pub physical: Option<i32>,
    /// Текущее психическое здоровье.
    pub mental: Option<i32>,
    pub xp: Option<i32>,
    pub class_focus: Option<ScpSkill>,
    pub quirk: Option<String>,
    pub notes: Option<String>,
    #[serde(default)]
    pub lore: Option<Vec<vtt_core::scp::LoreField>>,
}

/// Итог спасброска от смерти.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathOutcome {
    /// Раунд продолжается: ни стабилизации, ни смерти.
    Continue,
    /// Три успеха — стабилизировался.
    Stabilized,
    /// Три провала — умер.
    Dead,
    /// Натуральная 20 — пришёл в себя с 1 хитом.
    Revived,
}

/// Что произошло. Рассылается всем (с учётом видимости).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    TokenMoved {
        token: EntityId,
        to: Vec2,
    },
    TokenCreated {
        token: Token,
    },
    TokenRemoved {
        token: EntityId,
    },
    TokenChanged {
        token: Token,
    },
    ChatPosted {
        from: PlayerId,
        from_name: String,
        text: String,
        to_gm_only: bool,
    },
    Rolled {
        from: PlayerId,
        from_name: String,
        result: RollResult,
        to_gm_only: bool,
    },
    InitiativeSet {
        token: EntityId,
        value: i32,
        name: String,
    },
    TurnChanged {
        round: u32,
        active: Option<EntityId>,
        active_name: Option<String>,
    },
    SceneChanged {
        scene: EntityId,
        name: String,
    },
    /// Нанесён урон концентрирующемуся: спасбросок на концентрацию.
    ConcentrationCheck {
        token: EntityId,
        dc: i32,
        effect: String,
    },
    /// Результат спасброска от смерти.
    DeathSaveResult {
        token: EntityId,
        roll: RollResult,
        successes: u8,
        failures: u8,
        outcome: DeathOutcome,
    },
    /// ГМ показал хендаут.
    HandoutPosted {
        title: String,
        text: String,
    },
    /// Изменился туман.
    FogChanged {
        shapes: Vec<vtt_core::scene::FogShape>,
    },
    /// Лист персонажа обновился (снапшот для владельца/ГМ).
    CharacterUpdated {
        character: ScpCharacter,
    },
    /// Бросок проверки персонажа.
    CharacterCheck {
        character: EntityId,
        name: String,
        skill: Option<ScpSkill>,
        result: CheckResult,
    },
    /// Бросок психики.
    SanityRolled {
        character: EntityId,
        name: String,
        outcome: SanityOutcome,
    },
    /// Начислен опыт.
    XpGranted {
        character: EntityId,
        name: String,
        gained: i32,
        total: i32,
    },
    /// Изменились стены/двери.
    WallsChanged {
        walls: Vec<vtt_core::scene::Wall>,
    },
    /// Изменились границы сцены.
    SceneBoundsChanged {
        width_px: f32,
        height_px: f32,
    },
    /// Изменилась сетка.
    GridChanged {
        kind: GridKind,
        cell_px: f32,
        feet_per_cell: f32,
    },
    /// Изменился фон сцены.
    SceneBackgroundChanged {
        path: Option<String>,
    },
}

/// Кто подключён.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub player: PlayerId,
    pub name: String,
    pub role: Role,
    pub connected: bool,
}

/// Срез состояния при подключении.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub campaign: String,
    pub scene: Scene,
    pub combat: Combat,
    pub players: Vec<PlayerInfo>,
    /// Персонажи, доступные этому клиенту (ГМ видит всех, игрок — своих).
    #[serde(default)]
    pub characters: Vec<ScpCharacter>,
    /// Кем сервер считает этого клиента.
    pub you: PlayerInfo,
}

/// Подтверждение входа.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Welcome {
    pub client: ClientId,
    pub player: PlayerId,
    pub role: Role,
    pub campaign: String,
    pub protocol: u16,
}

/// Сообщения от сервера к клиенту.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome(Welcome),
    Snapshot(Box<Snapshot>),
    Events(Vec<Event>),
    /// Действие отклонено (права, лимиты, ошибки ввода).
    Denied {
        reason: String,
    },
    /// Информационное сообщение (кто-то подключился, сохранение и т.п.).
    Notice {
        text: String,
    },
    Pong {
        nonce: u64,
    },
    /// Сервер завершает соединение.
    Rejected {
        reason: String,
    },
}

impl ServerMsg {
    /// Короткое описание для логов и CLI.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Welcome(w) => format!(
                "добро пожаловать: {} ({}) в «{}»",
                w.player.0,
                w.role.name_ru(),
                w.campaign
            ),
            Self::Snapshot(snapshot) => format!(
                "снапшот: сцена «{}», токенов {}, раунд {}",
                snapshot.scene.name,
                snapshot.scene.tokens.len(),
                snapshot.combat.round
            ),
            Self::Events(events) => format!("событий: {}", events.len()),
            Self::Denied { reason } => format!("отказано: {reason}"),
            Self::Notice { text } => format!("сообщение: {text}"),
            Self::Pong { nonce } => format!("pong {nonce}"),
            Self::Rejected { reason } => format!("отклонено: {reason}"),
        }
    }
}

/// Проверить совместимость версии протокола.
pub fn check_protocol_version(remote: u16) -> Result<(), String> {
    if remote == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(format!(
            "версия протокола {remote} не совпадает с {PROTOCOL_VERSION}; обновите приложение"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vtt_core::entities::Hp;

    #[test]
    fn protocol_version_checked() {
        assert!(check_protocol_version(PROTOCOL_VERSION).is_ok());
        let err = check_protocol_version(PROTOCOL_VERSION + 1).unwrap_err();
        assert!(err.contains("обновите"));
    }

    #[test]
    fn intent_roundtrip() {
        let intent = Intent::MoveToken {
            token: EntityId(7),
            to: Vec2::new(100.0, 50.0),
        };
        let json = serde_json::to_string(&ClientMsg::Intent(intent.clone())).unwrap();
        let back: ClientMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ClientMsg::Intent(intent));
    }

    #[test]
    fn hello_without_resume_still_parses() {
        // Старый клиент не шлёт поле resume — сервер должен это принять.
        let json = r#"{"hello":{"name":"Алиса","code":"ABC123","protocol":1}}"#;
        let msg: ClientMsg = serde_json::from_str(json).unwrap();
        match msg {
            ClientMsg::Hello { name, resume, .. } => {
                assert_eq!(name, "Алиса");
                assert!(resume.is_none());
            }
            _ => panic!("ожидался Hello"),
        }
    }

    #[test]
    fn death_and_rest_intents_roundtrip() {
        for intent in [
            Intent::Rest {
                token: EntityId(2),
                long: true,
            },
            Intent::DeathSave { token: EntityId(2) },
            Intent::SetConcentration {
                token: EntityId(2),
                effect: Some("Благословение".into()),
            },
            Intent::PostHandout {
                title: "Письмо".into(),
                text: "Текст письма".into(),
            },
            Intent::AddFog {
                center: Vec2::new(70.0, 70.0),
                radius: 50.0,
            },
            Intent::ClearFog,
        ] {
            let json = serde_json::to_string(&ClientMsg::Intent(intent.clone())).unwrap();
            let back: ClientMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(back, ClientMsg::Intent(intent));
        }
    }

    #[test]
    fn server_msg_roundtrip() {
        let welcome = ServerMsg::Welcome(Welcome {
            client: 3,
            player: PlayerId(1),
            role: Role::Player,
            campaign: "Кампания".to_string(),
            protocol: PROTOCOL_VERSION,
        });
        let json = serde_json::to_string(&welcome).unwrap();
        assert_eq!(serde_json::from_str::<ServerMsg>(&json).unwrap(), welcome);
    }

    #[test]
    fn snapshot_carries_scene_and_combat() {
        let scene = Scene::new(EntityId(1), "Тест");
        let snapshot = Snapshot {
            campaign: "Тест".into(),
            scene,
            combat: Combat::default(),
            players: Vec::new(),
            characters: Vec::new(),
            you: PlayerInfo {
                player: PlayerId(1),
                name: "Игрок".into(),
                role: Role::Player,
                connected: true,
            },
        };
        let json = serde_json::to_string(&ServerMsg::Snapshot(Box::new(snapshot))).unwrap();
        let back: ServerMsg = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMsg::Snapshot(_)));
    }

    #[test]
    fn hp_type_is_available_through_reexports() {
        // Просто проверяем, что core доступен и типы совпадают.
        let hp = Hp::new(10);
        assert_eq!(hp.current, 10);
    }

    #[test]
    fn character_intents_roundtrip() {
        let patch = CharacterPatch {
            name: Some("Смит".into()),
            attributes: Some(Attributes::default()),
            physical: Some(10),
            ..CharacterPatch::default()
        };
        for intent in [
            Intent::CreateCharacter {
                name: "Смит".into(),
                class: ScpClass::Agent,
                organization: "Фонд SCP".into(),
                access: AccessLevel::First,
            },
            Intent::UpdateCharacter {
                character: EntityId(3),
                patch,
            },
            Intent::RollCharacterCheck {
                character: EntityId(3),
                skill: Some(ScpSkill::Pistols),
                attribute: Some(ScpAttr::Agility),
                dc: 15,
            },
            Intent::RollSanity {
                character: EntityId(3),
            },
            Intent::GrantXp {
                character: EntityId(3),
                amount: 100,
            },
            Intent::LevelUpAttribute {
                character: EntityId(3),
                attribute: ScpAttr::Strength,
            },
            Intent::AddWall {
                a: Vec2::ZERO,
                b: Vec2::new(100.0, 0.0),
                door: true,
            },
            Intent::ToggleDoor { index: 0 },
            Intent::RemoveWall { index: 1 },
            Intent::SetTokenVision {
                token: EntityId(2),
                range_ft: 30.0,
            },
            Intent::SetTokenSize {
                token: EntityId(2),
                size: TokenSize::Large,
            },
            Intent::SetTokenImage {
                token: EntityId(2),
                image: Some("tokens/hero.png".into()),
            },
            Intent::SetSceneBounds {
                width_px: 2000.0,
                height_px: 1500.0,
            },
        ] {
            let json = serde_json::to_string(&ClientMsg::Intent(intent.clone())).unwrap();
            let back: ClientMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(back, ClientMsg::Intent(intent));
        }
    }

    #[test]
    fn describe_does_not_panic() {
        assert!(ServerMsg::Denied {
            reason: "нет прав".into()
        }
        .describe()
        .contains("нет прав"));
    }
}
