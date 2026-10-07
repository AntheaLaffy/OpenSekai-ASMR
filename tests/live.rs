use opensekai::{
    live::{Judge, Session},
    model::{Event, Note, Score},
};

fn chart() -> Score {
    Score {
        note_list: vec![
            Note {
                id: 1,
                ticks: 480,
                lane_start: 0,
                lane_end: 1,
                ..Note::default()
            },
            Note {
                id: 2,
                ticks: 960,
                lane_start: 3,
                lane_end: 4,
                category: 1,
                next_connection_id: 3,
                ..Note::default()
            },
            Note {
                id: 3,
                ticks: 1920,
                lane_start: 3,
                lane_end: 4,
                category: 1,
                previous_connection_id: 2,
                ..Note::default()
            },
            Note {
                id: 4,
                ticks: 2400,
                lane_start: 7,
                lane_end: 8,
                category: 3,
                direction: 1,
                ..Note::default()
            },
        ],
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120.),
            ..Event::default()
        }],
        music_score_ticks_max: 2400,
        ..Score::default()
    }
}
#[test]
fn keyboard_long_release_flick_pause_and_result() {
    let mut game = Session::new(&chart(), false, true).unwrap();
    game.update(2.5);
    game.key_lane(0, true);
    game.key_lane(0, true);
    game.key_lane(0, false);
    assert_eq!(game.result.combo, 1); // repeat is not a second press
    game.update(0.5);
    game.key_lane(3, true);
    game.pause();
    let time = game.time;
    game.update(10.);
    assert_eq!(game.time, time);
    game.pause();
    game.key_lane(3, true);
    for _ in 0..3 {
        game.update(0.25);
    }
    game.update(0.25);
    game.key_lane(3, false);
    game.update(0.5);
    game.key_lane(7, true);
    game.flick(1);
    game.update(2.);
    assert!(game.finished);
    assert_eq!(game.result.counts[Judge::Miss as usize], 0);
    assert_eq!(
        game.result.counts[Judge::JustPerfect as usize] as usize,
        game.result.total
    );
    assert_eq!(game.result.max_combo as usize, game.result.total);
    assert!(game.result.practice_without_music);
}
#[test]
fn zero_life_keeps_music_time_and_later_notes_playable() {
    let mut score = chart();
    score.note_list = (1..=20)
        .map(|i| Note {
            id: i,
            ticks: i64::from(i) * 480,
            lane_start: 0,
            lane_end: 1,
            ..Note::default()
        })
        .collect();
    score.music_score_ticks_max = 20 * 480;
    for practice in [false, true] {
        let mut game = Session::new(&score, false, practice).unwrap();
        game.update(9.2); // 14 misses drain life before the later notes.
        assert_eq!(game.result.life, 0);
        assert_eq!(game.result.counts[Judge::Miss as usize], 14);
        assert!(!game.finished);
        game.update(0.3);
        game.key_lane(0, true);
        game.key_lane(0, false);
        assert_eq!(game.result.counts[Judge::JustPerfect as usize], 1);
        assert!(game.result.score > 0);
        assert!(!game.finished);
        game.update(20.);
        assert!(game.finished);
        assert_eq!(game.result.life, 0);
        assert_eq!(game.result.counts[Judge::Miss as usize], 19);
    }
}

#[test]
fn auto_and_manual_outcomes_are_separate() {
    let mut auto = Session::new(&chart(), true, true).unwrap();
    auto.update(10.);
    assert!(auto.finished);
    assert_eq!(
        auto.result.counts[Judge::Auto as usize] as usize,
        auto.result.total
    );
    assert_eq!(auto.result.counts[Judge::JustPerfect as usize], 0);
    let mut manual = Session::new(&chart(), false, true).unwrap();
    manual.update(10.);
    assert!(manual.finished);
    assert_eq!(
        manual.result.counts[Judge::Miss as usize] as usize,
        manual.result.total
    );
    assert_eq!(manual.result.score, 0);
}
#[test]
fn source_hidden_sus_markers_never_count_as_judgements() {
    // Converter.ConvertNormalNote creates HiddenConnectionNote for Skip rows.
    // These occurred as standalone markers in 0433/0524/0545 official charts.
    let mut score = chart();
    for (i, (category, skip)) in [(14, true), (0, true), (13, false)].into_iter().enumerate() {
        score.note_list.push(Note {
            id: 20 + i as i32,
            ticks: 500,
            category,
            is_skip: skip,
            ..Note::default()
        });
    }
    let expected = Session::new(&chart(), true, true).unwrap().result.total;
    let mut game = Session::new(&score, true, true).unwrap();
    game.update(10.);
    assert_eq!(game.result.total, expected);
    assert_eq!(game.result.counts[Judge::Auto as usize] as usize, expected);
}
#[test]
fn original_timing_windows_direction_and_score_weights() {
    for (offset, expected) in [
        (0., Judge::JustPerfect),
        (0.03, Judge::Perfect),
        (0.07, Judge::Great),
        (0.10, Judge::Good),
        (0.12, Judge::Bad),
    ] {
        let mut game = Session::new(&chart(), false, true).unwrap();
        game.update(2.5 + offset);
        game.key_lane(0, true);
        assert_eq!(game.result.counts[expected as usize], 1);
    }
    let mut game = Session::new(&chart(), false, true).unwrap();
    game.update(4.5);
    game.key_lane(7, true);
    game.flick(2);
    assert_eq!(game.last_judge.unwrap().0, Judge::Great);
}
#[test]
fn curved_hold_hit_lanes_follow_elapsed_time_across_bpm_changes() {
    let score = Score {
        note_list: vec![
            Note {
                id: 1,
                category: 1,
                next_connection_id: 2,
                ..Note::default()
            },
            Note {
                id: 2,
                ticks: 1920,
                lane_start: 10,
                lane_end: 10,
                category: 1,
                previous_connection_id: 1,
                ..Note::default()
            },
        ],
        music_score_event_data_list: vec![
            Event {
                change_value: serde_json::json!(120),
                ..Event::default()
            },
            Event {
                ticks: 960,
                change_value: serde_json::json!(240),
                ..Event::default()
            },
        ],
        ..Score::default()
    };
    let mut game = Session::new(&score, false, true).unwrap();
    // At tick 960, time is 1 s of a 1.5 s slide: lane 6.6667, not lane 5.
    game.time = 1.;
    game.key_lane(7, true);
    game.update(0.);
    assert_eq!(game.result.counts[Judge::JustPerfect as usize], 1);
}
