//! GPU capture uses the exact client scene, assets and systems, without a window.
use pillbox_core::*;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let path = args.get(1).expect("output PNG path");
    let map = arena_maps().remove(args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0));
    pillbox_client::set_map(&serde_json::to_string(&map).unwrap()).unwrap();
    let mut w = World::new(map, Mode::builtin("Capture", Objective::None, false)).unwrap();
    w.join("capture".into(), "Capture".into()).unwrap();
    w.join("opponent".into(), "Vector".into()).unwrap();
    let p = w.players.get_mut("capture").unwrap();
    p.position = Vec3::new(0., 3., -4.);
    p.yaw = std::f32::consts::PI;
    p.weapon = args.get(4).map_or(Weapon::Rifle, |name| {
        serde_json::from_value(serde_json::Value::String(name.clone())).expect("weapon name")
    });
    let me = w.players["capture"].clone();
    let (camera, target) = if args.get(2).is_some_and(|s| s == "overview") {
        (Some([38., 29., 38.]), Some([0., 2., 0.]))
    } else {
        (None, None)
    };
    pillbox_client::set_frame(&serde_json::json!({"state":w.snapshot(),"local_id":"capture","predicted":me,"camera":camera,"target":target}).to_string()).unwrap();
    pillbox_client::capture(path);
}
