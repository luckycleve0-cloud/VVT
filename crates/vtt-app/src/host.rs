//! Хост комнаты: сервер + консоль ГМа.

use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use vtt_net::TcpHub;
use vtt_proto::{ClientMsg, ServerMsg};
use vtt_server::{
    campaign_exists, load_campaign, load_pack_file, save_campaign, Campaign, GameServer, Outgoing,
    HOST_CLIENT,
};

use crate::common::{self, Command};

/// Запустить комнату. Блокируется до команды `quit` или закрытия stdin.
pub fn run(
    port: u16,
    dir: PathBuf,
    name: String,
    code: Option<String>,
    packs: Vec<PathBuf>,
) -> Result<()> {
    let mut campaign = if campaign_exists(&dir) {
        load_campaign(&dir).with_context(|| format!("не удалось загрузить {}", dir.display()))?
    } else {
        Campaign::new(name)
    };

    for pack_path in &packs {
        let pack = load_pack_file(pack_path)
            .with_context(|| format!("не удалось прочитать пак {}", pack_path.display()))?;
        let stats = campaign.content.add_pack(pack);
        println!("пак {}: +{} записей", pack_path.display(), stats.total());
    }

    let mut server = match code {
        Some(code) => GameServer::with_room_code(campaign, code),
        None => GameServer::new(campaign),
    };
    let hub = TcpHub::bind(("0.0.0.0", port)).context("не удалось занять порт")?;
    let welcome = server.add_host("ГМ");

    println!("=== Комната создана ===");
    println!("Код:      {}", server.room_code());
    if let Some(ip) = common::local_ip() {
        println!("Адрес:    {ip}:{}", hub.local_addr().port());
    }
    println!("Порт:     {} (TCP)", hub.local_addr().port());
    println!("Кампания: «{}»", welcome.campaign);
    if !server.campaign.content.is_empty() {
        println!(
            "Компендиум: {} записей, монстров {}",
            server.campaign.content.len(),
            server.campaign.content.monsters.len()
        );
    }
    println!();
    println!("Игрокам:  vtt join <адрес> --code {}", server.room_code());
    println!("Команды:  help\n");

    let (line_tx, line_rx) = mpsc::channel::<String>();
    thread::spawn(move || {
        for line in io::stdin().lines() {
            match line {
                Ok(text) => {
                    if line_tx.send(text).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut log: Vec<String> = Vec::new();
    let mut running = true;
    while running {
        // 1. Сеть: кто-то подключился или что-то прислал.
        for (client, msg) in hub.poll() {
            let outgoing = server.handle(client, msg);
            dispatch(&hub, outgoing, &mut log);
        }

        // 2. Консоль ГМа.
        while let Ok(line) = line_rx.try_recv() {
            match common::parse(&line) {
                Ok(Command::Help) => common::print_help(),
                Ok(Command::Quit) => running = false,
                Ok(Command::Save) => {
                    let path = save_campaign(&server.campaign, &dir)?;
                    write_log(&dir, &log);
                    println!("сохранено: {}", path.display());
                }
                Ok(Command::Tokens) => print_tokens(&server),
                Ok(Command::Who) => print_who(&server),
                Ok(Command::Monsters) => print_monsters(&server),
                Ok(Command::Search(query)) => print_search(&server, &query),
                Ok(command) => {
                    if let Some(intent) = common::to_intent(command) {
                        let outgoing = server.handle(HOST_CLIENT, ClientMsg::Intent(intent));
                        dispatch(&hub, outgoing, &mut log);
                    }
                }
                Err(err) => println!("? {err}"),
            }
        }

        thread::sleep(Duration::from_millis(20));
    }

    let path = save_campaign(&server.campaign, &dir)?;
    write_log(&dir, &log);
    println!("сохранено: {}", path.display());
    Ok(())
}

/// Записать журнал сессии рядом с кампанией.
fn write_log(dir: &std::path::Path, log: &[String]) {
    let path = dir.join("session.log");
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(path, log.join("\n"));
}

/// Разослать исходящие сообщения.
///
/// Локальный ГМ (`HOST_CLIENT`) не ходит по сети — его сообщения печатаем сразу.
fn dispatch(hub: &TcpHub, outgoing: Vec<Outgoing>, log: &mut Vec<String>) {
    for out in outgoing {
        match out.to {
            Some(HOST_CLIENT) => print_local(&out.msg, log),
            Some(client) => {
                if let Err(err) = hub.send_to(client, &out.msg) {
                    eprintln!("клиенту {client}: {err}");
                }
            }
            None => {
                if let Err(err) = hub.broadcast(&out.msg) {
                    eprintln!("рассылка: {err}");
                }
                print_local(&out.msg, log);
            }
        }
    }
}

fn print_local(msg: &ServerMsg, log: &mut Vec<String>) {
    match msg {
        ServerMsg::Events(events) => {
            for event in events {
                let line = common::describe_event(event);
                println!("{line}");
                log.push(line);
            }
        }
        ServerMsg::Notice { text } => {
            println!("· {text}");
            log.push(text.clone());
        }
        _ => {}
    }
}

fn print_tokens(server: &GameServer) {
    let scene = server.campaign.scene();
    println!("сцена «{}» ({}):", scene.name, scene.id);
    if scene.tokens.is_empty() {
        println!("  (пусто)");
    }
    for token in &scene.tokens {
        let owner = token
            .owner
            .map(|id| format!(", владелец #{0}", id.0))
            .unwrap_or_default();
        let hidden = if token.hidden { ", скрыт" } else { "" };
        let dead = if token.dead { ", мёртв" } else { "" };
        let concentration = token
            .concentration
            .as_ref()
            .map(|effect| format!(", концентрация «{effect}»"))
            .unwrap_or_default();
        let hp = token
            .hp
            .map(|bar| format!(" HP {}/{}", bar.value, bar.max))
            .unwrap_or_default();
        println!(
            "  {} «{}» ({:.0}, {:.0}){hp}{owner}{hidden}{dead}{concentration}",
            token.id, token.name, token.pos.x, token.pos.y
        );
    }
    println!(
        "  туман: {} областей, режим {:?}",
        scene.fog.shapes.len(),
        scene.fog.mode
    );
}

fn print_who(server: &GameServer) {
    println!("участников: {}", server.client_count());
    for player in server.players() {
        let state = if player.connected {
            "на связи"
        } else {
            "отключён"
        };
        println!(
            "  #{} {} ({}, {state})",
            player.player.0,
            player.name,
            player.role.name_ru()
        );
    }
}

fn print_monsters(server: &GameServer) {
    let content = &server.campaign.content;
    if content.monsters.is_empty() {
        println!("компендиум пуст: запустите с --pack <файл>");
        return;
    }
    println!("монстры ({}):", content.monsters.len());
    for monster in &content.monsters {
        println!(
            "  {} «{}»: КД {}, HP {}, CR {}",
            monster.id,
            monster.name,
            monster.armor_class,
            monster.hit_points,
            monster.challenge_rating
        );
    }
}

fn print_search(server: &GameServer, query: &str) {
    let hits = server.campaign.content.search(query, 15);
    if hits.is_empty() {
        println!("ничего не найдено по запросу «{query}»");
        return;
    }
    for hit in hits {
        println!("  {} {} — {}", hit.id, hit.name, hit.subtitle);
    }
}
