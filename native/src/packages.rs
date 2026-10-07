//! Local song packages retain the original manifest/score/media directory form.
use crate::{
    model::{Manifest, Score},
    storage::{self, Entry},
};
use std::{
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
type Result<T> = std::result::Result<T, String>;
struct Staging(PathBuf);
impl Staging {
    fn new(parent: &Path) -> Result<Self> {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let p = parent.join(format!(".opensekai-stage-{}", storage::short_id()?));
        fs::create_dir(&p).map_err(|e| e.to_string())?;
        Ok(Self(p))
    }
}
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn folder_name(m: &Manifest) -> String {
    let mut title = String::new();
    for c in m.title.chars() {
        let c = if c.is_control() || c.is_whitespace() || "/\\:*?\"<>|".contains(c) {
            '_'
        } else {
            c
        };
        if c != '_' || !title.ends_with('_') {
            title.push(c);
        }
    }
    let title = title.trim_matches('_');
    format!(
        "{}_{}",
        if title.is_empty() { "Untitled" } else { title },
        m.id
    )
}
fn unique(parent: &Path, name: &str) -> PathBuf {
    let p = parent.join(name);
    if !p.exists() {
        return p;
    }
    for i in 1.. {
        let p = parent.join(format!("{name}_{i}"));
        if !p.exists() {
            return p;
        }
    }
    unreachable!()
}
fn list_files(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
        for d in fs::read_dir(current).map_err(|e| e.to_string())? {
            let d = d.map_err(|e| e.to_string())?;
            let kind = d.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                return Err(format!(
                    "Package contains a symbolic link: {}",
                    d.path().display()
                ));
            }
            if kind.is_dir() {
                walk(root, &d.path(), out)?;
            } else if kind.is_file() {
                out.push(
                    d.path()
                        .strip_prefix(root)
                        .map_err(|e| e.to_string())?
                        .to_path_buf(),
                );
            } else {
                return Err(format!(
                    "Not an ordinary package file: {}",
                    d.path().display()
                ));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}
fn load_manifest(directory: &Path) -> Result<Manifest> {
    let mut manifest: Manifest =
        serde_json::from_str(&storage::read_text(&directory.join("manifest.json"))?)
            .map_err(|e| e.to_string())?;
    manifest.normalize_with_id(&storage::short_id()?);
    // IDs form part of folder names; do not allow a foreign package to choose a
    // path outside the selected library. Original generated IDs are base36.
    if manifest
        .id
        .chars()
        .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        return Err("Manifest ID contains a path separator or invalid filename character".into());
    }
    Ok(manifest)
}
fn copy_to_library(source: &Path, root: &Path, manifest: Manifest) -> Result<Entry> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if root.starts_with(&source) {
        return Err("Cannot import a folder into itself".into());
    }
    let files = list_files(&source)?;
    let stage = Staging::new(&root)?;
    for rel in files {
        let dest = stage.0.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(source.join(&rel), dest).map_err(|e| e.to_string())?;
    }
    let mut entry = Entry {
        directory: stage.0.clone(),
        manifest,
        modified: SystemTime::now(),
    };
    if entry.has_score() {
        entry.load_score()?;
    }
    entry.save_manifest()?;
    let destination = unique(&root, &folder_name(&entry.manifest));
    fs::rename(&stage.0, &destination).map_err(|e| e.to_string())?;
    entry.directory = destination;
    Ok(entry)
}
pub fn import_folder(source: &Path, root: &Path) -> Result<Entry> {
    copy_to_library(source, root, load_manifest(source)?)
}
pub fn duplicate(source: &Entry, root: &Path) -> Result<Entry> {
    let mut manifest = source.manifest.clone();
    manifest.id = storage::short_id()?;
    manifest.score_title = if manifest.score_title.trim().is_empty() {
        "副本".into()
    } else {
        format!("{} 副本", manifest.score_title)
    };
    copy_to_library(&source.directory, root, manifest)
}
pub fn import_zip(path: &Path, root: &Path) -> Result<Entry> {
    let mut zip =
        ZipArchive::new(File::open(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if zip.len() > 10000 {
        return Err("Package exceeds 10,000 entries".into());
    }
    let stage = Staging::new(&std::env::temp_dir())?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        let rel = file.enclosed_name().ok_or("ZIP path escapes the package")?;
        let rel = PathBuf::from(rel.to_string_lossy().replace('\\', "/"));
        if rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
            || rel.to_string_lossy().contains(':')
        {
            return Err("ZIP path is not relative".into());
        }
        if file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000) {
            return Err("ZIP symbolic links are unsupported".into());
        }
        let dest = stage.0.join(rel);
        if file.is_dir() {
            fs::create_dir_all(dest).map_err(|e| e.to_string())?;
            continue;
        }
        total = total.checked_add(file.size()).ok_or("ZIP size overflow")?;
        if total > 2 * 1024 * 1024 * 1024 {
            return Err("Package exceeds 2 GiB uncompressed".into());
        }
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(|e| e.to_string())?;
        let declared = file.size();
        let copied = std::io::copy(&mut (&mut file).take(declared + 1), &mut output)
            .map_err(|e| e.to_string())?;
        if copied != declared {
            return Err("ZIP size mismatch".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
    }
    let candidates: Vec<_> = list_files(&stage.0)?
        .into_iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "manifest.json"))
        .collect();
    let selected = candidates
        .iter()
        .find(|p| p.components().count() == 1)
        .or_else(|| candidates.first())
        .ok_or("ZIP contains no manifest.json")?;
    import_folder(
        &stage.0.join(selected.parent().unwrap_or(Path::new(""))),
        root,
    )
}
pub fn export_zip(entry: &Entry, destination: &Path) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or("Export destination has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if parent
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(entry.directory.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("Export ZIP must be outside the song folder".into());
    }
    let files = list_files(&entry.directory)?;
    let stage = Staging::new(parent)?;
    let tmp = stage.0.join("package.zip");
    let mut zip = ZipWriter::new(File::create(&tmp).map_err(|e| e.to_string())?);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for rel in files {
        zip.start_file(rel.to_string_lossy().replace('\\', "/"), options)
            .map_err(|e| e.to_string())?;
        std::io::copy(
            &mut File::open(entry.directory.join(&rel)).map_err(|e| e.to_string())?,
            &mut zip,
        )
        .map_err(|e| e.to_string())?;
    }
    zip.finish()
        .map_err(|e| e.to_string())?
        .sync_all()
        .map_err(|e| e.to_string())?;
    fs::rename(tmp, destination).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn replace_file(entry: &mut Entry, slot: &str, source: &Path) -> Result<()> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let allowed = match slot {
        "audio" => ["ogg", "mp3", "wav"].as_slice(),
        "jacket" => ["png", "jpg", "jpeg"].as_slice(),
        "score" => ["json", "sus", "txt"].as_slice(),
        "video" => ["mp4"].as_slice(),
        _ => return Err("Unknown asset slot".into()),
    };
    if !allowed.contains(&extension.as_str()) {
        return Err(format!("Unsupported {slot} extension: {extension}"));
    }
    let name = if slot == "score" {
        "score.json".into()
    } else {
        format!("{slot}.{extension}")
    };
    let bytes = if slot == "score" {
        let text = storage::read_text(source)?;
        let mut score = if extension == "json" {
            Score::from_json(&text)?
        } else {
            crate::sus::parse(&text)?
        };
        let issues = score.validate();
        if !issues.is_empty() {
            return Err(issues.join("; "));
        }
        score.music_id = entry.manifest.music_id();
        score.to_json().map_err(|e| e.to_string())?.into_bytes()
    } else {
        fs::read(source).map_err(|e| e.to_string())?
    };
    storage::atomic_write(&entry.directory.join(&name), &bytes)?;
    match slot {
        "audio" => entry.manifest.audio_file_name = name,
        "jacket" => entry.manifest.jacket_file_name = name,
        "score" => entry.manifest.score_file_name = name,
        "video" => entry.manifest.video_file_name = name,
        _ => unreachable!(),
    }
    entry.save_manifest()
}
