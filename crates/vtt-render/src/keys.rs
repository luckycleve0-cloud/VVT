//! Настраиваемые горячие клавиши.
//!
//! Соответствие «действие → клавиша» хранится в `keys.json` рядом с кампанией.
//! В настройках (вкладка «Клавиши») любую клавишу можно переназначить.

use std::collections::BTreeMap;
use std::path::Path;

use macroquad::prelude::KeyCode;

/// Действие, на которое можно назначить клавишу.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Select,
    Ruler,
    Fog,
    Ping,
    Wall,
    Door,
    Grid,
    FogToggle,
    Sheet,
    Roster,
    Chat,
    TokenPanel,
    Settings,
    Wizard,
    ClearFog,
    Save,
    Monster,
    NewToken,
    Delete,
    Quit,
}

impl Action {
    /// Все действия в порядке отображения.
    pub const ALL: [Action; 20] = [
        Action::Select,
        Action::Ruler,
        Action::Fog,
        Action::Ping,
        Action::Wall,
        Action::Door,
        Action::Grid,
        Action::FogToggle,
        Action::Sheet,
        Action::Roster,
        Action::Chat,
        Action::TokenPanel,
        Action::Settings,
        Action::Wizard,
        Action::ClearFog,
        Action::Save,
        Action::Monster,
        Action::NewToken,
        Action::Delete,
        Action::Quit,
    ];

    /// Название для интерфейса.
    #[must_use]
    pub fn name_ru(self) -> &'static str {
        match self {
            Self::Select => "Инструмент: выбор",
            Self::Ruler => "Инструмент: линейка",
            Self::Fog => "Инструмент: туман",
            Self::Ping => "Инструмент: пинг",
            Self::Wall => "Инструмент: стена",
            Self::Door => "Инструмент: дверь",
            Self::Grid => "Сетка вкл/выкл",
            Self::FogToggle => "Туман памяти вкл/выкл",
            Self::Sheet => "Лист персонажа",
            Self::Roster => "Список персонажей",
            Self::Chat => "Чат",
            Self::TokenPanel => "Панель токена",
            Self::Settings => "Настройки",
            Self::Wizard => "Создать персонажа",
            Self::ClearFog => "Убрать туман",
            Self::Save => "Сохранить",
            Self::Monster => "Поставить гоблина",
            Self::NewToken => "Новый токен",
            Self::Delete => "Удалить выбранное",
            Self::Quit => "Выход (Ctrl)",
        }
    }

    /// Стабильный ключ для JSON.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Ruler => "ruler",
            Self::Fog => "fog",
            Self::Ping => "ping",
            Self::Wall => "wall",
            Self::Door => "door",
            Self::Grid => "grid",
            Self::FogToggle => "fog_toggle",
            Self::Sheet => "sheet",
            Self::Roster => "roster",
            Self::Chat => "chat",
            Self::TokenPanel => "token_panel",
            Self::Settings => "settings",
            Self::Wizard => "wizard",
            Self::ClearFog => "clear_fog",
            Self::Save => "save",
            Self::Monster => "monster",
            Self::NewToken => "new_token",
            Self::Delete => "delete",
            Self::Quit => "quit",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
}

/// Текущая раскладка.
#[derive(Debug, Clone)]
pub struct Keymap {
    map: BTreeMap<Action, KeyCode>,
}

impl Default for Keymap {
    fn default() -> Self {
        let mut map = BTreeMap::new();
        for (action, key) in [
            (Action::Select, KeyCode::Key1),
            (Action::Ruler, KeyCode::Key2),
            (Action::Fog, KeyCode::Key3),
            (Action::Ping, KeyCode::Key4),
            (Action::Wall, KeyCode::Key5),
            (Action::Door, KeyCode::Key6),
            (Action::Grid, KeyCode::G),
            (Action::FogToggle, KeyCode::F),
            (Action::Sheet, KeyCode::Tab),
            (Action::Roster, KeyCode::L),
            (Action::Chat, KeyCode::T),
            (Action::TokenPanel, KeyCode::Y),
            (Action::Settings, KeyCode::R),
            (Action::Wizard, KeyCode::P),
            (Action::ClearFog, KeyCode::C),
            (Action::Save, KeyCode::S),
            (Action::Monster, KeyCode::B),
            (Action::NewToken, KeyCode::N),
            (Action::Delete, KeyCode::Delete),
            (Action::Quit, KeyCode::Q),
        ] {
            map.insert(action, key);
        }
        Self { map }
    }
}

impl Keymap {
    #[must_use]
    pub fn key(&self, action: Action) -> KeyCode {
        self.map.get(&action).copied().unwrap_or(KeyCode::Unknown)
    }

    /// Назначить клавишу; если она уже занята другим действием — снять её.
    pub fn set(&mut self, action: Action, key: KeyCode) {
        for value in self.map.values_mut() {
            if *value == key {
                *value = KeyCode::Unknown;
            }
        }
        self.map.insert(action, key);
    }

    #[must_use]
    pub fn pressed(&self, action: Action) -> bool {
        macroquad::input::is_key_pressed(self.key(action))
    }

    /// Человекочитаемое имя клавиши.
    #[must_use]
    pub fn label(&self, action: Action) -> String {
        key_label(self.key(action))
    }

    /// Сохранить раскладку в файл.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut lines = Vec::new();
        for action in Action::ALL {
            lines.push(format!(
                "\"{}\": \"{}\"",
                action.id(),
                key_label(self.key(action))
            ));
        }
        let text = format!("{{\n  {}\n}}\n", lines.join(",\n  "));
        std::fs::write(path, text)
    }

    /// Загрузить раскладку; отсутствующие клавиши остаются по умолчанию.
    pub fn load(path: &Path) -> Self {
        let mut keymap = Self::default();
        let Ok(text) = std::fs::read_to_string(path) else {
            return keymap;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
            return keymap;
        };
        let Some(object) = json.as_object() else {
            return keymap;
        };
        for (id, value) in object {
            let Some(action) = Action::from_id(id) else {
                continue;
            };
            let Some(label) = value.as_str() else {
                continue;
            };
            if let Some(key) = key_from_label(label) {
                keymap.map.insert(action, key);
            }
        }
        keymap
    }
}

/// Подпись клавиши.
#[must_use]
pub fn key_label(key: KeyCode) -> String {
    match key {
        KeyCode::Unknown => "—".to_string(),
        KeyCode::Key1 => "1".to_string(),
        KeyCode::Key2 => "2".to_string(),
        KeyCode::Key3 => "3".to_string(),
        KeyCode::Key4 => "4".to_string(),
        KeyCode::Key5 => "5".to_string(),
        KeyCode::Key6 => "6".to_string(),
        KeyCode::Key7 => "7".to_string(),
        KeyCode::Key8 => "8".to_string(),
        KeyCode::Key9 => "9".to_string(),
        KeyCode::Key0 => "0".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Space => "Space".to_string(),
        KeyCode::LeftShift => "Shift".to_string(),
        KeyCode::LeftControl => "Ctrl".to_string(),
        KeyCode::LeftAlt => "Alt".to_string(),
        other => format!("{other:?}"),
    }
}

/// Разбор подписи обратно в клавишу (латиница A–Z и цифры).
#[must_use]
pub fn key_from_label(label: &str) -> Option<KeyCode> {
    let upper = label.trim();
    let key = match upper {
        "1" => KeyCode::Key1,
        "2" => KeyCode::Key2,
        "3" => KeyCode::Key3,
        "4" => KeyCode::Key4,
        "5" => KeyCode::Key5,
        "6" => KeyCode::Key6,
        "7" => KeyCode::Key7,
        "8" => KeyCode::Key8,
        "9" => KeyCode::Key9,
        "0" => KeyCode::Key0,
        "Del" => KeyCode::Delete,
        "Tab" => KeyCode::Tab,
        "Space" => KeyCode::Space,
        "Shift" => KeyCode::LeftShift,
        "Ctrl" => KeyCode::LeftControl,
        "Alt" => KeyCode::LeftAlt,
        _ => {
            let mut chars = upper.chars();
            let first = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            match first.to_ascii_uppercase() {
                'A' => KeyCode::A,
                'B' => KeyCode::B,
                'C' => KeyCode::C,
                'D' => KeyCode::D,
                'E' => KeyCode::E,
                'F' => KeyCode::F,
                'G' => KeyCode::G,
                'H' => KeyCode::H,
                'I' => KeyCode::I,
                'J' => KeyCode::J,
                'K' => KeyCode::K,
                'L' => KeyCode::L,
                'M' => KeyCode::M,
                'N' => KeyCode::N,
                'O' => KeyCode::O,
                'P' => KeyCode::P,
                'Q' => KeyCode::Q,
                'R' => KeyCode::R,
                'S' => KeyCode::S,
                'T' => KeyCode::T,
                'U' => KeyCode::U,
                'V' => KeyCode::V,
                'W' => KeyCode::W,
                'X' => KeyCode::X,
                'Y' => KeyCode::Y,
                'Z' => KeyCode::Z,
                _ => return None,
            }
        }
    };
    Some(key)
}

/// Первая нажатая клавиша (для режима «нажмите клавишу»).
#[must_use]
pub fn first_pressed() -> Option<KeyCode> {
    let keys = macroquad::input::get_keys_pressed();
    let mut list: Vec<KeyCode> = keys.into_iter().collect();
    list.sort_by_key(|key| format!("{key:?}"));
    list.into_iter().next()
}
