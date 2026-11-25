//! Общее для хоста и клиента: разбор команд, печать событий, мелочи.

use std::io::{self, Write};
use std::net::IpAddr;

use vtt_core::dice::RollMode;
use vtt_core::entities::TokenSize;
use vtt_core::geom::Vec2;
use vtt_core::ids::EntityId;
use vtt_core::scp::{AccessLevel, ScpAttr, ScpClass, ScpSkill};
use vtt_proto::{Event, Intent, ServerMsg, Snapshot};

/// Команда из консоли.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Help,
    Say(String),
    GmSay(String),
    Roll(String),
    Advantage(String),
    Disadvantage(String),
    Move {
        token: u64,
        x: f32,
        y: f32,
    },
    Create {
        name: String,
        x: f32,
        y: f32,
    },
    Spawn {
        name: String,
        x: f32,
        y: f32,
    },
    Rest {
        token: u64,
        long: bool,
    },
    DeathSave {
        token: u64,
    },
    Concentration {
        token: u64,
        effect: Option<String>,
    },
    Handout {
        title: String,
        text: String,
    },
    Fog {
        x: f32,
        y: f32,
        radius: f32,
    },
    FogClear,
    Search(String),
    Monsters,
    NewChar {
        name: String,
        class: ScpClass,
    },
    Check {
        character: u64,
        skill: ScpSkill,
        dc: i32,
    },
    AttrCheck {
        character: u64,
        attr: ScpAttr,
        dc: i32,
    },
    Sanity {
        character: u64,
    },
    Xp {
        character: u64,
        amount: i32,
    },
    LevelUp {
        character: u64,
        attr: ScpAttr,
    },
    Wall {
        a: (f32, f32),
        b: (f32, f32),
        door: bool,
    },
    ToggleWall {
        index: usize,
    },
    Hp {
        token: u64,
        value: i32,
    },
    Damage {
        token: u64,
        amount: i32,
    },
    Hide {
        token: u64,
        hidden: bool,
    },
    Init {
        token: u64,
        value: i32,
    },
    NextTurn,
    Scene(u64),
    Tokens,
    Who,
    Save,
    Quit,
}

/// Разобрать строку команды.
pub fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("пустая команда (help — список)".to_string());
    }
    let (head, rest) = match line.split_once(char::is_whitespace) {
        Some((head, rest)) => (head, rest.trim_start()),
        None => (line, ""),
    };
    let args: Vec<&str> = rest.split_whitespace().collect();
    let command = head.to_lowercase();

    match command.as_str() {
        "help" | "?" | "помощь" => Ok(Command::Help),
        "quit" | "exit" | "выход" => Ok(Command::Quit),
        "save" | "сохранить" => Ok(Command::Save),
        "who" | "кто" => Ok(Command::Who),
        "tokens" | "токены" => Ok(Command::Tokens),
        "next" | "ход" => Ok(Command::NextTurn),
        "say" | "сказать" => {
            if rest.is_empty() {
                Err("нужен текст: say <текст>".to_string())
            } else {
                Ok(Command::Say(rest.to_string()))
            }
        }
        "gm" | "шёпот" => {
            if rest.is_empty() {
                Err("нужен текст: gm <текст>".to_string())
            } else {
                Ok(Command::GmSay(rest.to_string()))
            }
        }
        "roll" | "бросок" => one_arg(&args, "roll <формула>", Command::Roll),
        "adv" | "преимущество" => one_arg(&args, "adv <формула>", Command::Advantage),
        "dis" | "помеха" => one_arg(&args, "dis <формула>", Command::Disadvantage),
        "move" | "двинуть" => {
            let (token, x, y) = three_args(&args, "move <токен> <x> <y>")?;
            Ok(Command::Move { token, x, y })
        }
        "create" | "создать" => {
            require_len(&args, 3, "create <имя> <x> <y>")?;
            let x = number(args[1], "x")?;
            let y = number(args[2], "y")?;
            Ok(Command::Create {
                name: args[0].to_string(),
                x,
                y,
            })
        }
        "spawn" | "монстр" => {
            if args.len() < 3 {
                return Err("нужно: spawn <имя монстра> <x> <y>".to_string());
            }
            let count = args.len();
            let x = number(args[count - 2], "x")?;
            let y = number(args[count - 1], "y")?;
            let name = args[..count - 2].join(" ");
            Ok(Command::Spawn { name, x, y })
        }
        "rest" | "отдых" => {
            require_len(&args, 1, "rest <токен> [long|short]")?;
            let token = number(args[0], "токен")?;
            let long = args.get(1).is_some_and(|mode| {
                matches!(mode.to_lowercase().as_str(), "long" | "длинный" | "д")
            });
            Ok(Command::Rest { token, long })
        }
        "death" | "смерть" => {
            require_len(&args, 1, "death <токен>")?;
            Ok(Command::DeathSave {
                token: number(args[0], "токен")?,
            })
        }
        "conc" | "концентрация" => {
            require_len(&args, 1, "conc <токен> <эффект>")?;
            let token = number(args[0], "токен")?;
            let effect = args[1..].join(" ");
            Ok(Command::Concentration {
                token,
                effect: if effect.trim().is_empty() {
                    None
                } else {
                    Some(effect)
                },
            })
        }
        "unconc" | "снять_концентрацию" => {
            require_len(&args, 1, "unconc <токен>")?;
            Ok(Command::Concentration {
                token: number(args[0], "токен")?,
                effect: None,
            })
        }
        "handout" | "хендаут" => {
            let Some((title, text)) = rest.split_once('|') else {
                return Err("нужно: handout <заголовок> | <текст>".to_string());
            };
            if title.trim().is_empty() || text.trim().is_empty() {
                return Err("нужны и заголовок, и текст".to_string());
            }
            Ok(Command::Handout {
                title: title.trim().to_string(),
                text: text.trim().to_string(),
            })
        }
        "fog" | "туман" => {
            require_len(&args, 2, "fog <x> <y> [радиус]")?;
            let x = number(args[0], "x")?;
            let y = number(args[1], "y")?;
            let radius = if args.len() > 2 {
                number(args[2], "радиус")?
            } else {
                60.0
            };
            Ok(Command::Fog { x, y, radius })
        }
        "fogclear" | "туман_убрать" => Ok(Command::FogClear),
        "search" | "поиск" => {
            if rest.is_empty() {
                return Err("нужно: search <текст>".to_string());
            }
            Ok(Command::Search(rest.to_string()))
        }
        "monsters" | "монстры" => Ok(Command::Monsters),
        "newchar" | "новый" => {
            if args.is_empty() {
                return Err("нужно: newchar [класс] <имя>".to_string());
            }
            let (class, name_args) = match parse_scp_class(args[0]) {
                Some(class) => (class, &args[1..]),
                None => (ScpClass::Soldier, &args[..]),
            };
            if name_args.is_empty() {
                return Err("нужно имя персонажа".to_string());
            }
            Ok(Command::NewChar {
                name: name_args.join(" "),
                class,
            })
        }
        "check" | "проверка" => {
            require_len(&args, 2, "check <id> <навык> [СЛ]")?;
            let character = number(args[0], "персонаж")?;
            let mut tail = &args[1..];
            let mut dc = 15;
            if let Some(last) = tail.last() {
                if let Ok(value) = last.parse::<i32>() {
                    dc = value.clamp(1, 40);
                    tail = &tail[..tail.len() - 1];
                }
            }
            let skill_name = tail.join(" ");
            let skill = parse_scp_skill(&skill_name)
                .ok_or_else(|| format!("неизвестный навык «{skill_name}»"))?;
            Ok(Command::Check {
                character,
                skill,
                dc,
            })
        }
        "attrcheck" | "проверка_характеристики" => {
            require_len(&args, 2, "attrcheck <id> <ХАР> [СЛ]")?;
            let character = number(args[0], "персонаж")?;
            let attr = parse_scp_attr(args[1])
                .ok_or_else(|| format!("неизвестная характеристика «{}»", args[1]))?;
            let dc = if args.len() > 2 {
                number(args[2], "СЛ")?
            } else {
                15
            };
            Ok(Command::AttrCheck {
                character,
                attr,
                dc,
            })
        }
        "sanity" | "психоз" => {
            require_len(&args, 1, "sanity <id>")?;
            Ok(Command::Sanity {
                character: number(args[0], "персонаж")?,
            })
        }
        "xp" | "опыт" => {
            require_len(&args, 2, "xp <id> <количество>")?;
            Ok(Command::Xp {
                character: number(args[0], "персонаж")?,
                amount: number(args[1], "опыт")?,
            })
        }
        "levelup" | "повысить" => {
            require_len(&args, 2, "levelup <id> <ХАР>")?;
            let character = number(args[0], "персонаж")?;
            let attr = parse_scp_attr(args[1])
                .ok_or_else(|| format!("неизвестная характеристика «{}»", args[1]))?;
            Ok(Command::LevelUp { character, attr })
        }
        "wall" | "стена" => wall_command(&args, false),
        "door" | "дверь" => wall_command(&args, true),
        "toggle" | "переключить_дверь" => {
            require_len(&args, 1, "toggle <индекс стены>")?;
            Ok(Command::ToggleWall {
                index: number(args[0], "индекс")?,
            })
        }
        "hp" | "хиты" => {
            require_len(&args, 2, "hp <токен> <значение>")?;
            Ok(Command::Hp {
                token: number(args[0], "токен")?,
                value: number(args[1], "значение")?,
            })
        }
        "dmg" | "урон" => {
            require_len(&args, 2, "dmg <токен> <количество>")?;
            Ok(Command::Damage {
                token: number(args[0], "токен")?,
                amount: number(args[1], "урон")?,
            })
        }
        "hide" | "скрыть" => {
            require_len(&args, 2, "hide <токен> <true|false>")?;
            let hidden = match args[1].to_lowercase().as_str() {
                "true" | "1" | "да" => true,
                "false" | "0" | "нет" => false,
                _ => return Err("третий аргумент: true или false".to_string()),
            };
            Ok(Command::Hide {
                token: number(args[0], "токен")?,
                hidden,
            })
        }
        "init" | "инициатива" => {
            require_len(&args, 2, "init <токен> <значение>")?;
            Ok(Command::Init {
                token: number(args[0], "токен")?,
                value: number(args[1], "значение")?,
            })
        }
        "scene" | "сцена" => {
            require_len(&args, 1, "scene <id сцены>")?;
            Ok(Command::Scene(number(args[0], "сцена")?))
        }
        other => Err(format!("неизвестная команда «{other}» (help — список)")),
    }
}

fn require_len(args: &[&str], needed: usize, usage: &str) -> Result<(), String> {
    if args.len() < needed {
        Err(format!("мало аргументов, нужно: {usage}"))
    } else {
        Ok(())
    }
}

fn wall_command(args: &[&str], door: bool) -> Result<Command, String> {
    let usage = if door {
        "door <x1> <y1> <x2> <y2>"
    } else {
        "wall <x1> <y1> <x2> <y2>"
    };
    require_len(args, 4, usage)?;
    Ok(Command::Wall {
        a: (number(args[0], "x1")?, number(args[1], "y1")?),
        b: (number(args[2], "x2")?, number(args[3], "y2")?),
        door,
    })
}

/// Навык по русскому названию (точное или по началу слова).
fn parse_scp_skill(raw: &str) -> Option<ScpSkill> {
    let needle = raw.trim().to_lowercase();
    ScpSkill::ALL
        .iter()
        .copied()
        .find(|skill| skill.name_ru().to_lowercase() == needle)
        .or_else(|| {
            ScpSkill::ALL
                .iter()
                .copied()
                .find(|skill| skill.name_ru().to_lowercase().starts_with(&needle))
        })
}

/// Характеристика по «СИЛ» или «Сила».
fn parse_scp_attr(raw: &str) -> Option<ScpAttr> {
    let needle = raw.trim().to_lowercase();
    ScpAttr::ALL.iter().copied().find(|attr| {
        attr.short_ru().to_lowercase() == needle || attr.name_ru().to_lowercase() == needle
    })
}

/// Класс по русскому названию.
fn parse_scp_class(raw: &str) -> Option<ScpClass> {
    let needle = raw.trim().to_lowercase();
    ScpClass::ALL
        .iter()
        .copied()
        .find(|class| class.name_ru().to_lowercase() == needle)
}

fn one_arg(args: &[&str], usage: &str, f: impl Fn(String) -> Command) -> Result<Command, String> {
    require_len(args, 1, usage)?;
    Ok(f(args.join(" ")))
}

fn three_args(args: &[&str], usage: &str) -> Result<(u64, f32, f32), String> {
    require_len(args, 3, usage)?;
    Ok((
        number(args[0], "токен")?,
        number(args[1], "x")?,
        number(args[2], "y")?,
    ))
}

fn number<T: std::str::FromStr>(value: &str, what: &str) -> Result<T, String> {
    value
        .parse::<T>()
        .map_err(|_| format!("не число в «{what}»: {value}"))
}

/// Превратить команду в намерение. `None` — команда не про игру (help/save/…).
#[must_use]
pub fn to_intent(command: Command) -> Option<Intent> {
    match command {
        Command::Say(text) => Some(Intent::Chat {
            text,
            to_gm_only: false,
        }),
        Command::GmSay(text) => Some(Intent::Chat {
            text,
            to_gm_only: true,
        }),
        Command::Roll(expr) => Some(Intent::Roll {
            expr,
            mode: RollMode::Normal,
            to_gm_only: false,
        }),
        Command::Advantage(expr) => Some(Intent::Roll {
            expr,
            mode: RollMode::Advantage,
            to_gm_only: false,
        }),
        Command::Disadvantage(expr) => Some(Intent::Roll {
            expr,
            mode: RollMode::Disadvantage,
            to_gm_only: false,
        }),
        Command::Move { token, x, y } => Some(Intent::MoveToken {
            token: EntityId(token),
            to: Vec2::new(x, y),
        }),
        Command::Create { name, x, y } => Some(Intent::CreateToken {
            name,
            pos: Vec2::new(x, y),
            size: TokenSize::Medium,
        }),
        Command::Spawn { name, x, y } => Some(Intent::SpawnMonster {
            name,
            pos: Vec2::new(x, y),
        }),
        Command::Rest { token, long } => Some(Intent::Rest {
            token: EntityId(token),
            long,
        }),
        Command::DeathSave { token } => Some(Intent::DeathSave {
            token: EntityId(token),
        }),
        Command::Concentration { token, effect } => Some(Intent::SetConcentration {
            token: EntityId(token),
            effect,
        }),
        Command::Handout { title, text } => Some(Intent::PostHandout { title, text }),
        Command::Fog { x, y, radius } => Some(Intent::AddFog {
            center: Vec2::new(x, y),
            radius,
        }),
        Command::FogClear => Some(Intent::ClearFog),
        Command::NewChar { name, class } => Some(Intent::CreateCharacter {
            name,
            class,
            organization: "Фонд SCP".to_string(),
            access: AccessLevel::First,
        }),
        Command::Check {
            character,
            skill,
            dc,
        } => Some(Intent::RollCharacterCheck {
            character: EntityId(character),
            skill: Some(skill),
            attribute: None,
            dc,
        }),
        Command::AttrCheck {
            character,
            attr,
            dc,
        } => Some(Intent::RollCharacterCheck {
            character: EntityId(character),
            skill: None,
            attribute: Some(attr),
            dc,
        }),
        Command::Sanity { character } => Some(Intent::RollSanity {
            character: EntityId(character),
        }),
        Command::Xp { character, amount } => Some(Intent::GrantXp {
            character: EntityId(character),
            amount,
        }),
        Command::LevelUp { character, attr } => Some(Intent::LevelUpAttribute {
            character: EntityId(character),
            attribute: attr,
        }),
        Command::Wall { a, b, door } => Some(Intent::AddWall {
            a: Vec2::new(a.0, a.1),
            b: Vec2::new(b.0, b.1),
            door,
        }),
        Command::ToggleWall { index } => Some(Intent::ToggleDoor { index }),
        Command::Hp { token, value } => Some(Intent::SetTokenHp {
            token: EntityId(token),
            value,
        }),
        Command::Damage { token, amount } => Some(Intent::DamageToken {
            token: EntityId(token),
            amount,
        }),
        Command::Hide { token, hidden } => Some(Intent::SetTokenHidden {
            token: EntityId(token),
            hidden,
        }),
        Command::Init { token, value } => Some(Intent::SetInitiative {
            token: EntityId(token),
            value,
        }),
        Command::NextTurn => Some(Intent::NextTurn),
        Command::Scene(scene) => Some(Intent::ChangeScene {
            scene: EntityId(scene),
        }),
        Command::Help
        | Command::Tokens
        | Command::Who
        | Command::Save
        | Command::Quit
        | Command::Search(_)
        | Command::Monsters => None,
    }
}

/// Напечатать справку по командам.
pub fn print_help() {
    println!(
        "\
Команды:
  help                       этот текст
  say <текст>                сообщение в чат
  gm <текст>                 шёпот ГМу (виден только ГМу)
  roll <формула>             бросок: 2d6+3, 4d6kh3, 1d20+5
  adv <формула>              бросок с преимуществом
  dis <формула>              бросок с помехой
  move <токен> <x> <y>       подвинуть токен (пиксели сцены)
  create <имя> <x> <y>       создать токен (только ГМ)
  spawn <монстр> <x> <y>     токен из компендиума (например: spawn гоблин 100 100)
  hp <токен> <значение>      установить хиты
  dmg <токен> <n>            нанести урон
  hide <токен> true|false    скрыть/показать (только ГМ)
  rest <токен> [long]        отдых: long — длинный (лечит, снимает смерть)
  death <токен>              спасбросок от смерти (при 0 хитов)
  conc <токен> <эффект>      установить концентрацию (unconc — снять)
  handout <заголовок> | <текст>   показать хендаут игрокам
  fog <x> <y> [радиус]       закрыть область туманом (fogclear — убрать весь)
  init <токен> <n>           задать инициативу
  next                       следующий ход
  scene <id>                 сменить сцену (только ГМ)
  tokens                     список токенов сцены
  monsters                   монстры в компендиуме
  search <текст>             поиск по компендиуму
  newchar [класс] <имя>      создать персонажа SCP (по умолчанию Солдат)
  check <id> <навык> [СЛ]    проверка навыка (например: check 3 Пистолеты 15)
  attrcheck <id> <ХАР> [СЛ]  проверка характеристики (СИЛ, ЛВК, ВСП…)
  sanity <id>                бросок психики (психоз d10 / подъём d4)
  xp <id> <n>                начислить опыт (только ГМ)
  levelup <id> <ХАР>         потратить опыт на характеристику
  wall <x1> <y1> <x2> <y2>   добавить стену (только ГМ)
  door <x1> <y1> <x2> <y2>   добавить дверь (только ГМ)
  toggle <индекс>            открыть/закрыть дверь (только ГМ)
  who                        кто подключён
  save                       сохранить кампанию (хост)
  quit                       выйти"
    );
}

/// Человекочитаемое описание события.
#[must_use]
pub fn describe_event(event: &Event) -> String {
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
            let hidden = if token.hidden { ", скрыт" } else { "" };
            format!("токен {} «{}» обновлён{hp}{hidden}", token.id, token.name)
        }
        Event::ChatPosted {
            from_name,
            text,
            to_gm_only,
            ..
        } => {
            let tag = if *to_gm_only {
                " [только ГМу]"
            } else {
                ""
            };
            format!("💬 {from_name}{tag}: {text}")
        }
        Event::Rolled {
            from_name,
            result,
            to_gm_only,
            ..
        } => {
            let tag = if *to_gm_only {
                " [только ГМу]"
            } else {
                ""
            };
            format!("🎲 {from_name}{tag}: {}", result.describe())
        }
        Event::InitiativeSet { name, value, .. } => {
            format!("инициатива «{name}»: {value}")
        }
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
            successes,
            failures,
            outcome,
            ..
        } => format!(
            "спасбросок от смерти: успехи {successes}, провалы {failures}, итог {outcome:?}"
        ),
        Event::HandoutPosted { title, text } => {
            format!("📜 {title}: {text}")
        }
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

/// Напечатать сообщение сервера (для клиента).
pub fn print_server_msg(msg: &ServerMsg) {
    match msg {
        ServerMsg::Welcome(welcome) => println!(
            "✔ подключено: кампания «{}», роль {}, игрок #{}",
            welcome.campaign,
            welcome.role.name_ru(),
            welcome.player.0
        ),
        ServerMsg::Snapshot(snapshot) => print_snapshot(snapshot),
        ServerMsg::Events(events) => {
            for event in events {
                println!("{}", describe_event(event));
            }
        }
        ServerMsg::Denied { reason } => println!("✖ отказано: {reason}"),
        ServerMsg::Notice { text } => println!("· {text}"),
        ServerMsg::Pong { nonce } => println!("· pong {nonce}"),
        ServerMsg::Rejected { reason } => println!("✖ сервер отклонил подключение: {reason}"),
    }
}

/// Краткая сводка сцены из снапшота.
pub fn print_snapshot(snapshot: &Snapshot) {
    println!(
        "сцена «{}»: {}×{} px, клетка {:.0} px, токенов {}",
        snapshot.scene.name,
        snapshot.scene.width_px as i32,
        snapshot.scene.height_px as i32,
        snapshot.scene.grid.cell_px,
        snapshot.scene.tokens.len()
    );
    for token in &snapshot.scene.tokens {
        let hp = token
            .hp
            .map(|bar| format!(" HP {}/{}", bar.value, bar.max))
            .unwrap_or_default();
        let owner = token
            .owner
            .map(|id| format!(" (владелец #{})", id.0))
            .unwrap_or_default();
        println!(
            "  {} «{}» ({:.0}, {:.0}){hp}{owner}",
            token.id, token.name, token.pos.x, token.pos.y
        );
    }
    if !snapshot.combat.order.is_empty() {
        println!(
            "  бой: раунд {}, ход {}/{}",
            snapshot.combat.round,
            snapshot.combat.turn + 1,
            snapshot.combat.order.len()
        );
    }
}

/// Спросить строку с консоли.
pub fn prompt(text: &str) -> std::io::Result<String> {
    print!("{text}");
    io::stdout().flush()?;
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

/// Локальный IP для подсказки игрокам (без внешних сервисов).
#[must_use]
pub fn local_ip() -> Option<IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}
