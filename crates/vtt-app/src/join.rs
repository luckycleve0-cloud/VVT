//! Клиент игрока: подключение к комнате ГМа по TCP.

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use vtt_net::TcpClient;
use vtt_proto::{ClientMsg, PlayerId, ServerMsg, PROTOCOL_VERSION};

use crate::common::{self, Command};

/// Подключиться к комнате и работать в консоли до `quit`.
///
/// `resume` — идентификатор прошлой сессии (`--resume`), чтобы вернуться
/// с тем же персонажем и владением токенами.
pub fn run(addr: String, name: String, code: Option<String>, resume: Option<u64>) -> Result<()> {
    let code = match code {
        Some(code) => code,
        None => {
            let answer = common::prompt("Код комнаты: ")?;
            if answer.is_empty() {
                anyhow::bail!("без кода комнаты подключиться нельзя");
            }
            answer
        }
    };

    let client = TcpClient::connect(addr.as_str())
        .with_context(|| format!("не удалось подключиться к {addr}"))?;
    client.send(&ClientMsg::Hello {
        name,
        code,
        protocol: PROTOCOL_VERSION,
        resume: resume.map(PlayerId::new),
    })?;
    println!("подключаемся к {}…", client.peer());

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

    let mut running = true;
    while running {
        for msg in client.poll() {
            common::print_server_msg(&msg);
            if matches!(msg, ServerMsg::Rejected { .. }) {
                running = false;
            }
        }

        while let Ok(line) = line_rx.try_recv() {
            match common::parse(&line) {
                Ok(Command::Help) => common::print_help(),
                Ok(Command::Quit) => {
                    let _ = client.send(&ClientMsg::Bye);
                    running = false;
                }
                Ok(Command::Save) => println!("кампанию сохраняет только ГМ"),
                Ok(Command::Tokens) => println!("список доступен в снапшоте при подключении"),
                Ok(Command::Who | Command::Monsters | Command::Search(_)) => {
                    println!("эта команда доступна только ГМу")
                }
                Ok(command) => {
                    if let Some(intent) = common::to_intent(command) {
                        if let Err(err) = client.send(&ClientMsg::Intent(intent)) {
                            println!("не отправлено: {err}");
                            running = false;
                        }
                    }
                }
                Err(err) => println!("? {err}"),
            }
        }

        if running && !client.is_alive() {
            println!("соединение с сервером закрыто");
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
