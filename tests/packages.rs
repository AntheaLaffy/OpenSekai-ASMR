use opensekai::{packages, storage};
use std::{fs, io::Write, path::PathBuf};
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "opensekai-package-test-{}",
            storage::short_id().unwrap()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn package_roundtrip_preserves_original_files_and_changes_copy_identity() {
    let t = Temporary::new();
    let root = t.0.join("library");
    let mut entry = storage::create(&root).unwrap();
    entry.manifest.title = "中文 title".into();
    entry.manifest.score_title = "谱面标题".into();
    entry.save_manifest().unwrap();
    fs::write(entry.directory.join("audio.ogg"), b"test-media-bytes").unwrap();
    fs::create_dir(entry.directory.join("extra")).unwrap();
    fs::write(entry.directory.join("extra/说明.txt"), "必须保留").unwrap();
    let destination = t.0.join("export.zip");
    packages::export_zip(&entry, &destination).unwrap();
    let imported = packages::import_zip(&destination, &root).unwrap();
    assert_eq!(imported.manifest, entry.manifest);
    assert_ne!(imported.directory, entry.directory);
    assert_eq!(
        fs::read(imported.directory.join("audio.ogg")).unwrap(),
        b"test-media-bytes"
    );
    assert_eq!(
        fs::read_to_string(imported.directory.join("extra/说明.txt")).unwrap(),
        "必须保留"
    );
    let copy = packages::duplicate(&entry, &root).unwrap();
    assert_ne!(copy.manifest.id, entry.manifest.id);
    assert_eq!(copy.manifest.score_title, "谱面标题 副本");
    assert_eq!(
        copy.load_score().unwrap().music_id,
        copy.manifest.music_id()
    );
}
#[test]
fn invalid_zip_cannot_escape_or_leave_half_imported_chart() {
    let t = Temporary::new();
    let path = t.0.join("bad.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    zip.start_file("../outside.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"outside").unwrap();
    zip.finish().unwrap();
    assert!(packages::import_zip(&path, &t.0.join("library")).is_err());
    assert!(!t.0.join("outside.txt").exists());
    assert!(!t.0.join("library").exists());
}
#[test]
fn replacement_validates_score_before_changing_file() {
    let t = Temporary::new();
    let mut entry = storage::create(&t.0.join("library")).unwrap();
    let before = fs::read(entry.directory.join("score.json")).unwrap();
    let invalid = t.0.join("invalid.json");
    fs::write(&invalid, b"{bad}").unwrap();
    assert!(packages::replace_file(&mut entry, "score", &invalid).is_err());
    assert_eq!(
        fs::read(entry.directory.join("score.json")).unwrap(),
        before
    );
}
#[test]
fn sus_file_selection_converts_to_the_native_package_contract() {
    let t = Temporary::new();
    let mut entry = storage::create(&t.0.join("library")).unwrap();
    let sus = t.0.join("easy.txt");
    let original = b"#BPM01:120\n#00008:01\n#00112:12\n";
    fs::write(&sus, original).unwrap();
    packages::replace_file(&mut entry, "score", &sus).unwrap();
    assert_eq!(entry.manifest.score_file_name, "score.json");
    let score = entry.load_score().unwrap();
    assert_eq!(score.note_list.len(), 1);
    assert_eq!(score.note_list[0].ticks, 1920);
    assert_eq!(fs::read(&sus).unwrap(), original);
}
