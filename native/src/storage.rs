use crate::model::{Event, Manifest, Score};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone)]
pub struct Entry {
    pub directory: PathBuf,
    pub manifest: Manifest,
    pub modified: SystemTime,
}
impl Entry {
    pub fn asset(&self, name: &str) -> PathBuf {
        // Preserve the original basename-only package contract on both platforms.
        self.directory
            .join(name.rsplit(['/', '\\']).next().unwrap_or(""))
    }
    pub fn has_audio(&self) -> bool {
        self.asset(&self.manifest.audio_file_name).is_file()
    }
    pub fn has_score(&self) -> bool {
        self.asset(&self.manifest.score_file_name).is_file()
    }
    pub fn has_jacket(&self) -> bool {
        self.asset(&self.manifest.jacket_file_name).is_file()
    }
    pub fn status(&self) -> &'static str {
        if !self.has_score() {
            "缺少谱面"
        } else if !self.has_audio() {
            "缺少音频"
        } else if !self.has_jacket() {
            "缺少封面"
        } else {
            "就绪"
        }
    }
    pub fn save_manifest(&mut self) -> Result<(), String> {
        self.manifest.normalize_with_id(&short_id()?);
        atomic_write(
            &self.directory.join("manifest.json"),
            &serde_json::to_vec_pretty(&self.manifest).map_err(|e| e.to_string())?,
        )?;
        self.modified = SystemTime::now();
        Ok(())
    }
    pub fn load_score(&self) -> Result<Score, String> {
        let mut score = Score::from_json(&read_text(&self.asset(&self.manifest.score_file_name))?)?;
        score.music_id = self.manifest.music_id();
        Ok(score)
    }
}
pub fn read_text(path: &Path) -> Result<String, String> {
    let f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if f.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
        return Err("Chart exceeds 64 MiB".into());
    }
    let mut text = String::new();
    f.take(64 * 1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 64 * 1024 * 1024 {
        return Err("Chart exceeds 64 MiB".into());
    }
    Ok(text.trim_start_matches('\u{feff}').into())
}
pub fn scan(root: &Path) -> Result<(Vec<Entry>, Vec<String>), String> {
    if !root.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for dir in fs::read_dir(root).map_err(|e| e.to_string())? {
        let dir = dir.map_err(|e| e.to_string())?;
        if !dir.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let path = dir.path().join("manifest.json");
        if !path.is_file() {
            continue;
        }
        match (|| -> Result<Entry, String> {
            let mut manifest: Manifest =
                serde_json::from_str(&read_text(&path)?).map_err(|e| e.to_string())?;
            manifest.normalize_with_id(&short_id()?);
            Ok(Entry {
                directory: dir.path(),
                manifest,
                modified: fs::metadata(path)
                    .and_then(|m| m.modified())
                    .map_err(|e| e.to_string())?,
            })
        })() {
            Ok(entry) => entries.push(entry),
            Err(e) => warnings.push(format!("{}: {e}", dir.path().display())),
        }
    }
    entries.sort_by(|a, b| {
        b.modified.cmp(&a.modified).then_with(|| {
            a.manifest
                .score_title
                .to_lowercase()
                .cmp(&b.manifest.score_title.to_lowercase())
        })
    });
    Ok((entries, warnings))
}
pub fn short_id() -> Result<String, String> {
    let mut bytes = [0; 12];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| e.to_string())?;
    Ok(bytes
        .iter()
        .map(|b| b"0123456789abcdefghijklmnopqrstuvwxyz"[(b % 36) as usize] as char)
        .collect())
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("File has no parent directory")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = parent.join(format!(".opensekai-{}.tmp", short_id()?));
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        File::open(parent)?.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("{}: {e}", path.display()))
}
pub fn create(root: &Path) -> Result<Entry, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut manifest = Manifest {
        id: short_id()?,
        title: "未命名".into(),
        score_title: "未命名谱面".into(),
        user_name: std::env::var("USER").unwrap_or_default(),
        sec_for_music_score_maker: 120,
        music_difficulty_type: "master".into(),
        play_level: 1,
        ..Manifest::default()
    };
    manifest.normalize_with_id("");
    let directory = root.join(format!("{}_{}", manifest.title, manifest.id));
    fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let mut entry = Entry {
        directory,
        manifest,
        modified: SystemTime::now(),
    };
    let score = Score {
        music_id: entry.manifest.music_id(),
        music_score_event_data_list: [
            (0, serde_json::json!(120.)),
            (3, serde_json::json!("4/4")),
            (1, serde_json::json!(1.)),
            (2, serde_json::json!(1.)),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (kind, value))| Event {
            id: i as i32 + 1,
            event_type: kind,
            change_value: value,
            ..Event::default()
        })
        .collect(),
        ..Score::default()
    };
    atomic_write(
        &entry.asset(&entry.manifest.score_file_name),
        score.to_json().map_err(|e| e.to_string())?.as_bytes(),
    )?;
    entry.save_manifest()?;
    Ok(entry)
}
