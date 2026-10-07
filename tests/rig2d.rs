use opensekai::{rig2d::Rig, ui::Painting};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn asset() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/generated/results/rig-v1/rig.json")
}
fn frame(rig: &Rig, action: &str, time: f32) -> Painting {
    let mut p = Painting::default();
    rig.draw(&mut p, action, time, [0., 0., 1024., 1536.], 1.);
    p
}
fn close(a: [f32; 2], b: [f32; 2]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 0.002, "{a} != {b}");
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "ojsk-rig-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        fs::copy(
            asset().with_file_name("atlas-v2.png"),
            path.join("atlas.png"),
        )
        .unwrap();
        Self(path)
    }
    fn load(&self, data: &Value) -> Result<Rig, String> {
        let path = self.0.join("rig.json");
        fs::write(&path, serde_json::to_vec(data).unwrap()).unwrap();
        Rig::load(&path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn hierarchy() -> Value {
    let key = |time, value| {
        json!({"time":time,"value":value,"incoming":0.,"outgoing":0.,
            "in_weight":0.33333334,"out_weight":0.33333334,"step":false,"step_in":false})
    };
    json!({
        "schema":1,"atlas":"atlas.png","atlas_size":[1254,1254],"canvas":[1024,1536],
        "bones":[
            {"name":"shoulder","parent":null,"translation":[100,200],"scale":[2,1]},
            {"name":"elbow","parent":0,"translation":[30,0]},
            {"name":"wrist","parent":1,"translation":[30,0]}
        ],
        "layers":[
            {"name":"upper","bone":0,"source":[1,1,20,20],"rect":[0,-4,30,8]},
            {"name":"lower","bone":1,"source":[25,1,20,20],"rect":[0,-4,30,8]}
        ],
        "clips":{"move":{"duration":2,"tracks":[
            {"bone":0,"property":"rotation","keys":[key(0.,0.),key(2.,90.)]},
            {"bone":1,"property":"rotation","keys":[key(0.,0.),key(2.,30.)]}
        ]}}
    })
}

#[test]
fn elbow_and_wrist_stay_attached_when_parent_rotates_and_scales() {
    let fixture = Fixture::new();
    let rig = fixture.load(&hierarchy()).unwrap();
    close(rig.joint("move", 0., "elbow").unwrap(), [160., 200.]);
    close(rig.joint("move", 2., "elbow").unwrap(), [100., 260.]);
    for step in 0..=120 {
        let time = step as f32 / 60.;
        let p = frame(&rig, "move", time);
        for (d, joint) in p.draws.iter().zip(["elbow", "wrist"]) {
            let end = [(d.xy[2] + d.xy[4]) * 0.5, (d.xy[3] + d.xy[5]) * 0.5];
            close(end, rig.joint("move", time, joint).unwrap());
        }
    }
    assert!(rig.joint("missing", 0., "wrist").is_none());
    assert!(rig.joint("move", 0., "missing").is_none());
}

#[test]
fn generated_character_loops_continuously_and_expression_changes_do_not_move_the_body() {
    let rig = Rig::load(&asset()).unwrap();
    for action in ["idle", "celebrate"] {
        let first = frame(&rig, action, 0.);
        let looped = frame(&rig, action, 6.4);
        assert_eq!(first.draws.len(), 14);
        assert_eq!(first.draws.len(), looped.draws.len());
        for (a, b) in first.draws.iter().zip(&looped.draws) {
            assert_eq!(a.xy, b.xy);
            assert_eq!(a.uv, b.uv);
        }
        let last = frame(&rig, action, 6.4 - 0.001);
        for (a, b) in first.draws.iter().zip(&last.draws) {
            assert!(a.xy.iter().zip(b.xy).all(|(x, y)| (*x - y).abs() < 0.05));
        }
        for time in [f32::NAN, f32::INFINITY, -10., 0., 0.8, 3., 1280.] {
            let p = frame(&rig, action, time);
            assert_eq!(p.draws.len(), 14);
            for d in &p.draws {
                assert_eq!(d.kind, 3);
                assert!(d.xy.iter().chain(&d.rect).all(|v| v.is_finite()));
                assert!(d.uv.iter().all(|v| (0. ..=1.).contains(v)));
                assert_eq!(d.text, rig.texture().as_ptr());
            }
        }
    }
    let open = frame(&rig, "idle", 2.899);
    let closed = frame(&rig, "idle", 2.901);
    assert_ne!(open.draws[12].uv, closed.draws[12].uv);
    for index in 0..12 {
        assert!(
            open.draws[index]
                .xy
                .iter()
                .zip(closed.draws[index].xy)
                .all(|(x, y)| (*x - y).abs() < 0.05)
        );
        assert_eq!(open.draws[index].uv, closed.draws[index].uv);
    }
    assert_ne!(
        frame(&rig, "celebrate", 1.7).draws[13].uv,
        frame(&rig, "celebrate", 1.9).draws[13].uv
    );
    assert_ne!(
        frame(&rig, "celebrate", 0.).draws[4].xy,
        frame(&rig, "celebrate", 1.6).draws[4].xy
    );
}

#[test]
fn malformed_parentage_uvs_clocks_and_paths_are_rejected() {
    let fixture = Fixture::new();
    assert!(fixture.load(&hierarchy()).is_ok());
    let mut bad = hierarchy();
    bad["bones"][0]["parent"] = json!(2);
    assert!(fixture.load(&bad).is_err());
    bad = hierarchy();
    bad["layers"][0]["source"] = json!([1250, 0, 20, 20]);
    assert!(fixture.load(&bad).is_err());
    bad = hierarchy();
    bad["clips"]["move"]["duration"] = json!(0);
    assert!(fixture.load(&bad).is_err());
    bad = hierarchy();
    bad["clips"]["move"]["tracks"][0]["keys"][1]["time"] = json!(0);
    assert!(fixture.load(&bad).is_err());
    bad = hierarchy();
    bad["atlas"] = json!("../atlas.png");
    assert!(fixture.load(&bad).is_err());
}
