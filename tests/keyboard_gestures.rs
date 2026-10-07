use opensekai::{
    ffi::AppEvent,
    live::{Judge, KEYS, LOWER_FLICK_KEYS, Live, Session},
    model::{Event, Manifest, Note, Score},
    storage::Entry,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn score(notes: Vec<Note>) -> Score {
    Score {
        music_score_ticks_max: notes.iter().map(|n| n.ticks).max().unwrap(),
        note_list: notes,
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120.),
            ..Event::default()
        }],
        ..Score::default()
    }
}
fn flick(id: i32, tick: i64) -> Note {
    Note {
        id,
        ticks: tick,
        category: 3,
        direction: 1,
        ..Note::default()
    }
}
fn one() -> Session {
    Session::new(&score(vec![flick(1, 480)]), false, true).unwrap()
}
fn contact(session: &mut Session, lane: usize) {
    session.key_lane(lane, true);
    session.key_lane(lane, false);
}

#[test]
fn either_outer_row_finishes_one_fresh_released_middle_contact() {
    for lower in [false, true] {
        let mut game = one();
        game.update(2.44);
        contact(&mut game, 0);
        assert_eq!(game.note_judgement(0), None);
        game.update(0.06);
        game.flick_row_lane(0, lower, true);
        assert_eq!(game.note_judgement(0), Some(Judge::JustPerfect));
        assert_eq!(game.result.counts[0], 1);
    }
}

#[test]
fn outer_rows_cannot_replace_ordinary_keys_or_sustain_a_hold() {
    for lower in [false, true] {
        let mut tap = Session::new(
            &score(vec![Note {
                id: 1,
                ticks: 480,
                ..Note::default()
            }]),
            false,
            true,
        )
        .unwrap();
        tap.update(2.5);
        tap.flick_row_lane(0, lower, true);
        assert_eq!(tap.note_judgement(0), None);
        tap.flick_row_lane(0, lower, false);
        tap.key_lane(0, true);
        assert_eq!(tap.note_judgement(0), Some(Judge::JustPerfect));
        let mut hold = Session::new(
            &score(vec![
                Note {
                    id: 1,
                    ticks: 480,
                    category: 1,
                    next_connection_id: 2,
                    ..Note::default()
                },
                Note {
                    id: 2,
                    ticks: 960,
                    previous_connection_id: 1,
                    ..Note::default()
                },
            ]),
            false,
            true,
        )
        .unwrap();
        hold.update(2.5);
        contact(&mut hold, 0);
        hold.flick_row_lane(0, lower, true);
        hold.update(0.4); // Inner hold-combo tick has passed its miss window.
        assert_eq!(hold.result.counts[0], 1);
        assert!(hold.result.counts[5] > 0);
    }
}

#[test]
fn missing_release_wrong_column_stale_and_fabricated_contacts_do_not_flick() {
    for bad in 0..5 {
        let mut game = one();
        match bad {
            0 => {
                game.update(2.5);
            } // Outer key alone.
            1 => {
                game.update(2.44);
                game.key_lane(0, true);
                game.update(0.06);
            }
            2 => {
                game.update(2.44);
                contact(&mut game, 11);
                game.update(0.06);
            }
            3 => {
                game.update(2.30);
                contact(&mut game, 0);
                game.update(0.20);
            }
            _ => {
                game.update(2.5);
                game.key_lane(0, false);
            }
        }
        game.flick_row_lane(0, false, true);
        assert_eq!(game.note_judgement(0), None, "bad sequence {bad}");
    }
    let mut long_press = one();
    long_press.update(2.20);
    long_press.key_lane(0, true);
    long_press.update(0.30);
    long_press.key_lane(0, false);
    long_press.flick_row_lane(0, true, true);
    assert_eq!(long_press.note_judgement(0), None); // Not a scored long-note contact.
}

#[test]
fn repeats_and_other_outer_row_cannot_reuse_a_consumed_contact() {
    let mut game = Session::new(&score(vec![flick(1, 480), flick(2, 576)]), false, true).unwrap();
    game.update(2.5);
    contact(&mut game, 0);
    game.flick_row_lane(0, false, true);
    game.update(0.1);
    game.flick_row_lane(0, false, true);
    game.flick_row_lane(0, true, true);
    assert_eq!(game.note_judgement(1), None);
    assert_eq!(game.result.counts[0], 1);
    game.flick_row_lane(0, true, false);
    contact(&mut game, 0);
    game.flick_row_lane(0, true, true);
    assert_eq!(game.note_judgement(1), Some(Judge::JustPerfect));
}

#[test]
fn pause_discards_contacts_and_long_note_tail_accepts_a_recent_release() {
    let mut paused = one();
    paused.update(2.5);
    contact(&mut paused, 0);
    paused.pause();
    paused.pause();
    paused.flick_row_lane(0, true, true);
    assert_eq!(paused.note_judgement(0), None);
    let mut hold = Session::new(
        &score(vec![
            Note {
                id: 1,
                ticks: 480,
                category: 1,
                next_connection_id: 2,
                ..Note::default()
            },
            Note {
                previous_connection_id: 1,
                ..flick(2, 1920)
            },
        ]),
        false,
        true,
    )
    .unwrap();
    hold.update(2.5);
    hold.key_lane(0, true);
    hold.update(1.49);
    hold.key_lane(0, false);
    hold.update(0.01);
    hold.flick_row_lane(0, true, true);
    assert_eq!(hold.note_judgement(1), Some(Judge::JustPerfect));
    assert_eq!(hold.result.counts[5], 0);
}

fn live(lane: usize) -> (Live, PathBuf) {
    let data = std::env::temp_dir().join(format!(
        "ojsk-gesture-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&data).unwrap();
    let s = score(vec![Note {
        lane_start: lane as i32,
        lane_end: lane as i32,
        ..flick(1, 480)
    }]);
    fs::write(data.join("score.json"), serde_json::to_vec(&s).unwrap()).unwrap();
    let entry = Entry {
        directory: data.clone(),
        manifest: Manifest::default(),
        modified: SystemTime::now(),
    };
    let mut live = Live::new(Path::new(env!("CARGO_MANIFEST_DIR")), entry, false).unwrap();
    live.event(&AppEvent {
        kind: 16,
        ..AppEvent::default()
    });
    live.session.update(2.5);
    (live, data)
}
fn event(live: &mut Live, key: u32, down: bool) {
    live.event(&AppEvent {
        kind: if down { 5 } else { 12 },
        key,
        ..AppEvent::default()
    });
}

#[test]
fn both_shift_keys_are_game_columns_and_ctrl_is_not_a_binding() {
    assert!(!LOWER_FLICK_KEYS.contains(&1073742052));
    for lane in [0, 11] {
        let (mut live, path) = live(lane);
        event(&mut live, KEYS[lane] as u32, true);
        event(&mut live, KEYS[lane] as u32, false);
        event(&mut live, LOWER_FLICK_KEYS[lane], true);
        assert_eq!(live.session.note_judgement(0), Some(Judge::JustPerfect));
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn space_arrows_and_pointer_without_contact_cannot_bypass_the_gesture() {
    let (mut live, path) = live(0);
    event(&mut live, b'a' as u32, true);
    for key in [32, 1073741903, 1073741904, 1073741906] {
        event(&mut live, key, true);
    }
    live.event(&AppEvent {
        kind: 3,
        x: 240.,
        y: 850.,
        ..AppEvent::default()
    });
    assert_eq!(live.session.note_judgement(0), None);
    event(&mut live, b'a' as u32, false);
    event(&mut live, b'q' as u32, true);
    assert_eq!(live.session.note_judgement(0), Some(Judge::JustPerfect));
    fs::remove_dir_all(path).unwrap();
}
