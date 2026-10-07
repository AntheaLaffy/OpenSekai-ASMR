use opensekai::{
    live::Session,
    model::{Event, Note, Score},
    scoring::{MAX_SCORE, Score as Points, weight},
};

#[test]
fn same_judgement_gains_grow_and_fractional_points_reach_an_exact_million() {
    for length in [1, 10, 1147, 1151, 10000] {
        let contacts: Vec<_> = (0..length).map(|i| (i as f32, 100)).collect();
        let mut points = Points::new(&contacts);
        let gains: Vec<_> = (0..length)
            .map(|i| points.award(i, 100, i as u32))
            .collect();
        assert_eq!(gains.iter().sum::<u32>(), MAX_SCORE);
        if length > 1 {
            assert!(gains.last().unwrap() > &gains[0]);
            assert!(gains.windows(2).all(|pair| pair[1] + 1 >= pair[0]));
        }
        assert_eq!(points.award(0, 100, length as u32), 0);
    }
}

#[test]
fn note_weights_accuracy_and_chord_key_order_follow_one_rule() {
    assert_eq!(weight(0, false, false), 100);
    assert_eq!(weight(3, false, false), 125);
    assert_eq!(weight(0, true, false), 200);
    assert_eq!(weight(2, false, false), 50);
    assert_eq!(weight(12, false, true), 10);
    let contacts = [(0., 100), (0., 125), (0., 200), (0., 10)];
    for order in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]] {
        let mut score = Points::new(&contacts);
        let mut gains = [0; 4];
        for (combo, index) in order.into_iter().enumerate() {
            gains[index] = score.award(index, 100, combo as u32);
        }
        assert_eq!(gains.iter().sum::<u32>(), MAX_SCORE);
        assert!((i64::from(gains[2]) - 2 * i64::from(gains[0])).abs() <= 2);
        assert!(gains[2] > gains[1] && gains[1] > gains[0] && gains[0] > gains[3]);
    }
    for accuracy in [0, 50, 70, 90, 100] {
        let mut score = Points::new(&contacts);
        let total: u32 = (0..4).map(|i| score.award(i, accuracy, i as u32)).sum();
        assert_eq!(total, MAX_SCORE * accuracy / 100);
    }
}

#[test]
fn a_broken_streak_lowers_later_gains_without_subtracting_earned_points() {
    let contacts: Vec<_> = (0..10).map(|i| (i as f32, 100)).collect();
    let mut perfect = Points::new(&contacts);
    let mut broken = Points::new(&contacts);
    let mut combo = 0;
    let mut total = 0;
    for i in 0..10 {
        let full = perfect.award(i, 100, i as u32);
        let gain = broken.award(i, if i == 5 { 0 } else { 100 }, combo);
        total += gain;
        if i == 5 {
            combo = 0;
            assert_eq!(gain, 0);
        } else {
            combo += 1;
        }
        if i > 5 {
            assert!(gain < full);
        }
    }
    assert!((0..MAX_SCORE).contains(&total));
}

#[test]
fn actual_sessions_share_weights_and_scores_across_auto_frame_rates_and_practice() {
    let notes = vec![
        Note {
            id: 1,
            ticks: 480,
            ..Note::default()
        },
        Note {
            id: 2,
            ticks: 480,
            note_type: 1,
            lane_start: 6,
            lane_end: 6,
            ..Note::default()
        },
        Note {
            id: 3,
            ticks: 960,
            category: 3,
            ..Note::default()
        },
        Note {
            id: 4,
            ticks: 1440,
            category: 1,
            next_connection_id: 5,
            ..Note::default()
        },
        Note {
            id: 5,
            ticks: 2400,
            previous_connection_id: 4,
            ..Note::default()
        },
    ];
    let chart = Score {
        note_list: notes,
        music_score_event_data_list: vec![Event {
            change_value: serde_json::json!(120),
            ..Event::default()
        }],
        music_score_ticks_max: 2400,
        ..Score::default()
    };
    for practice in [false, true] {
        for dt in [1. / 60., 0.2, 10.] {
            let mut game = Session::new(&chart, true, practice).unwrap();
            while !game.finished {
                game.update(dt);
            }
            assert_eq!(game.result.score, MAX_SCORE);
            assert_eq!(game.result.max_combo as usize, game.result.total);
            assert_eq!(game.result.counts[6] as usize, game.result.total);
        }
    }
}
