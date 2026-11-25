//! Сохранение и загрузка кампании на диск.
//!
//! Формат намеренно простой: `campaign.json`. Позже рядом появится
//! `assets/` с файлами по хэшу и SQLite — но JSON полезно оставить:
//! он читается глазами, диффится в git и не требует миграций для MVP.

use crate::campaign::Campaign;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use vtt_core::content::ContentPack;

/// Имя файла кампании.
pub const CAMPAIGN_FILE: &str = "campaign.json";

/// Загрузить пак контента из JSON-файла.
pub fn load_pack_file(path: impl AsRef<Path>) -> io::Result<ContentPack> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

/// Сохранить кампанию в каталог (создаст его при необходимости).
///
/// Возвращает путь к записанному файлу.
pub fn save_campaign(campaign: &Campaign, dir: impl AsRef<Path>) -> io::Result<PathBuf> {
    let dir = dir.as_ref();
    fs::create_dir_all(dir)?;
    let path = dir.join(CAMPAIGN_FILE);
    let json = serde_json::to_string_pretty(campaign)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    // Пишем через временный файл, чтобы не оставить «полукампанию» при сбое.
    let tmp = dir.join(format!("{CAMPAIGN_FILE}.tmp"));
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Загрузить кампанию из каталога.
pub fn load_campaign(dir: impl AsRef<Path>) -> io::Result<Campaign> {
    let path = dir.as_ref().join(CAMPAIGN_FILE);
    let json = fs::read_to_string(&path)?;
    let mut campaign: Campaign =
        serde_json::from_str(&json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    // Страховка от старых/битых файлов: если сцен нет — создаём пустую.
    if campaign.scenes.is_empty() {
        let scene_id = campaign.next_id();
        campaign
            .scenes
            .push(vtt_core::scene::Scene::new(scene_id, "Сцена 1"));
        campaign.active_scene = scene_id;
    }
    if campaign.scene_by_id(campaign.active_scene).is_none() {
        campaign.active_scene = campaign.scenes[0].id;
    }
    Ok(campaign)
}

/// Существует ли кампания в каталоге.
#[must_use]
pub fn campaign_exists(dir: impl AsRef<Path>) -> bool {
    dir.as_ref().join(CAMPAIGN_FILE).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        // Пишем в target/, а не в системный TEMP: так тесты работают в песочницах
        // и не зависят от прав на временный каталог.
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-tmp")
            .join(format!("{name}-{nanos}-{n}"))
    }

    #[test]
    fn pack_file_loads() {
        let dir = temp_dir("pack");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.json");
        fs::write(
            &path,
            r#"{
                "source": { "id": "test", "name": "Тест", "license": "private" },
                "monsters": [{
                    "name": "Крыса",
                    "armor_class": 10,
                    "hit_points": 1,
                    "abilities": {"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10}
                }]
            }"#,
        )
        .unwrap();

        let pack = load_pack_file(&path).unwrap();
        assert_eq!(pack.monsters.len(), 1);
        assert_eq!(pack.source.license, "private");
        assert!(pack.source.attribution.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn broken_pack_file_errors_cleanly() {
        let dir = temp_dir("pack-broken");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.json");
        fs::write(&path, "не json").unwrap();
        let err = load_pack_file(&path).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = temp_dir("roundtrip");
        let mut campaign = Campaign::new("Поход");
        campaign.add_actor(vtt_core::entities::Actor::new(
            vtt_core::ids::EntityId::NONE,
            "Бард",
            2,
        ));

        let path = save_campaign(&campaign, &dir).unwrap();
        assert!(path.exists());
        assert!(campaign_exists(&dir));

        let loaded = load_campaign(&dir).unwrap();
        assert_eq!(loaded, campaign);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_dir_errors() {
        let dir = temp_dir("missing");
        assert!(load_campaign(&dir).is_err());
        assert!(!campaign_exists(&dir));
    }

    #[test]
    fn broken_file_errors_cleanly() {
        let dir = temp_dir("broken");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(CAMPAIGN_FILE), "{ это не json }").unwrap();
        let err = load_campaign(&dir).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        fs::remove_dir_all(&dir).ok();
    }
}
