use opensekai::{
    ffi::AppEvent,
    maker::{Document, Maker, quantize},
    model::{Note, Score},
    storage,
    ui::Painting,
    unity::{Bundle, Scene, number},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

struct Data(PathBuf);

#[test]
fn long_mesh_matches_original_csharp_vertices_uvs_and_diagonal() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("../native/baseline/long-notes.json")).unwrap();
    for case in oracle["cases"].as_array().unwrap() {
        let mut start: [f32; 4] = serde_json::from_value(case["start"].clone()).unwrap();
        let mut end: [f32; 4] = serde_json::from_value(case["end"].clone()).unwrap();
        let mut uv: [f32; 4] = serde_json::from_value(case["uv"].clone()).unwrap();
        start[1] = -start[1];
        start[3] = -start[3];
        end[1] = -end[1];
        end[3] = -end[3];
        uv[1] = 1. - uv[1];
        uv[3] = 1. - uv[3];
        let quads = opensekai::long_notes::mesh(
            start,
            end,
            case["kind"].as_i64().unwrap() as i32,
            case["index"].as_u64().unwrap() as usize,
            case["count"].as_u64().unwrap() as usize,
            uv,
            [-10000., -10000., 20000., 20000.],
        );
        let expected = case["vertices"].as_array().unwrap();
        assert_eq!(quads.len() * 4, expected.len());
        for (i, quad) in quads.iter().enumerate() {
            let gpu_uv = [
                [quad.uv[0], quad.uv[3]],
                [quad.uv[2], quad.uv[3]],
                [quad.uv[2], quad.uv[1]],
                [quad.uv[0], quad.uv[1]],
            ];
            for (native, source) in [1, 0, 2, 3].into_iter().enumerate() {
                let expected: Vec<f32> =
                    serde_json::from_value(expected[i * 4 + source].clone()).unwrap();
                let actual = [
                    quad.xy[native * 2],
                    -quad.xy[native * 2 + 1],
                    gpu_uv[native][0],
                    1. - gpu_uv[native][1],
                ];
                for (a, b) in actual.into_iter().zip(expected) {
                    assert!(
                        (a - b).abs() < 0.0001,
                        "kind {} quad {i}: {a} != {b}",
                        case["kind"]
                    );
                }
            }
            let source_triangles = &case["triangles"].as_array().unwrap()[i * 6..i * 6 + 6];
            assert_eq!(
                source_triangles
                    .iter()
                    .map(|v| v.as_u64().unwrap() as usize - i * 4)
                    .collect::<Vec<_>>(),
                [0, 1, 2, 1, 2, 3]
            );
        }
    }
}

#[test]
fn connected_note_creation_preserves_original_end_types_and_history() {
    for (category, end_category, end_base) in [(1, 1, 1), (6, 4, 11), (7, 5, 12), (9, 10, 13)] {
        let mut document = Document::new(Score::default());
        document
            .place(
                Note {
                    ticks: 1180,
                    lane_start: 2,
                    lane_end: 5,
                    category,
                    note_base_type: opensekai::maker::note_base_type(category),
                    ..Note::default()
                },
                1200,
            )
            .unwrap();
        let notes = &document.score.note_list;
        assert_eq!(notes.len(), 2);
        assert_eq!((notes[0].ticks, notes[1].ticks), (960, 1200));
        assert_eq!(
            (notes[1].category, notes[1].note_base_type),
            (end_category, end_base)
        );
        assert_eq!(notes[0].next_connection_id, notes[1].id);
        assert_eq!(notes[1].previous_connection_id, notes[0].id);
        assert!(document.score.validate().is_empty());
        document.undo();
        assert!(document.score.note_list.is_empty());
        document.redo();
        assert_eq!(document.score.note_list.len(), 2);
        document.selection.clear();
        document.selection.insert(document.score.note_list[0].id);
        document.delete_selected().unwrap();
        assert!(document.score.note_list.is_empty());
        document.undo();
        assert_eq!(document.score.note_list.len(), 2);
    }
}
impl Data {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("opensekai-maker-{}", storage::short_id().unwrap())))
    }
}
impl Drop for Data {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn source_prefab_anchors_and_original_atlas_coordinates() {
    let bundle = Bundle::load(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut scene = Scene::new(bundle.clone(), &bundle.main).unwrap();
    scene.layout();
    for (name, expected) in [
        ("NotesView", [442., -50., 1064., 1180.]),
        ("BackButton", [16., 16., 96., 96.]),
        ("QuickSaveMusicScoreButton", [1704., 250., 200., 96.]),
    ] {
        let node = &scene.nodes[scene.find(name).unwrap()];
        assert_eq!(node.rect, expected, "{name}");
    }
    let sprite = &bundle.sprites["9b5928c9c930a664c922bc4f0a73e1fe"];
    assert_eq!(sprite.rect, [1930., 357., 24., 24.]);
    assert_eq!(sprite.border, [12.; 4]);
    let mut painting = Painting::default();
    bundle.sprite(
        &mut painting,
        "9b5928c9c930a664c922bc4f0a73e1fe",
        [20., 30., 120., 80.],
        [0., 0., 1920., 1080.],
        0xffffffff,
        true,
        false,
        1.,
        0.,
        false,
    );
    assert_eq!(painting.draws.len(), 9);
    assert_eq!(painting.draws[0].rect, [20., 30., 12., 12.]);
    assert_eq!(painting.draws[4].rect, [32., 42., 96., 56.]);
    assert_eq!(
        painting.draws[0].uv,
        [1930. / 2048., 643. / 1024., 1942. / 2048., 655. / 1024.]
    );
}

#[test]
fn edit_history_preserves_data_and_reconnects_deleted_chain() {
    let mut score = Score::default();
    score
        .extra
        .insert("FutureFormatField".into(), serde_json::json!({"keep":true}));
    score.note_list = (1..=4)
        .map(|id| Note {
            id,
            ticks: id as i64 * 480,
            lane_start: 1,
            lane_end: 2,
            category: 1,
            previous_connection_id: if id == 1 { -1 } else { id - 1 },
            next_connection_id: if id == 4 { -1 } else { id + 1 },
            ..Note::default()
        })
        .collect();
    let mut document = Document::new(score.clone());
    document.selection.extend([2, 3]);
    document.delete_selected().unwrap();
    assert_eq!(
        document
            .score
            .note_list
            .iter()
            .map(|n| n.id)
            .collect::<Vec<_>>(),
        [1, 4]
    );
    assert_eq!(document.score.note_list[0].next_connection_id, 4);
    assert_eq!(document.score.note_list[1].previous_connection_id, 1);
    assert!(document.dirty());
    document.undo();
    assert_eq!(document.score, score);
    assert!(!document.dirty());
    document.redo();
    assert_eq!(document.score.note_list.len(), 2);
    let valid = document.score.clone();
    assert!(
        document
            .change(|s, _| s.note_list[0].lane_end = 12)
            .is_err()
    );
    assert_eq!(document.score, valid);
}

#[test]
fn pointer_edit_undo_redo_and_original_json_save() {
    let data = Data::new();
    let entry = storage::create(&data.0).unwrap();
    let mut maker = Maker::new(Path::new(env!("CARGO_MANIFEST_DIR")), entry.clone()).unwrap();
    let mut paint = Painting::default();
    maker.draw(&mut paint, false);
    let palette = maker
        .scene
        .nodes
        .iter()
        .find(|n| {
            n.component("SelectedNoteDataButton").is_some_and(|c| {
                number(&c.fields, "_noteCategory", -1.) == 0.
                    && number(&c.fields, "_noteTypes", -1.) == 0.
            })
        })
        .unwrap()
        .rect;
    let click = |maker: &mut Maker, x, y| {
        for kind in [1, 2] {
            maker.event(&AppEvent {
                kind,
                key: 1,
                x,
                y,
                ..AppEvent::default()
            });
        }
    };
    click(
        &mut maker,
        palette[0] + palette[2] * 0.5,
        palette[1] + palette[3] * 0.5,
    );
    let y = maker.tick_y(480);
    click(&mut maker, 650., y);
    assert_eq!(maker.document.score.note_list.len(), 1, "{}", maker.status);
    assert_eq!(maker.document.score.note_list[0].ticks, 480);
    assert_eq!(maker.document.score.note_list[0].lane_start, 1);
    assert_eq!(maker.document.score.note_list[0].lane_end, 3);
    assert_eq!(maker.document.score.note_list[0].note_base_type, 1);
    maker.event(&AppEvent {
        kind: 5,
        key: 122,
        modifiers: 1,
        ..AppEvent::default()
    });
    assert!(maker.document.score.note_list.is_empty());
    maker.event(&AppEvent {
        kind: 5,
        key: 121,
        modifiers: 1,
        ..AppEvent::default()
    });
    maker.event(&AppEvent {
        kind: 5,
        key: 115,
        modifiers: 1,
        ..AppEvent::default()
    });
    assert!(!maker.document.dirty());
    assert_eq!(entry.load_score().unwrap(), maker.document.score);
    maker.event(&AppEvent {
        kind: 5,
        key: 27,
        ..AppEvent::default()
    });
    assert!(maker.exit);
}

#[test]
fn original_minimap_pixel_loops_and_time_signatures() {
    use opensekai::{minimap::Minimap, model::Event, timeline::time_signature};
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("../native/baseline/minimap.json")).unwrap();
    for case in oracle["signatures"].as_array().unwrap() {
        assert_eq!(
            time_signature(&case["input"]),
            (
                case["numerator"].as_i64().unwrap() as i32,
                case["denominator"].as_i64().unwrap() as i32
            )
        );
    }
    for case in oracle["maps"].as_array().unwrap() {
        let score = Score {
            note_list: serde_json::from_value(case["notes"].clone()).unwrap(),
            music_score_event_data_list: serde_json::from_value::<Vec<Event>>(
                case["events"].clone(),
            )
            .unwrap(),
            ..Score::default()
        };
        let map = Minimap::new(
            &score,
            case["focus"].as_i64().unwrap(),
            case["maxTicks"].as_i64().unwrap(),
        );
        assert_eq!(
            (map.start, map.end, map.range),
            (
                case["start"].as_i64().unwrap(),
                case["end"].as_i64().unwrap(),
                case["range"].as_i64().unwrap()
            )
        );
        assert_eq!(map.height, case["height"].as_u64().unwrap() as usize);
        let pixels: Vec<_> = case["runs"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|run| {
                std::iter::repeat_n(
                    run[0].as_u64().unwrap() as u32,
                    run[1].as_u64().unwrap() as usize,
                )
            })
            .collect();
        assert_eq!(map.pixels.len(), pixels.len());
        for (index, (actual, expected)) in map.pixels.iter().zip(&pixels).enumerate() {
            assert_eq!(
                actual, expected,
                "focus {} max {} pixel {index}",
                case["focus"], case["maxTicks"]
            );
        }
        let mut paint = Painting::default();
        let rect = [0., 0., 72., map.height as f32];
        map.draw(&mut paint, rect, rect);
        let mut drawn = vec![0; pixels.len()];
        for quad in &paint.draws {
            let [x, y, w, h] = quad.rect.map(|v| v as usize);
            for row in y..y + h {
                for col in x..x + w {
                    drawn[(map.height - 1 - row) * 72 + col] = quad.rgba;
                }
            }
        }
        assert_eq!(
            drawn, pixels,
            "Minimap draw commands must retain every source pixel"
        );
    }
}

#[test]
fn minimap_seeks_and_scroll_stops_at_original_duration() {
    let data = Data::new();
    let mut entry = storage::create(&data.0).unwrap();
    entry.manifest.sec_for_music_score_maker = 30;
    let mut maker = Maker::new(Path::new(env!("CARGO_MANIFEST_DIR")), entry).unwrap();
    let mut paint = Painting::default();
    maker.draw(&mut paint, false);
    let r = maker.minimap_rect().unwrap();
    for kind in [1, 2] {
        maker.event(&AppEvent {
            kind,
            key: 1,
            x: r[0] + r[2] * 0.5,
            y: r[1] + r[3] * 0.5,
            ..AppEvent::default()
        });
    }
    assert_eq!(maker.focus, 14400);
    maker.event(&AppEvent {
        kind: 4,
        x: 1000.,
        delta: 1e6,
        ..AppEvent::default()
    });
    assert_eq!(maker.focus, 28800);
    maker.event(&AppEvent {
        kind: 4,
        x: 1000.,
        delta: -1e6,
        ..AppEvent::default()
    });
    assert_eq!(maker.focus, 0);
    assert!(!maker.document.dirty());
}

#[test]
fn quantization_uses_away_from_zero_not_time_conversion_rounding() {
    assert_eq!(quantize(60, 120), 120);
    assert_eq!(quantize(-60, 120), -120);
    assert_eq!(quantize(1980, 120), 2040);
    assert_eq!(quantize(1919, 120), 1920);
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("../native/baseline/oracle.json")).unwrap();
    for case in oracle["quantization"].as_array().unwrap() {
        assert_eq!(
            quantize(
                case["ticks"].as_i64().unwrap(),
                case["step"].as_i64().unwrap() as i32
            ),
            case["result"].as_i64().unwrap()
        );
    }
    for case in oracle["lanes"].as_array().unwrap() {
        assert_eq!(
            opensekai::maker::lane_range(
                case["center"].as_i64().unwrap() as i32,
                case["width"].as_i64().unwrap() as i32
            ),
            (
                case["start"].as_i64().unwrap() as i32,
                case["end"].as_i64().unwrap() as i32
            )
        );
    }
}
