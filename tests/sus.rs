use opensekai::{sus, timeline::Timeline};
use serde_json::Value;

#[test]
fn original_csharp_sus_rows_and_overlays() {
    let cases: Value = serde_json::from_str(include_str!("../native/baseline/sus.json")).unwrap();
    for case in cases.as_array().unwrap() {
        let score = sus::parse(case["text"].as_str().unwrap()).unwrap();
        let mut notes = score.note_list.iter().collect::<Vec<_>>();
        notes.sort_by_key(|n| (n.ticks, n.lane_start, n.category));
        let expected = case["notes"].as_array().unwrap();
        assert_eq!(notes.len(), expected.len());
        for (note, original) in notes.into_iter().zip(expected) {
            let ticks = ((original["bar"].as_f64().unwrap()
                + original["progress"].as_f64().unwrap())
                * 1920.)
                .round() as i64;
            assert_eq!(note.ticks, ticks);
            assert_eq!(note.lane_start as i64, original["lane"].as_i64().unwrap());
            assert_eq!(
                (note.lane_end - note.lane_start + 1) as i64,
                original["width"].as_i64().unwrap()
            );
            assert_eq!(note.category as i64, original["category"].as_i64().unwrap());
            assert_eq!(
                note.direction as i64,
                original["direction"].as_i64().unwrap()
            );
            assert_eq!(
                note.note_line_type as i64,
                original["line"].as_i64().unwrap()
            );
            assert_eq!(note.is_skip, original["skip"].as_bool().unwrap());
            assert!(
                (note.speed_ratio - original["speed"].as_f64().unwrap() as f32).abs() < 0.00001
            );
        }
        assert!(score.validate().is_empty());
    }
}

#[test]
fn variable_bars_bpm_channels_and_invalid_input() {
    let source = "#BPM01:120\n#BPM02:240\n#00008:01\n#00102:3\n#00108:0002\n#00202:4\n#00212:11\n#TIL00: \"1'240:2\"\n#000320:11\n#001330:51\n#002340:21\n";
    let score = sus::parse(source).unwrap();
    assert_eq!(score.music_score_ticks_max, 3360);
    assert_eq!(
        score
            .music_score_event_data_list
            .iter()
            .find(|e| e.event_type == 1)
            .unwrap()
            .ticks,
        2160
    );
    let timeline = Timeline::from_events(&score.music_score_event_data_list).unwrap();
    assert!((timeline.time_at(3360) - 3.125).abs() < 0.00001);
    assert_eq!(
        score
            .note_list
            .iter()
            .filter(|n| n.next_connection_id >= 0)
            .count(),
        2
    );
    assert!(sus::parse("#BPM01:NaN\n").is_err());
    assert!(sus::parse("#00008:02\n").is_err());
    assert!(sus::parse("#00012:z1\n").is_err());
}
