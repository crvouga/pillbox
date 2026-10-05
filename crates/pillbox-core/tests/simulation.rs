use pillbox_core::*;
use std::collections::BTreeMap;
fn world(objective: Objective, teams: bool) -> World {
    World::new(starter_map(), Mode::builtin("test", objective, teams)).unwrap()
}
fn two(w: &mut World) {
    w.join("a".into(), "Alice".into()).unwrap();
    w.join("b".into(), "Bob".into()).unwrap();
    w.start().unwrap();
    for _ in 0..180 {
        w.step(&BTreeMap::new());
    }
}
fn input(w: &mut World, id: &str, mut i: Input) {
    i.seq = w.players[id].ack + 1;
    w.step(&BTreeMap::from([(id.into(), i)]));
}
#[test]
fn maps_roundtrip_and_validation() {
    let m = starter_map();
    let json = serde_json::to_string(&m).unwrap();
    let copy: Map = serde_json::from_str(&json).unwrap();
    copy.validate().unwrap();
    assert_eq!(json, serde_json::to_string(&copy).unwrap());
    let mut bad = m.clone();
    bad.objects.push(bad.objects[0].clone());
    assert!(bad.validate().is_err());
    let mut bad = m.clone();
    let spawn = bad.objects.iter_mut().find(|o| o.kind.spawn()).unwrap();
    spawn.position = Vec3::new(-7., 0., -4.);
    assert!(bad.validate().unwrap_err().contains("intersects geometry"));
    let mut bad = m;
    bad.objects[0].scale.x = f32::NAN;
    assert!(bad.validate().is_err());
}
#[test]
fn lifecycle_join_leave_rematch() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    assert_eq!(w.phase, Phase::Playing);
    assert!(w.join("a".into(), "clone".into()).is_err());
    w.leave("b");
    assert_eq!(w.players.len(), 1);
    w.finish();
    w.start().unwrap();
    assert_eq!(w.phase, Phase::Starting);
    assert_eq!(w.team_scores, [0, 0]);
}
#[test]
fn movement_is_bounded_and_bad_input_ignored() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    let old = w.players["a"].position;
    for _ in 0..60 {
        input(
            &mut w,
            "a",
            Input {
                x: 1.,
                ..Default::default()
            },
        );
    }
    assert!(w.players["a"].position.distance(old) < 9.);
    let p = w.players["a"].position;
    input(
        &mut w,
        "a",
        Input {
            x: f32::NAN,
            ..Default::default()
        },
    );
    assert!(w.players["a"].position.is_finite());
    assert!(w.players["a"].position.distance(p) < 0.2);
    for _ in 0..600 {
        input(
            &mut w,
            "a",
            Input {
                x: 1.,
                ..Default::default()
            },
        );
    }
    assert!(w.players["a"].position.x < 22.);
}
#[test]
fn shared_prediction_matches_authority() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    let mut predicted = w.players["a"].clone();
    for n in 1..80 {
        let i = Input {
            seq: n,
            x: 0.5,
            z: -1.,
            jump: n == 10,
            ..Default::default()
        };
        move_player(&w.map, &mut predicted, i, w.tick + 1);
        w.step(&BTreeMap::from([("a".into(), i)]));
        assert!(predicted.position.distance(w.players["a"].position) < 0.0001);
    }
}
#[test]
fn shield_health_respawn_and_scoring() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    w.damage("a", "b", 80., Weapon::Pistol);
    assert_eq!(w.players["b"].shield, 20.);
    assert_eq!(w.players["b"].health, 100.);
    w.damage("a", "b", 130., Weapon::Pistol);
    assert!(!w.players["b"].alive);
    assert_eq!(w.players["a"].score, 1);
    for _ in 0..180 {
        w.step(&BTreeMap::new());
    }
    assert!(w.players["b"].alive);
    assert_eq!(w.players["b"].deaths, 1);
    assert_eq!(w.players["b"].shield, 100.);
    w.damage("b", "b", 300., Weapon::Pistol);
    assert_eq!(w.players["b"].score, 0);
}
#[test]
fn shields_regenerate_after_delay() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    w.damage("a", "b", 50., Weapon::Pistol);
    for _ in 0..299 {
        w.step(&BTreeMap::new());
    }
    assert_eq!(w.players["b"].shield, 50.);
    for _ in 0..130 {
        w.step(&BTreeMap::new());
    }
    assert_eq!(w.players["b"].shield, 100.);
}
#[test]
fn weapons_enforce_ownership_cooldown_ammo_and_reload() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    w.players.get_mut("a").unwrap().position = Vec3::new(0., 0., 5.);
    w.players.get_mut("b").unwrap().position = Vec3::ZERO;
    input(
        &mut w,
        "a",
        Input {
            weapon: Some(Weapon::Sniper),
            fire: true,
            ..Default::default()
        },
    );
    assert_eq!(w.players["a"].weapon, Weapon::Pistol);
    assert_eq!(w.players["a"].ammo(), 11);
    for _ in 0..10 {
        input(
            &mut w,
            "a",
            Input {
                fire: true,
                ..Default::default()
            },
        );
    }
    assert_eq!(w.players["a"].ammo(), 11);
    assert!(w.players["b"].shield < 100.);
    input(
        &mut w,
        "a",
        Input {
            reload: true,
            ..Default::default()
        },
    );
    for _ in 0..80 {
        input(
            &mut w,
            "a",
            Input {
                fire: true,
                ..Default::default()
            },
        );
    }
    assert!(w.players["a"].reserve[0] < 48);
    for weapon in Weapon::ALL {
        assert!(weapon.stats().damage > 0.);
        assert!(weapon.stats().cooldown > 0);
    }
}
#[test]
fn wall_blocks_hitscan() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    w.players.get_mut("a").unwrap().position = Vec3::new(-7., 0., 0.);
    w.players.get_mut("b").unwrap().position = Vec3::new(-7., 0., -8.);
    input(
        &mut w,
        "a",
        Input {
            fire: true,
            ..Default::default()
        },
    );
    assert_eq!(w.players["b"].shield, 100.);
}
#[test]
fn team_deathmatch_and_score_limit() {
    let mut w = world(Objective::None, true);
    w.mode.score_limit = 1;
    two(&mut w);
    assert_ne!(w.players["a"].team, w.players["b"].team);
    w.damage("a", "b", 300., Weapon::Sniper);
    w.step(&BTreeMap::new());
    assert_eq!(w.team_scores[0], 1);
    assert_eq!(w.phase, Phase::Finished);
}
#[test]
fn ctf_pickup_drop_return_capture_reset() {
    let mut w = world(Objective::Flags, true);
    two(&mut w);
    let enemy = w.flags.iter().find(|f| f.team == 1).unwrap().home;
    let home = w.flags.iter().find(|f| f.team == 0).unwrap().home;
    w.players.get_mut("a").unwrap().position = enemy;
    w.step(&BTreeMap::new());
    assert_eq!(w.players["a"].carrying, Some(1));
    w.players.get_mut("a").unwrap().position = home;
    w.step(&BTreeMap::new());
    assert_eq!(w.team_scores[0], 1);
    assert_eq!(w.players["a"].carrying, None);
    w.players.get_mut("a").unwrap().position = enemy;
    w.step(&BTreeMap::new());
    w.damage("b", "a", 300., Weapon::Pistol);
    assert!(w.flags.iter().any(|f| f.return_at > 0));
    w.finish();
    w.start().unwrap();
    assert!(
        w.flags
            .iter()
            .all(|f| f.carrier.is_none() && f.position == f.home && f.return_at == 0)
    );
}
#[test]
fn hill_contested_does_not_score() {
    let mut w = world(Objective::Hill, true);
    two(&mut w);
    w.players.get_mut("a").unwrap().position = Vec3::ZERO;
    w.players.get_mut("b").unwrap().position = Vec3::new(1., 0., 0.);
    for _ in 0..60 {
        w.step(&BTreeMap::new());
    }
    assert!(w.hill_contested);
    assert_eq!(w.team_scores, [0, 0]);
    w.players.get_mut("b").unwrap().position = Vec3::new(15., 0., 0.);
    for _ in 0..60 {
        w.step(&BTreeMap::new());
    }
    assert_eq!(w.hill_owner, Some(0));
    assert_eq!(w.team_scores[0], 1);
}
#[test]
fn booster_and_teleporter() {
    let mut map = starter_map();
    let mut p = Player::new("a".into(), "A".into(), 0, Vec3::new(-14., 0., 0.));
    move_player(&map, &mut p, Input::default(), 1);
    assert!(p.velocity.y > 10.);
    map.objects.push(MapObject {
        id: "exit".into(),
        kind: Primitive::TeleporterExit,
        position: Vec3::new(10., 0.1, 10.),
        rotation: 0.,
        scale: Vec3::ONE,
        properties: Properties::default(),
    });
    map.objects.push(MapObject {
        id: "entry".into(),
        kind: Primitive::TeleporterEntrance,
        position: Vec3::ZERO,
        rotation: 0.,
        scale: Vec3::splat(2.),
        properties: Properties {
            target: Some("exit".into()),
            ..Default::default()
        },
    });
    map.validate().unwrap();
    p.position = Vec3::ZERO;
    p.velocity = Vec3::ZERO;
    move_player(&map, &mut p, Input::default(), 2);
    assert!(p.position.x > 9.);
    assert!(p.teleport_at > 2);
}
#[test]
fn custom_rule_and_validation() {
    let mut w = world(Objective::None, false);
    w.mode.rules[0].effects = vec![Effect::AddScore {
        amount: 5,
        team: false,
    }];
    two(&mut w);
    w.damage("a", "b", 300., Weapon::Pistol);
    assert_eq!(w.players["a"].score, 5);
    w.mode.rules[0].effects = vec![Effect::AddScore {
        amount: 100000,
        team: false,
    }];
    assert!(w.mode.validate().is_err());
}
#[test]
fn jump_and_ramp_collision() {
    let mut map = starter_map();
    map.objects.push(MapObject {
        id: "ramp".into(),
        kind: Primitive::Ramp,
        position: Vec3::new(14., 1., 0.),
        rotation: 0.,
        scale: Vec3::new(4., 2., 8.),
        properties: Properties::default(),
    });
    let mut p = Player::new("a".into(), "A".into(), 0, Vec3::new(14., 0., -6.));
    for t in 1..60 {
        move_player(
            &map,
            &mut p,
            Input {
                z: 1.,
                ..Default::default()
            },
            t,
        );
    }
    assert!(p.position.y > 0.5);
    let y = p.position.y;
    move_player(
        &map,
        &mut p,
        Input {
            jump: true,
            ..Default::default()
        },
        61,
    );
    assert!(p.position.y > y);
}
#[test]
fn time_limit_finishes_match() {
    let mut w = world(Objective::None, false);
    w.mode.time_limit = 10;
    two(&mut w);
    for _ in 0..601 {
        w.step(&BTreeMap::new());
    }
    assert_eq!(w.phase, Phase::Finished);
    assert!(w.events.iter().any(|e| e.kind == "match_over"));
}

#[test]
fn forward_follows_aim_at_ninety_degrees() {
    let map = starter_map();
    let mut p = Player::new("a".into(), "A".into(), 0, Vec3::ZERO);
    for tick in 1..20 {
        move_player(
            &map,
            &mut p,
            Input {
                z: -1.,
                yaw: std::f32::consts::FRAC_PI_2,
                ..Default::default()
            },
            tick,
        );
    }
    assert!(
        p.position.x > 0.5,
        "forward must move toward positive X when aiming right"
    );
    assert!(p.position.z.abs() < 0.1);
}

#[test]
fn all_weapons_damage_and_rockets_travel() {
    for weapon in Weapon::ALL {
        let mut w = world(Objective::None, false);
        two(&mut w);
        let p = w.players.get_mut("a").unwrap();
        p.position = Vec3::new(0., 0., 3.);
        p.inventory = Weapon::ALL.to_vec();
        p.weapon = weapon;
        w.players.get_mut("b").unwrap().position = Vec3::ZERO;
        input(
            &mut w,
            "a",
            Input {
                fire: true,
                ..Default::default()
            },
        );
        if weapon == Weapon::Rocket {
            assert_eq!(w.projectiles.len(), 1);
            for _ in 0..20 {
                w.step(&BTreeMap::new());
            }
            assert!(w.events.iter().any(|e| e.kind == "explosion"));
        }
        assert!(
            w.players["b"].health + w.players["b"].shield < 200.,
            "{weapon:?} should hit"
        );
        assert_eq!(w.players["a"].ammo(), weapon.stats().magazine - 1);
    }
}
#[test]
fn pickups_have_authoritative_cooldowns() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    let mut pickup = w
        .map
        .objects
        .iter()
        .find(|o| o.kind == Primitive::WeaponSpawn)
        .unwrap()
        .clone();
    pickup.position = Vec3::new(0., 0.5, 0.);
    pickup.id = "test-pickup".into();
    pickup.properties.weapon = Weapon::Sniper;
    w.map.objects.push(pickup.clone());
    w.players.get_mut("a").unwrap().position = Vec3::ZERO;
    w.step(&BTreeMap::new());
    assert_eq!(w.players["a"].weapon, Weapon::Sniper);
    assert!(w.pickups["test-pickup"] > w.tick);
    w.players.get_mut("b").unwrap().position = Vec3::ZERO;
    w.step(&BTreeMap::new());
    assert!(!w.players["b"].inventory.contains(&Weapon::Sniper));
    pickup.id = "healing".into();
    pickup.kind = Primitive::HealthPickup;
    w.map.objects.push(pickup);
    w.players.get_mut("a").unwrap().health = 20.;
    w.step(&BTreeMap::new());
    assert_eq!(w.players["a"].health, 100.);
}
#[test]
fn ctf_dropped_flag_returns_and_disconnect_releases_it() {
    let mut w = world(Objective::Flags, true);
    two(&mut w);
    w.players.get_mut("a").unwrap().position = w.flags.iter().find(|f| f.team == 1).unwrap().home;
    w.step(&BTreeMap::new());
    w.players.get_mut("a").unwrap().position = Vec3::ZERO;
    w.step(&BTreeMap::new());
    w.leave("a");
    assert!(
        w.flags
            .iter()
            .any(|f| f.return_at > 0 && f.carrier.is_none())
    );
    for _ in 0..901 {
        w.step(&BTreeMap::new());
    }
    assert!(
        w.flags
            .iter()
            .all(|f| f.position == f.home && f.return_at == 0)
    );
}
#[test]
fn kill_volume_and_zone_rules() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    let mut volume = w
        .map
        .objects
        .iter()
        .find(|o| o.kind == Primitive::Hill)
        .unwrap()
        .clone();
    volume.kind = Primitive::KillVolume;
    volume.id = "void".into();
    w.map.objects.push(volume);
    w.players.get_mut("a").unwrap().position = Vec3::ZERO;
    w.step(&BTreeMap::new());
    assert!(!w.players["a"].alive);
    w.map.objects.retain(|o| o.id != "void");
    w.mode.rules.push(Rule {
        event: EventKind::PlayerEnteredZone,
        conditions: vec![Condition::Alive],
        effects: vec![Effect::AddScore {
            amount: 2,
            team: false,
        }],
    });
    w.players.get_mut("b").unwrap().position = Vec3::ZERO;
    w.step(&BTreeMap::new());
    assert_eq!(w.players["b"].score, 2);
    for _ in 0..10 {
        w.step(&BTreeMap::new());
    }
    assert_eq!(w.players["b"].score, 2, "zone fires only on entry");
}

#[test]
fn minimum_thickness_walls_block_bullets() {
    let mut w = world(Objective::None, false);
    two(&mut w);
    let mut wall = w
        .map
        .objects
        .iter()
        .find(|o| o.kind == Primitive::Wall)
        .unwrap()
        .clone();
    wall.position = Vec3::new(0., 1., 0.061);
    wall.scale = Vec3::new(4., 3., 0.1);
    wall.id = "thin-wall".into();
    w.map.objects.push(wall);
    w.players.get_mut("a").unwrap().position = Vec3::new(0., 0., 3.);
    w.players.get_mut("b").unwrap().position = Vec3::new(0., 0., -3.);
    input(
        &mut w,
        "a",
        Input {
            fire: true,
            ..Default::default()
        },
    );
    assert_eq!(w.players["b"].shield, 100.);
}

#[test]
fn reconnect_preserves_match_state_and_respawn_delay() {
    let mut w = world(Objective::None, true);
    two(&mut w);
    w.damage("a", "b", 210., Weapon::Pistol);
    input(
        &mut w,
        "a",
        Input {
            fire: true,
            ..Default::default()
        },
    );
    w.damage("b", "a", 150., Weapon::Pistol);
    let before = w.players["a"].clone();
    w.leave("a");
    w.join("a".into(), "Alice".into()).unwrap();
    let after = &w.players["a"];
    assert_eq!(after.health, before.health, "reconnect must not heal");
    assert_eq!(after.shield, before.shield);
    assert_eq!(after.score, 1);
    assert_eq!(after.kills, 1);
    assert_eq!(after.team, before.team);
    assert_eq!(after.magazines, before.magazines);
    assert_eq!(after.next_fire, before.next_fire);
    assert_eq!(after.ack, 0, "a new connection starts a new input sequence");
    assert!(
        w.departed.iter().all(|p| p.id != "a"),
        "participation is counted once"
    );
    w.damage("b", "a", 100., Weapon::Pistol);
    let respawn_at = w.players["a"].respawn_at;
    w.leave("a");
    w.join("a".into(), "Alice".into()).unwrap();
    assert!(!w.players["a"].alive, "reconnect must not bypass death");
    assert_eq!(w.players["a"].respawn_at, respawn_at);
    while w.tick + 1 < respawn_at {
        w.step(&BTreeMap::new());
    }
    assert!(!w.players["a"].alive);
    w.step(&BTreeMap::new());
    assert!(w.players["a"].alive);
    assert_eq!(w.players["a"].score, 1);
    assert_eq!(w.players["a"].deaths, 1);
}

#[test]
fn rockets_preserve_friendly_fire_policy_after_owner_disconnects() {
    let mut w = world(Objective::None, true);
    two(&mut w);
    w.join("c".into(), "Cyan teammate".into()).unwrap();
    assert_eq!(w.players["a"].team, w.players["c"].team);
    let shooter = w.players.get_mut("a").unwrap();
    shooter.position = Vec3::new(0., 0., 3.);
    shooter.inventory.push(Weapon::Rocket);
    shooter.weapon = Weapon::Rocket;
    w.players.get_mut("b").unwrap().position = Vec3::new(12., 0., 12.);
    w.players.get_mut("c").unwrap().position = Vec3::ZERO;
    input(
        &mut w,
        "a",
        Input {
            fire: true,
            ..Default::default()
        },
    );
    assert_eq!(w.projectiles.len(), 1);
    w.leave("a");
    for _ in 0..30 {
        w.step(&BTreeMap::new());
    }
    assert!(w.projectiles.is_empty(), "the rocket must have detonated");
    assert_eq!(w.players["c"].health, 100.);
    assert_eq!(
        w.players["c"].shield, 100.,
        "disconnect cannot enable friendly fire"
    );
}

#[test]
fn disconnected_rocket_owner_keeps_kill_credit_and_can_win() {
    let mut w = world(Objective::None, false);
    w.mode.score_limit = 1;
    two(&mut w);
    w.players.get_mut("a").unwrap().position = Vec3::new(0., 0., 3.);
    w.players.get_mut("b").unwrap().position = Vec3::ZERO;
    w.damage("a", "b", 100., Weapon::Pistol);
    let shooter = w.players.get_mut("a").unwrap();
    shooter.inventory.push(Weapon::Rocket);
    shooter.weapon = Weapon::Rocket;
    input(
        &mut w,
        "a",
        Input {
            fire: true,
            ..Default::default()
        },
    );
    w.leave("a");
    for _ in 0..30 {
        w.step(&BTreeMap::new());
    }
    assert!(!w.players["b"].alive);
    let shooter = w.departed.iter().find(|p| p.id == "a").unwrap();
    assert_eq!(shooter.kills, 1);
    assert_eq!(shooter.score, 1);
    assert_eq!(w.phase, Phase::Finished);
}
