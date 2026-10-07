use opensekai::{live::Judge, live_sound::note_cue};

#[test]
fn cue_selection_matches_original_csharp_execution() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../native/baseline/sound.json")).unwrap();
    for row in reference["cases"].as_array().unwrap() {
        let judge = match row["judge"].as_str().unwrap() {
            "JustPerfect" => Judge::JustPerfect,
            "Perfect" => Judge::Perfect,
            "Great" => Judge::Great,
            "Good" => Judge::Good,
            "Bad" => Judge::Bad,
            "Miss" => Judge::Miss,
            "Auto" => Judge::Auto,
            _ => unreachable!(),
        };
        let actual = note_cue(
            row["category"].as_i64().unwrap() as i32,
            row["critical"].as_bool().unwrap(),
            judge,
        );
        assert_eq!(actual.map(|v| v.0), row["cue"].as_str(), "{row}");
        assert_eq!(actual.map(|v| v.1), row["group"].as_str(), "{row}");
    }
}
