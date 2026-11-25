//! Интеграционный тест: настоящий TCP, настоящий сервер, настоящий клиент.
//!
//! Проверяем ровно то, что обещано продуктом: «ГМ = сервер, игроки
//! подключаются по адресу и видят состояние».

use std::time::{Duration, Instant};

use vtt_core::entities::Token;
use vtt_core::geom::Vec2;
use vtt_core::permissions::Role;
use vtt_net::{TcpClient, TcpHub};
use vtt_proto::{ClientMsg, Event, Intent, ServerMsg, Snapshot, PROTOCOL_VERSION};
use vtt_server::{Campaign, GameServer, Outgoing, HOST_CLIENT};

const CODE: &str = "CODE12";

fn test_room() -> (TcpHub, GameServer, u64) {
    let mut campaign = Campaign::new("Сетевая кампания");
    let token_id = campaign.next_id();
    campaign
        .scene_mut()
        .add_token(Token::new(token_id, "Гоблин", Vec2::new(35.0, 35.0)));

    let mut server = GameServer::with_room_code(campaign, CODE);
    server.add_host("ГМ");
    let hub = TcpHub::bind("127.0.0.1:0").expect("сервер должен занять порт");
    (hub, server, token_id.0)
}

/// Разослать исходящие сообщения сервера (как это делает `host.rs`).
fn dispatch(hub: &TcpHub, outgoing: Vec<Outgoing>) {
    for item in outgoing {
        match item.to {
            Some(HOST_CLIENT) => {}
            Some(client) => {
                let _ = hub.send_to(client, &item.msg);
            }
            None => {
                let _ = hub.broadcast(&item.msg);
            }
        }
    }
}

fn pump(hub: &TcpHub, server: &mut GameServer) {
    for (client, msg) in hub.poll() {
        let outgoing = server.handle(client, msg);
        dispatch(hub, outgoing);
    }
}

/// Собрать сообщения клиента за отведённое время, параллельно «крутя» сервер.
fn collect(
    client: &TcpClient,
    hub: &TcpHub,
    server: &mut GameServer,
    timeout: Duration,
) -> Vec<ServerMsg> {
    let deadline = Instant::now() + timeout;
    let mut collected = Vec::new();
    while Instant::now() < deadline {
        pump(hub, server);
        collected.extend(client.poll());
        if collected
            .iter()
            .any(|m| matches!(m, ServerMsg::Snapshot(_)))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    collected
}

fn join(client: &TcpClient, name: &str) {
    client
        .send(&ClientMsg::Hello {
            name: name.to_string(),
            code: CODE.to_string(),
            protocol: PROTOCOL_VERSION,
            resume: None,
        })
        .expect("Hello должен уйти");
}

fn snapshot_from(messages: &[ServerMsg]) -> Option<Snapshot> {
    messages.iter().find_map(|msg| match msg {
        ServerMsg::Snapshot(snapshot) => Some((**snapshot).clone()),
        _ => None,
    })
}

#[test]
fn player_connects_over_tcp_and_gets_snapshot() {
    let (hub, mut server, _token) = test_room();
    let client = TcpClient::connect(hub.local_addr()).expect("подключение");
    join(&client, "Алиса");

    let messages = collect(&client, &hub, &mut server, Duration::from_secs(5));

    assert!(
        messages.iter().any(|m| matches!(m, ServerMsg::Welcome(_))),
        "не пришло Welcome: {messages:?}"
    );
    let snapshot = snapshot_from(&messages).expect("не пришёл снапшот");
    assert_eq!(snapshot.campaign, "Сетевая кампания");
    assert_eq!(snapshot.scene.tokens.len(), 1);
    assert_eq!(snapshot.you.role, Role::Player);
}

#[test]
fn player_moves_own_token_over_tcp() {
    let (hub, mut server, token) = test_room();
    let client = TcpClient::connect(hub.local_addr()).expect("подключение");
    join(&client, "Алиса");

    let messages = collect(&client, &hub, &mut server, Duration::from_secs(5));
    let snapshot = snapshot_from(&messages).expect("снапшот");
    let player = snapshot.you.player;

    // ГМ отдаёт токен игроку (в будущем это будет интент).
    server
        .campaign
        .scene_mut()
        .token_mut(vtt_core::ids::EntityId(token))
        .expect("токен на месте")
        .owner = Some(player);

    client
        .send(&ClientMsg::Intent(Intent::MoveToken {
            token: vtt_core::ids::EntityId(token),
            to: Vec2::new(105.0, 105.0),
        }))
        .expect("намерение должно уйти");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut moved = false;
    while Instant::now() < deadline && !moved {
        pump(&hub, &mut server);
        for msg in client.poll() {
            if let ServerMsg::Events(events) = msg {
                moved = events.iter().any(|e| matches!(e, Event::TokenMoved { .. }));
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    assert!(moved, "игрок не получил событие перемещения");
    let pos = server
        .campaign
        .scene()
        .token(vtt_core::ids::EntityId(token))
        .expect("токен на месте")
        .pos;
    assert_eq!(pos, Vec2::new(105.0, 105.0));
}

#[test]
fn hidden_token_never_reaches_player_over_network() {
    let (hub, mut server, token) = test_room();

    // ГМ скрывает токен до подключения игрока.
    let outgoing = server.handle(
        HOST_CLIENT,
        ClientMsg::Intent(Intent::SetTokenHidden {
            token: vtt_core::ids::EntityId(token),
            hidden: true,
        }),
    );
    dispatch(&hub, outgoing);

    let client = TcpClient::connect(hub.local_addr()).expect("подключение");
    join(&client, "Алиса");
    let messages = collect(&client, &hub, &mut server, Duration::from_secs(5));

    let snapshot = snapshot_from(&messages).expect("снапшот");
    assert_eq!(
        snapshot.scene.tokens.len(),
        0,
        "скрытый токен не должен даже приезжать клиенту"
    );
}

#[test]
fn wrong_room_code_is_rejected_over_tcp() {
    let (hub, mut server, _token) = test_room();
    let client = TcpClient::connect(hub.local_addr()).expect("подключение");
    client
        .send(&ClientMsg::Hello {
            name: "Самозванец".into(),
            code: "NOPE00".into(),
            protocol: PROTOCOL_VERSION,
            resume: None,
        })
        .expect("Hello должен уйти");

    let messages = collect(&client, &hub, &mut server, Duration::from_secs(5));
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, ServerMsg::Rejected { .. })),
        "неверный код должен отклоняться: {messages:?}"
    );
}
