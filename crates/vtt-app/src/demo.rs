//! Демо: полный сценарий без сети — проверка ядра и живая документация.
//!
//! Запуск: `cargo run -p vtt-app -- demo`.
//!
//! Сценарий повторяет реальную сессию: ГМ создаёт комнату, подключает пак
//! монстров, ставит гоблина из компендиума, игрок подключается и двигает свой
//! токен (но не чужой), бросает кубик, получает урон под концентрацией,
//! умирает и бросает спасброски от смерти, а в конце — сохранение, загрузка и
//! возврат в игру по `resume`.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{ensure, Result};
use vtt_core::content::ContentPack;
use vtt_core::dice::RollMode;
use vtt_core::geom::Vec2;
use vtt_core::ids::EntityId;
use vtt_core::rng::Rng;
use vtt_core::scp::{AccessLevel, ScpAttr, ScpClass, ScpSkill};
use vtt_proto::{
    CharacterPatch, ClientMsg, DeathOutcome, Event, Intent, ServerMsg, PROTOCOL_VERSION,
};
use vtt_server::{load_campaign, save_campaign, Campaign, GameServer, Outgoing, HOST_CLIENT};

const PLAYER_CLIENT: u64 = 10;
const RETURNING_CLIENT: u64 = 11;

/// Прогнать демонстрационный сценарий.
pub fn run() -> Result<()> {
    println!("=== Демо: полный цикл без сети ===\n");

    // 1. Комната и пак контента.
    let mut campaign = Campaign::new("Демо-кампания");
    let pack: ContentPack =
        serde_json::from_str(include_str!("../../../content/example-bestiary.json"))
            .expect("встроенный пак должен быть корректным JSON");
    let stats = campaign.content.add_pack(pack);
    ensure!(
        stats.monsters >= 3,
        "в паке должно быть хотя бы три монстра"
    );

    let mut server = GameServer::with_room_code(campaign, "DEMO42");
    // Фиксируем зерно: демо должно быть воспроизводимым.
    server.rng = Rng::new(2024);
    let host = server.add_host("ГМ");
    println!(
        "1. Комната {} создана. ГМ: {}. Пак: +{} записей (монстров {}).\n",
        server.room_code(),
        host.role.name_ru(),
        stats.total(),
        server.campaign.content.monsters.len()
    );

    // 2. Гоблин из компендиума.
    println!("2. ГМ ставит гоблина из компендиума:");
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SpawnMonster {
            name: "Гоблин".into(),
            pos: Vec2::new(140.0, 70.0),
        }),
    );
    print_outgoing(&out);
    let goblin = created_token(&out).expect("гоблин создан");
    let goblin_hp = server
        .campaign
        .scene()
        .token(goblin)
        .and_then(|t| t.hp)
        .expect("у гоблина есть хиты");
    ensure!(goblin_hp.max == 7, "у гоблина должно быть 7 хитов");
    println!(
        "   гоблин {goblin}, HP {}/{}\n",
        goblin_hp.value, goblin_hp.max
    );

    // 3. Игрок подключается.
    println!("3. Игрок подключается:");
    let out = server.handle(
        PLAYER_CLIENT,
        ClientMsg::Hello {
            name: "Алиса".into(),
            code: "DEMO42".into(),
            protocol: PROTOCOL_VERSION,
            resume: None,
        },
    );
    print_outgoing(&out);
    let player = server
        .session(PLAYER_CLIENT)
        .map(|s| s.player)
        .expect("игрок подключён");
    println!("   игрок #{}\n", player.0);

    // 4. Токен игрока и владение.
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::CreateToken {
            name: "Алиса".into(),
            pos: Vec2::new(105.0, 35.0),
            size: vtt_core::entities::TokenSize::Medium,
        }),
    );
    let alice = created_token(&out).expect("токен Алисы создан");
    server
        .campaign
        .scene_mut()
        .token_mut(alice)
        .expect("токен на месте")
        .owner = Some(player);
    println!("4. Токен {alice} принадлежит игроку #{}.\n", player.0);

    // 5. Игрок двигает свой токен.
    println!("5. Игрок двигает свой токен:");
    let target = Vec2::new(140.0, 70.0);
    let out = server.handle(
        PLAYER_CLIENT,
        ClientMsg::Intent(Intent::MoveToken {
            token: alice,
            to: target,
        }),
    );
    print_outgoing(&out);
    let snapped = server.campaign.scene().grid.snap(target);
    let pos = server.campaign.scene().token(alice).expect("токен").pos;
    ensure!(pos == snapped, "токен должен встать на сетку");
    println!("   позиция после сетки: ({:.0}, {:.0})\n", pos.x, pos.y);

    // 6. Чужой токен двигать нельзя.
    println!("6. Игрок пытается двигать чужой токен:");
    let out = server.handle(
        PLAYER_CLIENT,
        ClientMsg::Intent(Intent::MoveToken {
            token: goblin,
            to: Vec2::new(300.0, 300.0),
        }),
    );
    print_outgoing(&out);
    ensure!(
        out.iter()
            .any(|o| matches!(o.msg, ServerMsg::Denied { .. })),
        "сервер обязан отказать в перемещении чужого токена"
    );
    println!();

    // 7. Броски и чат.
    println!("7. Броски, чат и шёпот:");
    for intent in [
        Intent::Roll {
            expr: "1d20+5".into(),
            mode: RollMode::Advantage,
            to_gm_only: false,
        },
        Intent::Chat {
            text: "Я атакую гоблина!".into(),
            to_gm_only: false,
        },
        Intent::Chat {
            text: "у гоблина 7 хитов".into(),
            to_gm_only: true,
        },
    ] {
        let out = server.handle(PLAYER_CLIENT, ClientMsg::Intent(intent));
        print_outgoing(&out);
    }
    println!();

    // 8. Концентрация: урон требует спасброска.
    println!("8. Концентрация под уроном:");
    server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetConcentration {
            token: goblin,
            effect: Some("Проклятие".into()),
        }),
    );
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::DamageToken {
            token: goblin,
            amount: 5,
        }),
    );
    print_outgoing(&out);
    let list = events(&out);
    let check = list.iter().find_map(|event| match event {
        Event::ConcentrationCheck { dc, effect, .. } => Some((*dc, effect.clone())),
        _ => None,
    });
    let (dc, effect) = check.expect("должен быть спасбросок на концентрацию");
    ensure!(dc >= 10, "СЛ концентрации не может быть меньше 10");
    println!("   (событие выше — спасбросок на концентрацию «{effect}»)\n");

    // 9. Инициатива и раунд.
    println!("9. Инициатива:");
    server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetInitiative {
            token: goblin,
            value: 8,
        }),
    );
    server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetInitiative {
            token: alice,
            value: 15,
        }),
    );
    let out = server.handle(HOST_CLIENT, ClientMsg::Intent(Intent::NextTurn));
    print_outgoing(&out);
    println!();

    // 10. Смерть: добиваем гоблина до 0 и бросаем спасброски.
    println!("10. Смерть и спасброски:");
    server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::DamageToken {
            token: goblin,
            amount: 2,
        }),
    );
    let hp = server
        .campaign
        .scene()
        .token(goblin)
        .and_then(|t| t.hp)
        .expect("хиты");
    ensure!(hp.value == 0, "гоблин должен быть при смерти");

    let mut final_outcome = DeathOutcome::Continue;
    for attempt in 1..=8 {
        let out = server.handle(
            HOST_CLIENT,
            ClientMsg::Intent(Intent::DeathSave { token: goblin }),
        );
        print_outgoing(&out);
        let outcome = events(&out).into_iter().find_map(|event| match event {
            Event::DeathSaveResult { outcome, .. } => Some(outcome),
            _ => None,
        });
        if let Some(outcome) = outcome {
            final_outcome = outcome;
            if outcome != DeathOutcome::Continue {
                println!("   спасброски завершены на попытке {attempt}: {outcome:?}\n");
                break;
            }
        }
    }
    ensure!(
        matches!(
            final_outcome,
            DeathOutcome::Dead | DeathOutcome::Stabilized | DeathOutcome::Revived
        ),
        "спасброски должны прийти к исходу, а не длиться вечно"
    );

    // 11. Мёртвому отдых не помогает (или лечит, если ожил).
    println!("11. Отдых:");
    let goblin_dead = server
        .campaign
        .scene()
        .token(goblin)
        .is_some_and(|t| t.dead);
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::Rest {
            token: goblin,
            long: true,
        }),
    );
    print_outgoing(&out);
    if goblin_dead {
        ensure!(
            out.iter()
                .any(|o| matches!(o.msg, ServerMsg::Denied { .. })),
            "мёртвому отдых не должен помогать"
        );
    }
    println!();

    // 12. Хендаут игрокам.
    println!("12. Хендаут:");
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::PostHandout {
            title: "Письмо старосты".into(),
            text: "В подвале мельницы что-то живёт. Помогите, и мы заплатим.".into(),
        }),
    );
    print_outgoing(&out);
    ensure!(
        server.campaign.handouts.len() == 1,
        "хендаут должен попасть в кампанию"
    );
    println!();

    // 13. Туман: закрасили и убрали.
    println!("13. Туман:");
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::AddFog {
            center: Vec2::new(140.0, 105.0),
            radius: 80.0,
        }),
    );
    print_outgoing(&out);
    ensure!(
        server.campaign.scene().fog.shapes.len() == 1,
        "туман должен добавиться"
    );
    let out = server.handle(HOST_CLIENT, ClientMsg::Intent(Intent::ClearFog));
    print_outgoing(&out);
    ensure!(
        server.campaign.scene().fog.shapes.is_empty(),
        "туман должен очищаться"
    );
    println!();

    // 14. Скрытый токен не попадает игроку.
    server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetTokenHidden {
            token: goblin,
            hidden: true,
        }),
    );
    let player_snapshot = server.snapshot(PLAYER_CLIENT).expect("снапшот игрока");
    let gm_snapshot = server.snapshot(HOST_CLIENT).expect("снапшот ГМа");
    ensure!(
        player_snapshot.scene.tokens.len() == 1,
        "игрок не должен видеть скрытый токен"
    );
    ensure!(
        gm_snapshot.scene.tokens.len() == 2,
        "ГМ должен видеть все токены"
    );
    println!(
        "14. Скрытый токен: игрок видит {}, ГМ — {}.\n",
        player_snapshot.scene.tokens.len(),
        gm_snapshot.scene.tokens.len()
    );

    // 15. Сохранение и загрузка.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/demo-campaign");
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let path = save_campaign(&server.campaign, &dir)?;
    let loaded = load_campaign(&dir)?;
    ensure!(
        loaded == server.campaign,
        "кампания после загрузки отличается"
    );
    let shown = path.canonicalize().map_or_else(
        |_| path.display().to_string(),
        |full| {
            full.to_string_lossy()
                .trim_start_matches(r"\\?\")
                .to_string()
        },
    );
    println!("15. Кампания сохранена и загружена: {shown}\n");

    // 16. Возврат в игру по resume.
    let departed = server.leave(PLAYER_CLIENT).expect("игрок был в комнате");
    let connected_before = server
        .players()
        .iter()
        .find(|p| p.player == departed)
        .is_some_and(|p| p.connected);
    ensure!(
        !connected_before,
        "после выхода участник должен быть отключён"
    );

    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Hello {
            name: "Алиса".into(),
            code: "DEMO42".into(),
            protocol: PROTOCOL_VERSION,
            resume: Some(departed),
        },
    );
    print_outgoing(&out);
    let resumed = server.session(RETURNING_CLIENT).map(|s| s.player);
    ensure!(
        resumed == Some(departed),
        "resume должен вернуть тот же PlayerId"
    );
    ensure!(
        server.campaign.scene().token(alice).and_then(|t| t.owner) == Some(departed),
        "владение токеном должно сохраниться"
    );
    println!("   вернулись как игрок #{}\n", departed.0);

    // 17. Неверный код комнаты.
    let out = server.handle(
        999,
        ClientMsg::Hello {
            name: "Самозванец".into(),
            code: "AAAAAA".into(),
            protocol: PROTOCOL_VERSION,
            resume: None,
        },
    );
    print_outgoing(&out);
    ensure!(
        out.iter()
            .any(|o| matches!(o.msg, ServerMsg::Rejected { .. })),
        "сервер обязан отклонить неверный код"
    );

    // 18. SCP-лист персонажа.
    println!("18. SCP-лист персонажа:");
    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::CreateCharacter {
            name: "Агент Смит".into(),
            class: ScpClass::Agent,
            organization: "Фонд SCP".into(),
            access: AccessLevel::Second,
        }),
    );
    print_outgoing(&out);
    let character = server
        .campaign
        .characters
        .iter()
        .find(|ch| ch.name == "Агент Смит")
        .map(|ch| ch.id)
        .expect("персонаж создан");

    let mut attributes = server
        .campaign
        .character(character)
        .expect("персонаж на месте")
        .attributes;
    for (attr, value) in [
        (ScpAttr::Strength, 6),
        (ScpAttr::Speed, 6),
        (ScpAttr::Agility, 5),
        (ScpAttr::Perception, 6),
        (ScpAttr::Psyche, 5),
        (ScpAttr::Endurance, 6),
        (ScpAttr::Charisma, 4),
        (ScpAttr::Appeal, 4),
        (ScpAttr::Tech, 3),
        (ScpAttr::Intellect, 5),
    ] {
        attributes.set(attr, value);
    }
    let mut skills = BTreeMap::new();
    skills.insert(ScpSkill::Pistols, 4);
    skills.insert(ScpSkill::Deduction, 3);
    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::UpdateCharacter {
            character,
            patch: CharacterPatch {
                attributes: Some(attributes),
                skills: Some(skills),
                ..CharacterPatch::default()
            },
        }),
    );
    print_outgoing(&out);
    let report = server
        .campaign
        .character(character)
        .expect("персонаж на месте")
        .validate_creation();
    ensure!(
        report.is_ok(),
        "лист должен быть корректным: {}",
        report.describe()
    );
    println!("   {}\n", report.describe());

    // 19. Проверка навыка считается на сервере.
    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::RollCharacterCheck {
            character,
            skill: Some(ScpSkill::Pistols),
            attribute: None,
            dc: 15,
        }),
    );
    print_outgoing(&out);
    ensure!(
        events(&out)
            .iter()
            .any(|event| matches!(event, Event::CharacterCheck { .. })),
        "сервер должен вернуть результат проверки"
    );
    println!();

    // 20. Психоз, здоровье, опыт, повышение характеристики.
    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::RollSanity { character }),
    );
    print_outgoing(&out);

    server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::UpdateCharacter {
            character,
            patch: CharacterPatch {
                physical: Some(10),
                mental: Some(20),
                ..CharacterPatch::default()
            },
        }),
    );
    ensure!(
        server
            .campaign
            .character(character)
            .is_some_and(|ch| ch.physical.current == 10 && ch.mental.current == 20),
        "здоровье должно обновиться"
    );

    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::GrantXp {
            character,
            amount: 100,
        }),
    );
    print_outgoing(&out);
    ensure!(
        server
            .campaign
            .character(character)
            .is_some_and(|ch| ch.xp > 100),
        "опыт начисляется с бонусом от ИНТ"
    );

    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::LevelUpAttribute {
            character,
            attribute: ScpAttr::Strength,
        }),
    );
    print_outgoing(&out);
    ensure!(
        server
            .campaign
            .character(character)
            .is_some_and(|ch| ch.attributes.strength == 7),
        "характеристика должна вырасти"
    );

    let out = server.handle(
        RETURNING_CLIENT,
        ClientMsg::Intent(Intent::GrantXp {
            character,
            amount: 50,
        }),
    );
    ensure!(
        out.iter()
            .any(|o| matches!(o.msg, ServerMsg::Denied { .. })),
        "опыт выдаёт только ГМ"
    );
    println!();

    // 21. Токен персонажа, стены и линия видимости.
    println!("21. Стены и видимость:");
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SpawnCharacterToken {
            character,
            pos: Vec2::new(70.0, 70.0),
        }),
    );
    print_outgoing(&out);
    let character_token = created_token(&out).expect("токен персонажа создан");
    ensure!(
        server
            .campaign
            .scene()
            .token(character_token)
            .and_then(|token| token.character)
            == Some(character),
        "токен должен ссылаться на лист"
    );

    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::AddWall {
            a: Vec2::new(210.0, 0.0),
            b: Vec2::new(210.0, 420.0),
            door: false,
        }),
    );
    print_outgoing(&out);
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::AddWall {
            a: Vec2::new(210.0, 420.0),
            b: Vec2::new(210.0, 840.0),
            door: true,
        }),
    );
    print_outgoing(&out);

    let visible_cells = {
        let scene = server.campaign.scene();
        ensure!(
            !vtt_core::visibility::can_see(
                scene,
                Vec2::new(70.0, 70.0),
                Vec2::new(400.0, 70.0),
                5000.0
            ),
            "стена должна блокировать взгляд"
        );
        let cells = vtt_core::visibility::visible_cells(scene, Vec2::new(70.0, 70.0), 500.0);
        ensure!(!cells.is_empty(), "должны быть видимые клетки");
        cells.len()
    };

    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::ToggleDoor { index: 1 }),
    );
    print_outgoing(&out);

    println!(
        "   стен: {}, видимых клеток из (70,70): {visible_cells}\n",
        server.campaign.scene().walls.len()
    );

    // 22. Размер, рамка, картинка токена и границы карты.
    println!("22. Токен: размер, рамка, картинка; границы карты:");
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetTokenSize {
            token: character_token,
            size: vtt_core::entities::TokenSize::Large,
        }),
    );
    print_outgoing(&out);
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetTokenFrame {
            token: character_token,
            frame: vtt_core::entities::TokenFrame::Double,
            color: [230, 90, 90],
        }),
    );
    print_outgoing(&out);
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetTokenImage {
            token: character_token,
            image: Some("tokens/agent.png".into()),
        }),
    );
    print_outgoing(&out);
    let out = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetSceneBounds {
            width_px: 2800.0,
            height_px: 2100.0,
        }),
    );
    print_outgoing(&out);
    ensure!(
        server.campaign.scene().width_px == 2800.0,
        "границы карты должны измениться"
    );
    ensure!(
        server
            .campaign
            .scene()
            .token(character_token)
            .is_some_and(|token| token.size == vtt_core::entities::TokenSize::Large),
        "размер токена должен измениться"
    );
    println!();

    let final_path = save_campaign(&server.campaign, &dir)?;
    println!("   финальное состояние сохранено: {}", final_path.display());

    println!("\n=== Демо завершено успешно ===");
    println!("Проверено: комнаты, пак монстров, права, сетка, броски, шёпот,");
    println!("концентрация, урон, смерть, отдых, хендаут, туман, скрытые токены,");
    println!("сохранение и возврат в игру.");
    println!("Плюс SCP-лист: характеристики, навыки, проверки, психоз, опыт,");
    println!("токен персонажа, стены и линия видимости.");
    Ok(())
}

fn events(outgoing: &[Outgoing]) -> Vec<Event> {
    outgoing
        .iter()
        .filter_map(|item| match &item.msg {
            ServerMsg::Events(list) => Some(list.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn created_token(outgoing: &[Outgoing]) -> Option<EntityId> {
    events(outgoing).into_iter().find_map(|event| match event {
        Event::TokenCreated { token } => Some(token.id),
        _ => None,
    })
}

fn print_outgoing(outgoing: &[Outgoing]) {
    let mut last_printed: Option<String> = None;
    for item in outgoing {
        match &item.msg {
            ServerMsg::Events(events) => {
                for event in events {
                    let line = describe(event);
                    // Шёпот уходит и автору, и ГМу: не печатаем одну строку дважды.
                    if last_printed.as_deref() != Some(line.as_str()) {
                        println!("   {line}");
                        last_printed = Some(line);
                    }
                }
            }
            ServerMsg::Denied { reason } => println!("   ✖ отказано: {reason}"),
            ServerMsg::Welcome(welcome) => println!(
                "   ✔ добро пожаловать, игрок #{} ({})",
                welcome.player.0,
                welcome.role.name_ru()
            ),
            ServerMsg::Snapshot(snapshot) => println!(
                "   снапшот: сцена «{}», токенов {}",
                snapshot.scene.name,
                snapshot.scene.tokens.len()
            ),
            ServerMsg::Rejected { reason } => println!("   ✖ отклонено: {reason}"),
            ServerMsg::Notice { text } => println!("   · {text}"),
            ServerMsg::Pong { .. } => {}
        }
    }
}

fn describe(event: &Event) -> String {
    match event {
        Event::TokenMoved { token, to } => {
            format!("{token} перемещён в ({:.0}, {:.0})", to.x, to.y)
        }
        Event::TokenCreated { token } => format!(
            "создан токен {} «{}» в ({:.0}, {:.0})",
            token.id, token.name, token.pos.x, token.pos.y
        ),
        Event::TokenRemoved { token } => format!("удалён токен {token}"),
        Event::TokenChanged { token } => {
            let hp = token
                .hp
                .map(|bar| format!(", HP {}/{}", bar.value, bar.max))
                .unwrap_or_default();
            let dead = if token.dead { ", мёртв" } else { "" };
            format!("токен {} «{}»{hp}{dead}", token.id, token.name)
        }
        Event::ChatPosted {
            from_name,
            text,
            to_gm_only,
            ..
        } => format!(
            "💬 {from_name}{}: {text}",
            if *to_gm_only {
                " [только ГМу]"
            } else {
                ""
            }
        ),
        Event::Rolled {
            from_name,
            result,
            to_gm_only,
            ..
        } => format!(
            "🎲 {from_name}{}: {}",
            if *to_gm_only {
                " [только ГМу]"
            } else {
                ""
            },
            result.describe()
        ),
        Event::InitiativeSet { name, value, .. } => format!("инициатива «{name}»: {value}"),
        Event::TurnChanged {
            round, active_name, ..
        } => match active_name {
            Some(name) => format!("раунд {round}, ход: {name}"),
            None => format!("раунд {round}, участников нет"),
        },
        Event::SceneChanged { name, .. } => format!("сцена переключена на «{name}»"),
        Event::ConcentrationCheck { dc, effect, .. } => {
            format!("концентрация «{effect}»: спасбросок СЛ {dc}")
        }
        Event::DeathSaveResult {
            roll,
            successes,
            failures,
            outcome,
            ..
        } => format!(
            "спасбросок от смерти {}: успехи {successes}, провалы {failures}, итог {outcome:?}",
            roll.total
        ),
        Event::HandoutPosted { title, .. } => format!("хендаут «{title}» показан игрокам"),
        Event::FogChanged { shapes } => format!("туман: областей {}", shapes.len()),
        Event::CharacterUpdated { character } => format!("лист: {}", character.summary()),
        Event::CharacterCheck {
            name,
            skill,
            result,
            ..
        } => {
            let what = skill.map_or("проверка", |skill| skill.name_ru());
            format!("{name}: {}", result.describe(what))
        }
        Event::SanityRolled { name, outcome, .. } => format!("{name}: {}", outcome.describe()),
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
