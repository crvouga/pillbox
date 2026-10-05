use pillbox_core::*;
use std::collections::BTreeMap;
fn world(objective: Objective, teams: bool) -> World {
    World::new(test_map(), Mode::builtin("test", objective, teams)).unwrap()
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
    let m = test_map();
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
    let mut map = test_map();
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
    let mut map = test_map();
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
    let map = test_map();
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

fn test_map() -> Map {
    let mut objects = vec![];
    let mut add = |id: &str, kind, p, s, team, color: &str| {
        objects.push(MapObject {
            id: id.into(),
            kind,
            position: p,
            rotation: 0.,
            scale: s,
            properties: Properties {
                team,
                color: color.into(),
                ..Default::default()
            },
        })
    };
    add(
        "ground",
        Primitive::Floor,
        Vec3::new(0., -1., 0.),
        Vec3::new(44., 2., 44.),
        None,
        "#28344f",
    );
    for (id, x, z, sx, sz) in [
        ("north", 0., -22., 44., 1.),
        ("south", 0., 22., 44., 1.),
        ("west", -22., 0., 1., 44.),
        ("east", 22., 0., 1., 44.),
    ] {
        add(
            id,
            Primitive::Wall,
            Vec3::new(x, 2., z),
            Vec3::new(sx, 4., sz),
            None,
            "#536493",
        );
    }
    for (i, x, z) in [(0, -7., -4.), (1, 7., 4.), (2, 7., -8.), (3, -7., 8.)] {
        add(
            &format!("cover{i}"),
            Primitive::Box,
            Vec3::new(x, 1.5, z),
            Vec3::new(3., 3., 3.),
            None,
            "#9a79da",
        );
    }
    for t in 0..2 {
        let z = if t == 0 { -15. } else { 15. };
        let color = if t == 0 { "#54e3f0" } else { "#ff8b72" };
        for (i, x) in [-4., 4.].into_iter().enumerate() {
            add(
                &format!("spawn{t}-{i}"),
                Primitive::TeamSpawn,
                Vec3::new(x, 0.05, z),
                Vec3::ONE,
                Some(t),
                color,
            );
        }
        add(
            &format!("base{t}"),
            Primitive::FlagBase,
            Vec3::new(0., 0.1, z + if t == 0 { -3. } else { 3. }),
            Vec3::new(3., 2., 3.),
            Some(t),
            color,
        );
    }
    add(
        "hill",
        Primitive::Hill,
        Vec3::new(0., 1., 0.),
        Vec3::new(7., 3., 7.),
        None,
        "#ffe599",
    );
    add(
        "jump",
        Primitive::JumpPad,
        Vec3::new(-14., 0.15, 0.),
        Vec3::new(3., 0.3, 3.),
        None,
        "#92ffb5",
    );
    add(
        "rifle",
        Primitive::WeaponSpawn,
        Vec3::new(5., 0.5, 0.),
        Vec3::ONE,
        None,
        "#ffe599",
    );
    Map {
        name: "Prism Courtyard".into(),
        skybox: "#141c35".into(),
        objects,
    }
}

#[test]
fn curated_arenas_have_valid_spawns_vertical_routes_cover_and_lighting() {
    for map in arena_maps() {
        map.validate().unwrap();
        assert!(map.objects.len() > 75);
        assert!(
            map.objects
                .iter()
                .filter(|o| o.kind == Primitive::Light)
                .count()
                >= 6
        );
        assert!(
            map.objects
                .iter()
                .filter(|o| o.kind == Primitive::Ramp)
                .count()
                >= 8
        );
        assert!(map.objects.iter().any(|o| o.kind == Primitive::Stairs));
        for mode in [
            Mode::builtin("FFA", Objective::None, false),
            Mode::builtin("CTF", Objective::Flags, true),
            Mode::builtin("Hill", Objective::Hill, true),
        ] {
            map.compatible(&mode).unwrap();
        }
        let physics = PhysicsMap::new(&map);
        for spawn in map.objects.iter().filter(|o| o.kind.spawn()) {
            let mut p = Player::new("route".into(), "Route".into(), 0, spawn.position);
            for tick in 0..120 {
                move_player_with_physics(&map, &physics, &mut p, Input::default(), tick);
            }
            assert!(
                (p.position.y - 3.).abs() < 0.02,
                "{} spawn {} not supported {:?}",
                map.name,
                spawn.id,
                p.position
            );
            assert!(p.grounded);
        }
    }
}
#[test]
fn box3d_capsule_slides_on_rotated_walls_and_cannot_tunnel() {
    let mut map = test_map();
    map.objects.retain(|o| o.kind == Primitive::Floor);
    map.objects.push(MapObject {
        id: "thin diagonal".into(),
        kind: Primitive::Wall,
        position: Vec3::new(0., 2., 0.),
        rotation: 0.6,
        scale: Vec3::new(0.1, 4., 100.),
        properties: Properties::default(),
    });
    let physics = PhysicsMap::new(&map);
    let mut p = Player::new("a".into(), "A".into(), 0, Vec3::new(-5., 0., 0.));
    for tick in 0..180 {
        move_player_with_physics(
            &map,
            &physics,
            &mut p,
            Input {
                x: 1.,
                z: 0.4,
                ..Default::default()
            },
            tick,
        );
        assert!(physics.clear(p.position));
        let normal_x = 0.6_f32.cos() * p.position.x - 0.6_f32.sin() * p.position.z;
        assert!(normal_x < 0., "crossed wall");
    }
    assert!(p.position.x < 10.);
    assert!(p.position.z.abs() > 4., "must slide along the wall");
}
#[test]
fn box3d_climbs_steps_and_follows_downhill_without_falling_through() {
    let mut map = test_map();
    map.objects.retain(|o| o.kind == Primitive::Floor);
    map.objects.push(MapObject {
        id: "steps".into(),
        kind: Primitive::Stairs,
        position: Vec3::new(0., 1., 0.),
        rotation: 0.,
        scale: Vec3::new(4., 2., 8.),
        properties: Properties::default(),
    });
    let physics = PhysicsMap::new(&map);
    let mut p = Player::new("a".into(), "A".into(), 0, Vec3::new(0., 0., -5.));
    let mut max_y = 0_f32;
    for tick in 0..120 {
        move_player_with_physics(
            &map,
            &physics,
            &mut p,
            Input {
                z: 1.,
                ..Default::default()
            },
            tick,
        );
        max_y = max_y.max(p.position.y);
    }
    assert!(max_y > 1.8, "stairs blocked at {:?}", p.position);
    for tick in 120..240 {
        move_player_with_physics(
            &map,
            &physics,
            &mut p,
            Input {
                z: -1.,
                ..Default::default()
            },
            tick,
        );
    }
    assert!(p.position.y.abs() < 0.02);
    assert!(p.grounded);
}
#[test]
fn players_can_walk_curated_ramps_and_stairs_to_all_three_levels() {
    for (variant, map) in arena_maps().into_iter().enumerate() {
        let physics = PhysicsMap::new(&map);
        let angle = variant as f32 * std::f32::consts::FRAC_PI_2;
        let (s, c) = angle.sin_cos();
        let rotate = |v: Vec3| Vec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
        for (route, start, end) in [
            ("base", Vec3::new(8., 0.05, -9.), Vec3::new(8., 3., -25.)),
            ("reactor", Vec3::new(0., 0.05, -17.), Vec3::new(0., 3., -4.)),
            (
                "skywalk",
                Vec3::new(29., 0.05, -7.),
                Vec3::new(29., 6., -25.),
            ),
            (
                "overlook stairs",
                Vec3::new(10., 3.05, -8.),
                Vec3::new(10., 6., 7.),
            ),
        ] {
            let start = rotate(start);
            let end = rotate(end);
            let mut player = Player::new("climber".into(), "Climber".into(), 0, start);
            for tick in 0..240 {
                let delta = end - player.position;
                if Vec3::new(delta.x, 0., delta.z).length() < 0.2 {
                    break;
                }
                let direction = Vec3::new(delta.x, 0., delta.z).normalize();
                move_player_with_physics(
                    &map,
                    &physics,
                    &mut player,
                    Input {
                        x: direction.x,
                        z: direction.z,
                        ..Default::default()
                    },
                    tick,
                );
            }
            assert!(
                player.position.distance(end) < 0.3,
                "{} {route} blocked at {:?}, expected {end:?}; grounded {} velocity {:?}",
                map.name,
                player.position,
                player.grounded,
                player.velocity
            );
            assert!(player.grounded, "{} {route} unsupported", map.name);
        }
    }
}
#[test]
fn opponents_can_route_between_bases_and_reach_every_power_weapon() {
    for map in arena_maps() {
        let w = World::new(map.clone(), Mode::builtin("nav", Objective::None, false)).unwrap();
        let start = map
            .objects
            .iter()
            .find(|o| o.id == "spawn0-0")
            .unwrap()
            .position;
        for o in map
            .objects
            .iter()
            .filter(|o| o.kind == Primitive::WeaponSpawn || o.id == "spawn1-0")
        {
            assert!(
                !w.navigation.route(start, o.position).is_empty(),
                "{} unreachable {} {:?}",
                map.name,
                o.id,
                o.position
            );
        }
    }
}
#[test]
fn dense_valid_creator_geometry_cannot_build_an_unbounded_bot_graph() {
    let mut map = test_map();
    map.objects
        .retain(|o| o.kind == Primitive::Floor || o.kind.spawn());
    for level in 1..50 {
        map.objects.push(MapObject {
            id: format!("stack-{level}"),
            kind: Primitive::Platform,
            position: Vec3::new(0., level as f32 * 3., 0.),
            rotation: 0.,
            scale: Vec3::new(64., 0.5, 64.),
            properties: Properties::default(),
        });
    }
    map.validate().unwrap();
    let navigation = Navigation::new(&map, &PhysicsMap::new(&map));
    assert!(!navigation.nodes.is_empty());
    assert!(navigation.nodes.len() <= 2048);
}
#[test]
fn practice_opponents_move_fire_reload_and_score_through_authoritative_rules() {
    let mut w = World::new(
        starter_map(),
        Mode::builtin("practice", Objective::None, false),
    )
    .unwrap();
    let mut brains = BTreeMap::new();
    for i in 0..4 {
        let id = format!("bot:{i}");
        w.join(id.clone(), format!("Bot {i}")).unwrap();
        brains.insert(id, BotBrain::default());
    }
    w.start().unwrap();
    let mut shots = 0;
    let mut kills = 0;
    for _ in 0..6000 {
        let inputs = w.bot_inputs(&mut brains);
        w.step(&inputs);
        for e in w.events.iter().filter(|e| e.tick == w.tick) {
            shots += usize::from(e.kind == "shot");
            kills += usize::from(e.kind == "player_killed");
        }
    }
    assert!(shots > 40, "bots failed to engage: {shots}");
    assert!(kills > 2, "bots failed to complete combat: {kills}");
    for p in w.players.values() {
        assert!(p.position.is_finite());
        assert!(p.position.y > -2., "bot fell out {:?}", p.position);
    }
}
