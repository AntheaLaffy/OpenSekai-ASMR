use opensekai::{
    live::ResultData,
    live_result::{Kind, Sequence, select},
    ui::Painting,
    unity::Bundle,
};
use std::path::Path;

#[test]
fn result_selection_matches_original_csharp_execution() {
    let original: serde_json::Value =
        serde_json::from_str(include_str!("../native/baseline/results.json")).unwrap();
    assert_eq!(original["fade_ease"], "OutQuad");
    for case in original["cases"].as_array().unwrap() {
        let perfect = case["perfect"].as_u64().unwrap() as u32;
        let auto = case["autoCount"].as_u64().unwrap() as u32;
        let result = ResultData {
            counts: [
                perfect / 2,
                perfect - perfect / 2,
                3 - perfect - auto,
                0,
                0,
                0,
                auto,
            ],
            total: 3,
            max_combo: case["maxCombo"].as_u64().unwrap() as u32,
            life: case["life"].as_i64().unwrap() as i32,
            autoplay: case["auto"].as_bool().unwrap(),
            ..ResultData::default()
        };
        let expected = match case["expected"].as_str().unwrap() {
            "None" => Kind::None,
            "LifeZero" => Kind::Finish,
            "Clear" => Kind::Clear,
            "FullCombo" => Kind::FullCombo,
            "AllPerfect" => Kind::AllPerfect,
            s => panic!("{s}"),
        };
        assert_eq!(
            select(&result, case["mode"].as_u64().unwrap() as u32),
            expected,
            "{case}"
        );
    }
}

#[test]
fn finish_animation_event_fires_once_between_presentation_and_result_ui() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let result = ResultData {
        counts: [0, 0, 0, 0, 0, 20, 0],
        total: 20,
        ..ResultData::default()
    };
    assert_eq!(select(&result, 0), Kind::Finish);
    let mut end = Sequence::default();
    assert!(
        end.advance(100., false, &result, &bundle.presentations)
            .is_empty()
    );
    assert!(end.elapsed().is_none());
    assert!(
        end.advance(0., true, &result, &bundle.presentations)
            .is_empty()
    );
    assert!(
        end.advance(1.25, true, &result, &bundle.presentations)
            .is_empty()
    );
    assert_eq!(
        end.advance(0.02, true, &result, &bundle.presentations),
        ["se_live_finish"]
    );
    assert!(
        end.advance(0., true, &result, &bundle.presentations)
            .is_empty()
    );
    assert!(
        end.advance(f32::NAN, true, &result, &bundle.presentations)
            .is_empty()
    );
    let mut paint = Painting::default();
    end.draw(&bundle, &mut paint);
    assert!(paint.draws.iter().any(|d| d.kind == 3));
    assert!(!end.ready());
    assert!(
        end.advance(5.74, true, &result, &bundle.presentations)
            .is_empty()
    );
    assert!(end.ready());
    let mut paint = Painting::default();
    end.draw(&bundle, &mut paint);
    assert!(paint.draws.is_empty());
}

#[test]
fn result_selection_and_source_sprite_clips_cover_manual_and_auto_outcomes() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let result = ResultData {
        counts: [1, 1, 1, 0, 0, 0, 0],
        total: 3,
        max_combo: 3,
        life: 1000,
        ..ResultData::default()
    };
    assert_eq!(select(&result, 0), Kind::FullCombo);
    assert_eq!(
        select(
            &ResultData {
                counts: [2, 1, 0, 0, 0, 0, 0],
                ..result.clone()
            },
            0
        ),
        Kind::AllPerfect
    );
    assert_eq!(
        select(
            &ResultData {
                max_combo: 2,
                ..result.clone()
            },
            0
        ),
        Kind::Clear
    );
    let auto = ResultData {
        counts: [0, 0, 0, 0, 0, 0, 3],
        autoplay: true,
        ..result
    };
    for (mode, expected) in [
        Kind::None,
        Kind::AllPerfect,
        Kind::FullCombo,
        Kind::Clear,
        Kind::Finish,
        Kind::Clear,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(select(&auto, mode as u32), expected);
    }
    for presentation in bundle.presentations.results.values() {
        for frame in 0..=180 {
            let mut paint = Painting::default();
            presentation.draw(&bundle, &mut paint, frame as f32 / 60.);
            for draw in &paint.draws {
                assert!(draw.xy.iter().chain(&draw.uv).all(|v| v.is_finite()));
                if draw.reserved & 4 == 0 {
                    assert!(draw.uv[1] > draw.uv[3]); // Sprite top vertices use v1.
                }
            }
        }
    }
}
