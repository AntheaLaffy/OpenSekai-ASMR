use opensekai::{
    model::{Manifest, Score},
    timeline::Timeline,
};
use serde_json::{Value, json};

#[test]
fn manifest_matches_original_csharp() {
    let oracle: Value =
        serde_json::from_str(include_str!("../native/baseline/oracle.json")).unwrap();
    for case in oracle["manifests"].as_array().unwrap() {
        let mut manifest: Manifest = serde_json::from_value(case["input"].clone()).unwrap();
        manifest.normalize_with_id("baseline0001");
        let actual = serde_json::to_value(&manifest).unwrap();
        let expected = &case["normalized"];
        assert_eq!(
            actual.as_object().unwrap().len(),
            expected.as_object().unwrap().len()
        );
        for (key, value) in expected.as_object().unwrap() {
            if value.is_number() {
                assert_eq!(actual[key].as_f64(), value.as_f64(), "{key}");
            } else {
                assert_eq!(&actual[key], value, "{key}");
            }
        }
        assert_eq!(json!(manifest.music_id()), case["musicId"]);
    }
}

#[test]
fn bpm_math_matches_original_csharp() {
    let oracle: Value =
        serde_json::from_str(include_str!("../native/baseline/oracle.json")).unwrap();
    for case in oracle["timeline"].as_array().unwrap() {
        let points: Vec<_> = case["bpm"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["ticks"].as_i64().unwrap(),
                    p["bpm"].as_f64().unwrap() as f32,
                )
            })
            .collect();
        let timeline = Timeline::from_bpms(&points).unwrap();
        for sample in case["ticks"].as_array().unwrap() {
            let expected = sample["seconds"].as_f64().unwrap() as f32;
            assert_eq!(
                timeline
                    .time_at(sample["ticks"].as_i64().unwrap())
                    .to_bits(),
                expected.to_bits()
            );
        }
        for sample in case["seconds"].as_array().unwrap() {
            assert_eq!(
                timeline.ticks_at(sample["seconds"].as_f64().unwrap() as f32),
                sample["ticks"].as_i64().unwrap()
            );
        }
    }
}

#[test]
fn newtonsoft_references_and_unknown_fields_survive() {
    let text = r#"{"$id":"1","VersionCode":1,"MusicScoreEventDataList":[{"$id":"2","id":7,"ticks":0,"eventType":0,"changeValue":120}],"NoteList":[{"id":1,"ticks":480,"laneStart":0,"laneEnd":2,"nextConnectionId":2,"category":1},{"id":2,"ticks":960,"laneStart":1,"laneEnd":3,"previousConnectionId":1,"category":1}],"EventArray":[{"$type":"Sekai.Live.EventBpm, Assembly-CSharp","value":{"$ref":"2"}}],"FutureField":{"enabled":true}}"#;
    let score = Score::from_json(text).unwrap();
    assert!(score.validate().is_empty());
    assert_eq!(score.event_array[0]["value"]["changeValue"], 120);
    assert_eq!(
        score.event_array[0]["$type"],
        "Sekai.Live.EventBpm, Assembly-CSharp"
    );
    let copy = Score::from_json(&score.to_json().unwrap()).unwrap();
    assert_eq!(copy, score);
    assert_eq!(copy.extra["FutureField"], json!({"enabled":true}));
}

#[test]
fn invalid_connections_and_references_are_reported() {
    assert!(Score::from_json(r#"{"EventArray":[{"$ref":"missing"}]}"#).is_err());
    assert!(Score::from_json(r#"{"EventArray":[{"$id":"a","self":{"$ref":"a"}}]}"#).is_err());
    assert!(Score::from_json(r#"{"VersionCode":99}"#).is_err());
    let score = Score::from_json(
        r#"{"NoteList":[{"id":5,"laneStart":2,"laneEnd":4,"nextConnectionId":99}]}"#,
    )
    .unwrap();
    assert_eq!(score.validate(), ["Note 5 references missing note 99"]);
}

#[test]
fn original_chart_lanes_have_inclusive_endpoints() {
    let score = Score::from_json(r#"{"NoteList":[{"id":1,"laneStart":0,"laneEnd":0},{"id":2,"laneStart":11,"laneEnd":11},{"id":3,"laneStart":0,"laneEnd":11}]}"#).unwrap();
    assert!(score.validate().is_empty());
    for (start, end) in [(-1, 0), (0, 12), (11, 10)] {
        let score = Score::from_json(&format!(
            r#"{{"NoteList":[{{"id":1,"laneStart":{start},"laneEnd":{end}}}]}}"#
        ))
        .unwrap();
        assert_eq!(score.validate().len(), 1, "{start}..={end}");
    }
}
