//! # vtt — консольное приложение ядра VTT
//!
//! ```text
//! vtt demo                                    # прогнать полный сценарий без сети
//! vtt host --port 7878 --pack content/x.json  # создать комнату (ГМ = сервер)
//! vtt join 192.168.1.10:7878 --code ABC123    # подключиться к комнате
//! vtt info --dir ./campaign                   # посмотреть кампанию
//! ```
//!
//! Окно с картой — отдельный бинарник `vtt-play` (крейт `vtt-render`).

#![forbid(unsafe_code)]

mod common;
mod demo;
mod host;
mod join;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "vtt", version, about = "Единый VTT: ядро (консольная версия)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Прогнать полный сценарий без сети (проверка ядра).
    Demo,
    /// Создать комнату на этом компьютере и стать ГМом.
    Host {
        /// TCP-порт для игроков.
        #[arg(short, long, default_value_t = 7878)]
        port: u16,
        /// Каталог кампании.
        #[arg(short, long, default_value = "./campaign")]
        dir: PathBuf,
        /// Название новой кампании.
        #[arg(long, default_value = "Моя кампания")]
        name: String,
        /// Код комнаты (по умолчанию случайный).
        #[arg(long)]
        code: Option<String>,
        /// Пак контента (JSON). Можно указать несколько раз.
        #[arg(long = "pack", value_name = "FILE")]
        packs: Vec<PathBuf>,
    },
    /// Подключиться к комнате ГМа.
    Join {
        /// Адрес, например 192.168.1.10:7878.
        addr: String,
        /// Имя игрока.
        #[arg(long, default_value = "Игрок")]
        name: String,
        /// Код комнаты (если не указан — спросим).
        #[arg(long)]
        code: Option<String>,
        /// Вернуться в прошлую сессию с этим номером игрока.
        #[arg(long)]
        resume: Option<u64>,
    },
    /// Показать сохранённую кампанию.
    Info {
        #[arg(short, long, default_value = "./campaign")]
        dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Demo => demo::run(),
        Command::Host {
            port,
            dir,
            name,
            code,
            packs,
        } => host::run(port, dir, name, code, packs),
        Command::Join {
            addr,
            name,
            code,
            resume,
        } => join::run(addr, name, code, resume),
        Command::Info { dir } => {
            let campaign = vtt_server::load_campaign(&dir)?;
            println!("Кампания: «{}»", campaign.name);
            println!("Сцен: {}", campaign.scenes.len());
            for scene in &campaign.scenes {
                println!(
                    "  {} «{}»: токенов {}, стен {}, туман {:?}",
                    scene.id,
                    scene.name,
                    scene.tokens.len(),
                    scene.walls.len(),
                    scene.fog.mode
                );
            }
            println!("Персонажей: {}", campaign.actors.len());
            println!("Компендиум: {}", campaign.content.len());
            println!("Хендаутов: {}", campaign.handouts.len());
            Ok(())
        }
    }
}
