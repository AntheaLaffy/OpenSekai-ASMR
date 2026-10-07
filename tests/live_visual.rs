use opensekai::{
    ffi::AppEvent,
    live::{FlickDirection, HIT_Y, Judge, Live, Session, display_offset, note_geometry},
    model::{Event, Manifest, Note, Score},
    storage::Entry,
    ui::{App, Painting},
};
use std::{
    ffi::CStr,
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn temporary() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ojsk-visual-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn texture(draw: &opensekai::ffi::AppDraw) -> String {
    if draw.text.is_null() || !matches!(draw.kind, 2 | 3) {
        return String::new();
    }
    unsafe { CStr::from_ptr(draw.text) }
        .to_string_lossy()
        .into_owned()
}
#[test]
fn selected_speed_is_persistent_and_changes_visibility_without_moving_hit_time() {
    let project = env!("CARGO_MANIFEST_DIR");
    let data = temporary();
    let mut app = App::new(project, data.to_str().unwrap()).unwrap();
    app.build(false);
    // Real manager hit target, followed by disk reload, rather than a setter-only test.
    assert!(app.event(
        &AppEvent {
            kind: 1,
            key: 1,
            x: 1750.,
            y: 355.,
            ..AppEvent::default()
        },
        ""
    ));
    assert!(app.event(
        &AppEvent {
            kind: 2,
            key: 1,
            x: 1750.,
            y: 355.,
            ..AppEvent::default()
        },
        ""
    ));
    assert!((app.note_speed - 6.1).abs() < 0.001);
    assert!(
        (App::new(project, data.to_str().unwrap())
            .unwrap()
            .note_speed
            - 6.1)
            .abs()
            < 0.001
    );
    let note = Note::default();
    let slow = note_geometry(&note, -0.5, 0., 1., 1.);
    let fast = note_geometry(&note, -0.5, 0., 1., 12.);
    assert!(slow[2] > 100. && fast[2] > 0. && fast[2] < 10.);
    assert!(fast[1] + fast[3] * 0.5 < 25.);
    for speed in [1., 6.1, 10.5, 12.] {
        let rect = note_geometry(&note, 0., 0., 1., speed);
        assert!((rect[1] + rect[3] / 2. - HIT_Y).abs() < 0.001);
        let offset = display_offset(speed);
        let a = note_geometry(&note, -offset - 0.0001, 0., 1., speed);
        let b = note_geometry(&note, -offset + 0.0001, 0., 1., speed);
        assert!(
            a[2] > 0. && b[2] > 0. && (a[2] - b[2]).abs() < 0.1,
            "The note must already exist on the far plane before progress zero"
        );
        assert!(a[1] >= 0.);
    }
    fs::remove_dir_all(data).unwrap();
}
#[test]
fn accepted_note_disappears_in_same_frame_and_feedback_animates_then_expires() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = temporary();
    let score = Score {
        note_list: vec![
            Note {
                id: 1,
                ticks: 480,
                ..Note::default()
            },
            Note {
                id: 2,
                ticks: 1920,
                lane_start: 7,
                lane_end: 8,
                ..Note::default()
            },
        ],
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120),
            ..Event::default()
        }],
        music_score_ticks_max: 1920,
        ..Score::default()
    };
    fs::write(data.join("score.json"), serde_json::to_vec(&score).unwrap()).unwrap();
    let entry = Entry {
        directory: data.clone(),
        manifest: Manifest::default(),
        modified: SystemTime::now(),
    };
    let mut live = Live::new(root, entry, false).unwrap();
    live.event(&AppEvent {
        kind: 16,
        ..AppEvent::default()
    });
    live.session.update(2.5);
    let mut before = Painting::default();
    live.draw(&mut before, false);
    let notes = |paint: &Painting| {
        paint
            .draws
            .iter()
            .filter(|d| texture(d).ends_with("notes.png"))
            .count()
    };
    let before_count = notes(&before);
    assert!(before_count >= 6);
    let quad = before
        .draws
        .iter()
        .find(|d| texture(d).ends_with("notes.png"))
        .unwrap();
    assert_eq!(quad.kind, 3);
    assert!(quad.xy[0] != quad.xy[6]); // top and bottom lie at different lane widths
    live.session.key_lane(0, true);
    assert_eq!(live.session.note_judgement(0), Some(Judge::JustPerfect));
    let mut hit = Painting::default();
    live.draw(&mut hit, false);
    assert_eq!(notes(&hit), before_count - 3);
    let feedback = |paint: &Painting| {
        paint
            .draws
            .iter()
            .filter(|d| {
                texture(d).contains("tex_tap_flare.png")
                    || texture(d).contains("tex_note_common_all_v2.png")
            })
            .count()
    };
    assert!(feedback(&hit) >= 10);
    let modes: Vec<_> = hit
        .draws
        .iter()
        .filter(|d| texture(d).contains("tex_note_common_all_v2.png"))
        .map(|d| d.reserved & 4)
        .collect();
    assert!(
        modes.contains(&0) && modes.contains(&4),
        "Source patterns alpha-blend while the glow layers add light"
    );
    let background = hit
        .draws
        .iter()
        .position(|d| {
            texture(d).contains("tex_note_common_all_v2.png")
                && d.uv == [0.125, 0.25, 0.25, 0.375]
                && d.reserved & 4 == 0
        })
        .unwrap();
    let beam = hit
        .draws
        .iter()
        .position(|d| {
            texture(d).contains("tex_note_common_all_v2.png")
                && d.uv == [0.875, 0.125, 0.9375, 0.25]
        })
        .unwrap();
    assert!(
        background < beam,
        "A key's background plate must not cover its spotlight"
    );
    live.session.update(0.2);
    let mut later = Painting::default();
    live.draw(&mut later, false);
    let a = hit
        .draws
        .iter()
        .find(|d| texture(d).contains("tex_note_common_all_v2.png") && d.kind == 3)
        .unwrap();
    let b = later
        .draws
        .iter()
        .find(|d| texture(d).contains("tex_note_common_all_v2.png") && d.kind == 3)
        .unwrap();
    assert_ne!(a.rect, b.rect);
    assert_ne!(a.rgba, b.rgba);
    live.session.update(0.41);
    let mut expired = Painting::default();
    live.draw(&mut expired, false);
    assert_eq!(feedback(&expired), 0);
    fs::remove_dir_all(data).unwrap();
}

#[test]
fn desktop_contact_then_outer_row_ignores_direction_but_touch_controller_actions_keep_it() {
    for direction in [1, 2] {
        let score = Score {
            note_list: vec![Note {
                id: 1,
                ticks: 480,
                category: 3,
                direction,
                ..Note::default()
            }],
            music_score_event_data_list: vec![Event {
                change_value: serde_json::json!(120),
                ..Event::default()
            }],
            ..Score::default()
        };
        let mut keyboard = Session::new(&score, false, true).unwrap();
        keyboard.update(2.44);
        keyboard.key_lane(0, true);
        keyboard.key_lane(0, false);
        keyboard.update(0.06);
        keyboard.flick_key_lane(0, true);
        keyboard.flick_key_lane(0, true);
        assert_eq!(keyboard.note_judgement(0), Some(Judge::JustPerfect));
        assert_eq!(keyboard.result.counts[0], 1); // one middle contact, one judgement
        let mut gesture = Session::new(&score, false, true).unwrap();
        gesture.update(2.5);
        assert!(gesture.flick_lanes(
            1,
            if direction == 1 {
                FlickDirection::Right
            } else {
                FlickDirection::Left
            }
        ));
        assert_eq!(gesture.note_judgement(0), Some(Judge::Great));
    }
}

#[test]
fn sustaining_head_stays_on_line_and_live_body_pulses_then_ends() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = temporary();
    let score = Score {
        note_list: vec![
            Note {
                id: 1,
                ticks: 480,
                category: 1,
                next_connection_id: 2,
                lane_end: 5,
                ..Note::default()
            },
            Note {
                id: 2,
                ticks: 1440,
                previous_connection_id: 1,
                lane_end: 5,
                ..Note::default()
            },
        ],
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120),
            ..Event::default()
        }],
        music_score_ticks_max: 1440,
        ..Score::default()
    };
    fs::write(data.join("score.json"), serde_json::to_vec(&score).unwrap()).unwrap();
    let entry = Entry {
        directory: data.clone(),
        manifest: Manifest::default(),
        modified: SystemTime::now(),
    };
    let mut live = Live::new(root, entry, false).unwrap();
    live.event(&AppEvent {
        kind: 16,
        ..AppEvent::default()
    });
    live.session.update(2.5);
    live.session.key_lane(0, true);
    live.session.update(0.1);
    let mut held = Painting::default();
    live.draw(&mut held, false);
    let bodies = held
        .draws
        .iter()
        .filter(|d| texture(d).ends_with("longNoteLine.png"))
        .collect::<Vec<_>>();
    assert!(!bodies.is_empty());
    assert!(
        bodies
            .iter()
            .all(|d| d.kind == 3 && d.reserved == 8 && d.min_font_size == -0.25)
    );
    assert!((bodies[0].font_size - 0.3454915).abs() < 0.001);
    let head = held
        .draws
        .iter()
        .filter(|d| texture(d).ends_with("notes.png") && d.rect[1] > 750.)
        .collect::<Vec<_>>();
    assert_eq!(head.len(), 3); // Long head is a held anchor after its source judgement.
    live.session.key_lane(0, false);
    let mut released = Painting::default();
    live.draw(&mut released, false);
    assert!(
        !released
            .draws
            .iter()
            .any(|d| texture(d).ends_with("notes.png") && d.rect[1] > 750.)
    );
    assert!(
        released
            .draws
            .iter()
            .filter(|d| texture(d).ends_with("longNoteLine.png"))
            .all(|d| d.rgba == 0x99b4a7ff && d.font_size == 0.)
    );
    live.session.update(1.);
    let mut ended = Painting::default();
    live.draw(&mut ended, false);
    assert!(
        !ended
            .draws
            .iter()
            .any(|d| texture(d).ends_with("longNoteLine.png"))
    );
    fs::remove_dir_all(data).unwrap();
}

fn live_with_notes(notes: Vec<Note>, maximum: i64) -> (Live, PathBuf) {
    let data = temporary();
    let score = Score {
        note_list: notes,
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120),
            ..Event::default()
        }],
        music_score_ticks_max: maximum,
        ..Score::default()
    };
    fs::write(data.join("score.json"), serde_json::to_vec(&score).unwrap()).unwrap();
    let mut live = Live::new(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        Entry {
            directory: data.clone(),
            manifest: Manifest::default(),
            modified: SystemTime::now(),
        },
        false,
    )
    .unwrap();
    live.event(&AppEvent {
        kind: 16,
        ..AppEvent::default()
    });
    (live, data)
}
fn paint(live: &Live) -> Painting {
    let mut p = Painting::default();
    live.draw(&mut p, false);
    p
}
fn popup_digits(paint: &Painting) -> String {
    let scene: serde_json::Value = serde_json::from_slice(
        &fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("native/generated/editor-scene.json"))
            .unwrap(),
    )
    .unwrap();
    let number = &scene["live"]["hud"]["add_score_numbers"][1];
    let origin_y = number["origin"][1].as_f64().unwrap() as f32;
    let mut found = Vec::new();
    for draw in &paint.draws {
        if draw.kind != 2 || draw.rgba & 255 == 0 || draw.rgba >> 8 != 0xffffff {
            continue;
        }
        for (digit, glyph) in number["digits"].as_array().unwrap().iter().enumerate() {
            let sprite = &scene["sprites"][glyph["guid"].as_str().unwrap()];
            let path = sprite["texture"].as_str().unwrap();
            let rect: [f32; 4] =
                std::array::from_fn(|i| sprite["rect"][i].as_f64().unwrap() as f32);
            let [tw, th]: [f32; 2] =
                std::array::from_fn(|i| scene["textures"][path][i].as_f64().unwrap() as f32);
            let uv = [
                rect[0] / tw,
                1. - (rect[1] + rect[3]) / th,
                (rect[0] + rect[2]) / tw,
                1. - rect[1] / th,
            ];
            if texture(draw).ends_with(path)
                && draw.uv.iter().zip(uv).all(|(a, b)| (a - b).abs() < 1e-5)
                && (draw.rect[1] - origin_y - glyph["rect"][1].as_f64().unwrap() as f32).abs() < 0.1
                && (draw.rect[3] - glyph["rect"][3].as_f64().unwrap() as f32).abs() < 0.1
            {
                found.push((draw.rect[0], char::from(b'0' + digit as u8)));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, digit)| digit).collect()
}
#[test]
fn a_pressed_column_flashes_white_and_fades_even_while_the_key_remains_down() {
    let (mut live, data) = live_with_notes(
        vec![Note {
            id: 1,
            ticks: 4800,
            ..Note::default()
        }],
        4800,
    );
    live.session.update(2.5);
    live.session.key_lane(7, true); // Empty input still has visual touch feedback.
    let alpha = |paint: &Painting| {
        paint
            .draws
            .iter()
            .find(|d| {
                d.kind == 0 && d.rgba >> 8 == 0xffffff && (d.rect[1] - (HIT_Y - 3.)).abs() < 0.01
            })
            .map_or(0, |d| d.rgba & 255)
    };
    let initial = alpha(&paint(&live));
    assert!(initial > 0);
    live.session.update(0.09);
    assert!(alpha(&paint(&live)) < initial);
    live.session.update(0.1);
    assert_eq!(alpha(&paint(&live)), 0);
    assert_eq!(live.session.result.score, 0);
    assert!(popup_digits(&paint(&live)).is_empty());
    fs::remove_dir_all(data).unwrap();
}
#[test]
fn score_popup_shows_the_earned_amount_and_expires_without_a_new_hit() {
    let (mut live, data) = live_with_notes(
        vec![
            Note {
                id: 1,
                ticks: 480,
                ..Note::default()
            },
            Note {
                id: 2,
                ticks: 1440,
                lane_start: 7,
                lane_end: 7,
                ..Note::default()
            },
        ],
        1440,
    );
    live.session.update(2.5);
    assert!(popup_digits(&paint(&live)).is_empty());
    live.session.key_lane(0, true);
    assert_eq!(live.session.result.score, 444_444);
    live.session.update(0.1);
    assert_eq!(popup_digits(&paint(&live)), "444444");
    live.session.key_lane(0, false);
    live.session.update(0.8);
    assert!(popup_digits(&paint(&live)).is_empty());
    live.session.key_lane(11, true);
    assert!(popup_digits(&paint(&live)).is_empty());
    live.session.update(0.13);
    live.session.key_lane(7, true);
    assert_eq!(live.session.note_judgement(1), Some(Judge::Perfect));
    live.session.update(0.1);
    assert_eq!(popup_digits(&paint(&live)), "500000");
    fs::remove_dir_all(data).unwrap();
}
#[test]
fn continuous_hold_sections_keep_their_beam_but_do_not_pin_the_score_popup() {
    let (mut live, data) = live_with_notes(
        vec![
            Note {
                id: 1,
                ticks: 480,
                category: 1,
                next_connection_id: 2,
                lane_end: 3,
                ..Note::default()
            },
            Note {
                id: 2,
                ticks: 4320,
                previous_connection_id: 1,
                lane_end: 3,
                ..Note::default()
            },
        ],
        4320,
    );
    live.session.update(2.5);
    live.session.key_lane(0, true);
    live.session.update(1.1);
    let held = paint(&live);
    assert!(popup_digits(&held).is_empty());
    let is_effect = |d: &opensekai::ffi::AppDraw| {
        texture(d).contains("tex_note_common_all_v2.png") && d.rgba & 255 > 0
    };
    assert!(held.draws.iter().any(is_effect));
    let bodies = held
        .draws
        .iter()
        .filter(|d| texture(d).ends_with("longNoteLine.png"))
        .collect::<Vec<_>>();
    assert!(!bodies.is_empty());
    assert!(bodies.iter().all(|d| {
        d.clip[1] + d.clip[3] == HIT_Y
            && [d.xy[1], d.xy[3], d.xy[5], d.xy[7]]
                .into_iter()
                .all(|y| y <= HIT_Y)
    }));
    live.session.key_lane(0, false);
    assert!(!paint(&live).draws.iter().any(is_effect));
    fs::remove_dir_all(data).unwrap();
}
#[test]
fn flick_marker_has_its_own_small_sprite_and_rises_fades_and_restarts_above_the_base() {
    let note = Note {
        id: 1,
        ticks: 2400,
        category: 3,
        lane_end: 3,
        ..Note::default()
    };
    let (mut live, data) = live_with_notes(vec![note.clone()], 2400);
    live.session.update(3.05);
    let sample = |live: &Live| {
        let p = paint(live);
        let d = p
            .draws
            .iter()
            .find(|d| texture(d).ends_with("notes_flick_arrow_04.png"))
            .unwrap();
        let base = note_geometry(&note, live.session.time, 2.5, 1., live.note_speed);
        let scale = base[3] / (1.7222 * 108.);
        assert!(d.rect[2] < base[2] * 0.7);
        assert!(d.rect[1] + d.rect[3] < base[1] + base[3] / 2.);
        (
            (base[1] + base[3] / 2. - d.rect[1] - d.rect[3]) / scale,
            d.rgba & 255,
        )
    };
    let low = sample(&live);
    live.session.update(0.2);
    let high = sample(&live);
    assert!(high.0 > low.0 + 70.);
    live.session.update(0.2);
    let faded = sample(&live);
    assert!(faded.0 > high.0 + 70. && faded.1 < high.1);
    live.session.update(0.1);
    let restarted = sample(&live);
    assert!(restarted.0 < faded.0 - 100. && restarted.1 > faded.1);
    fs::remove_dir_all(data).unwrap();
}
