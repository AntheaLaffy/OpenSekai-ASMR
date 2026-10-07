use opensekai::{
    ffi::AppEvent,
    live::{Live, ResultData},
    live_result_ui::{SETTLED, View},
    model::{Event, Manifest, Note, Score},
    rig2d::Rig,
    storage::Entry,
    ui::Painting,
    unity::Bundle,
};
use std::{
    ffi::{CStr, CString},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

fn label(draw: &opensekai::ffi::AppDraw) -> String {
    if draw.text.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(draw.text) }
        .to_string_lossy()
        .into_owned()
}
fn result() -> ResultData {
    ResultData {
        score: 987654,
        counts: [1000, 112, 39, 0, 0, 0, 0],
        max_combo: 1151,
        total: 1151,
        life: 1000,
        ..ResultData::default()
    }
}

#[test]
fn result_cover_enters_and_real_score_counts_up_to_the_exact_saved_value() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let result = result();
    let manifest = Manifest::default();
    let cover = CString::new("test-cover.png").unwrap();
    let frame = |time| {
        let mut p = Painting::default();
        View {
            bundle: &bundle,
            manifest: &manifest,
            result: &result,
            jacket: Some(&cover),
            figure: None,
            english: false,
            elapsed: time,
        }
        .draw(&mut p);
        p
    };
    let jacket = |p: &Painting| {
        *p.draws
            .iter()
            .find(|d| d.kind == 2 && label(d) == "test-cover.png")
            .unwrap()
    };
    let early = frame(2.05);
    let later = frame(2.3);
    let final_frame = frame(SETTLED);
    assert!(jacket(&early).rect[1] < jacket(&later).rect[1]);
    assert!((jacket(&early).rgba & 255) < (jacket(&later).rgba & 255));
    assert_eq!(jacket(&final_frame).rect, [229., 29., 117., 120.]);
    let score = |p: &Painting| {
        label(
            p.draws
                .iter()
                .find(|d| d.kind == 1 && d.font_size == 94. && d.rgba & 255 > 0)
                .unwrap(),
        )
        .parse::<u32>()
        .unwrap()
    };
    let first = score(&frame(2.6));
    let middle = score(&frame(3.));
    assert!(first > 0 && first < middle && middle < result.score);
    assert_eq!(score(&final_frame), result.score);
    assert_eq!(result.score, 987654); // Display animation never changes the saved gameplay data.
}

#[test]
fn rank_gauge_and_statistics_animate_and_all_locales_end_with_real_counts() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let manifest = Manifest::default();
    let result = result();
    for english in [false, true] {
        let frame = |time| {
            let mut p = Painting::default();
            View {
                bundle: &bundle,
                manifest: &manifest,
                result: &result,
                jacket: None,
                figure: None,
                english,
                elapsed: time,
            }
            .draw(&mut p);
            p
        };
        let width = |p: &Painting| {
            p.draws
                .iter()
                .filter(|d| d.kind == 0 && d.rect[1] == 529. && d.rgba == 0x5af5caff)
                .map(|d| d.rect[2])
                .sum::<f32>()
        };
        assert!(width(&frame(0.3)) < width(&frame(0.7)));
        let row = |p: &Painting| {
            label(
                p.draws
                    .iter()
                    .find(|d| {
                        d.kind == 1 && d.font_size == 36. && d.rect[2] == 130. && d.rect[1] == 637.
                    })
                    .unwrap(),
            )
            .parse::<u32>()
            .unwrap()
        };
        assert!(row(&frame(2.95)) > 0 && row(&frame(2.95)) < 1112);
        assert_eq!(row(&frame(SETTLED)), 1112);
        let final_frame = frame(SETTLED);
        let values: Vec<_> = final_frame
            .draws
            .iter()
            .filter(|d| d.kind == 1 && d.font_size == 36. && d.rect[2] == 130.)
            .map(label)
            .collect();
        assert_eq!(values, ["1112", "0039", "0000", "0000", "0000"]);
        assert!(
            final_frame
                .draws
                .iter()
                .any(|d| d.kind == 1 && label(d) == "1151")
        );
        assert!(
            !frame(3.5)
                .draws
                .iter()
                .any(|d| d.kind == 1 && label(d).contains("Enter") && d.rgba & 255 > 0)
        );
        assert!(
            final_frame
                .draws
                .iter()
                .any(|d| d.kind == 1 && label(d).contains("Enter") && d.rgba & 255 == 255)
        );
        for time in [f32::NAN, f32::INFINITY, -1., 0., 0.5, 2.3, 3., 20.] {
            assert!(
                frame(time)
                    .draws
                    .iter()
                    .all(|d| d.rect.iter().chain(&d.xy).all(|x| x.is_finite()))
            );
        }
    }
}

#[test]
fn settled_character_keeps_animating_while_both_locales_show_the_same_final_data() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let bundle = Bundle::load(root).unwrap();
    let rig = Rig::load(&root.join("resources/generated/results/rig-v1/rig.json")).unwrap();
    let manifest = Manifest::default();
    let result = result();
    for english in [false, true] {
        let frame = |time| {
            let mut p = Painting::default();
            View {
                bundle: &bundle,
                manifest: &manifest,
                result: &result,
                jacket: None,
                figure: Some(&rig),
                english,
                elapsed: time,
            }
            .draw(&mut p);
            p
        };
        let first = frame(SETTLED);
        let later = frame(SETTLED + 3.2);
        let figure = |p: &Painting| {
            p.draws
                .iter()
                .filter(|d| d.text == rig.texture().as_ptr())
                .map(|d| (d.xy, d.uv))
                .collect::<Vec<_>>()
        };
        assert_eq!(figure(&first).len(), 14);
        assert_ne!(figure(&first), figure(&later));
        let data = |p: &Painting| {
            p.draws
                .iter()
                .filter(|d| d.text != rig.texture().as_ptr())
                .map(|d| (d.kind, d.rgba, d.rect, d.xy, d.uv, label(d)))
                .collect::<Vec<_>>()
        };
        assert_eq!(data(&first), data(&later));
        assert!(first.draws.iter().any(|d| label(d) == "00987654"));
    }
}

#[test]
fn result_clock_runs_after_music_and_keeps_retry_locked_until_the_intro_finishes() {
    let directory = std::env::temp_dir().join(format!(
        "ojsk-result-clock-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let chart = Score {
        note_list: vec![Note {
            id: 1,
            ..Note::default()
        }],
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120),
            ..Event::default()
        }],
        ..Score::default()
    };
    fs::write(
        directory.join("score.json"),
        serde_json::to_vec(&chart).unwrap(),
    )
    .unwrap();
    let entry = Entry {
        directory: directory.clone(),
        manifest: Manifest::default(),
        modified: SystemTime::now(),
    };
    let mut live = Live::new(Path::new(env!("CARGO_MANIFEST_DIR")), entry, true).unwrap();
    live.event(&AppEvent {
        kind: 16,
        ..AppEvent::default()
    });
    live.update(3.);
    assert!(live.session.finished && live.result_age().is_none());
    live.update(7.);
    assert!(!live.result_ready());
    assert_eq!(live.result_age(), Some(0.));
    let epoch = live.epoch;
    live.event(&AppEvent {
        kind: 5,
        key: 114,
        ..AppEvent::default()
    });
    assert_eq!(live.epoch, epoch);
    let record_before = fs::read(directory.join("native-last-result.json")).unwrap();
    live.update(4.2);
    assert!(live.result_ready());
    assert_eq!(
        fs::read(directory.join("native-last-result.json")).unwrap(),
        record_before
    );
    live.event(&AppEvent {
        kind: 5,
        key: 114,
        ..AppEvent::default()
    });
    assert_ne!(live.epoch, epoch);
    assert!(live.result_age().is_none());
    fs::remove_dir_all(directory).unwrap();
}
