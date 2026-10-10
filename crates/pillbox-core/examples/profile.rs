use pillbox_core::*;
use std::{collections::BTreeMap, time::Instant};
fn main() {
    let mut mode = Mode::builtin("Profile", Objective::None, false);
    mode.score_limit = 1000;
    let mut world = World::new(starter_map(), mode).unwrap();
    for i in 0..16 {
        world.join(format!("p{i:02}"), format!("Pill {i}")).unwrap();
    }
    world.start().unwrap();
    let start = Instant::now();
    for seq in 1..=3600 {
        let inputs: BTreeMap<_, _> = world
            .players
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    Input {
                        seq,
                        x: 0.7,
                        z: -0.5,
                        yaw: seq as f32 * 0.01,
                        fire: true,
                        jump: seq % 120 == 0,
                        reload: seq % 180 == 0,
                        ..Default::default()
                    },
                )
            })
            .collect();
        world.step(&inputs);
    }
    let elapsed = start.elapsed();
    println!(
        "16 players / 3,600 ticks: {:.2}s; {:.3} ms/tick; {:.1}x realtime",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64() * 1000. / 3600.,
        60. / elapsed.as_secs_f64()
    );
}
