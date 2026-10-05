//! Arenas assembled exclusively from the fixed construction kit.
use crate::*;
struct Builder {
    objects: Vec<MapObject>,
}
impl Builder {
    fn brick(&mut self, piece: BuildingPiece, x: i32, y: i32, z: i32, turn: u8) {
        let mut o = MapObject::brick(
            format!("brick-{}", self.objects.len()),
            piece,
            [x, y, z],
            turn,
        );
        o.properties.color = match piece {
            BuildingPiece::SquareSlab | BuildingPiece::RectangleSlab if y < 0 => "#77848b",
            BuildingPiece::SquareSlab | BuildingPiece::RectangleSlab => "#adb8bc",
            BuildingPiece::ShortRamp | BuildingPiece::LongRamp => "#9ea9ae",
            _ => "#87949c",
        }
        .into();
        self.objects.push(o);
    }
    fn deck(&mut self, x: i32, z: i32, width: i32, depth: i32, level: i32) {
        for a in (x..x + width).step_by(4) {
            for b in (z..z + depth).step_by(8) {
                self.brick(BuildingPiece::RectangleSlab, a, level - 1, b, 0);
            }
        }
    }
    fn column(&mut self, x: i32, z: i32, top: i32) {
        let mut y = 0;
        while y + 4 <= top {
            self.brick(BuildingPiece::SquareBox, x, y, z, 0);
            y += 4;
        }
        while y < top {
            self.brick(BuildingPiece::SquareSlab, x, y, z, 0);
            y += 1;
        }
    }
    fn marker(
        &mut self,
        id: &str,
        kind: Primitive,
        anchor: [i32; 3],
        team: Option<u8>,
        label: &str,
    ) {
        let mut o = MapObject::marker(id, kind, anchor);
        o.properties.team = team;
        o.properties.label = label.into();
        o.properties.color = if team == Some(0) {
            "#62c6d2"
        } else if team == Some(1) {
            "#e69a83"
        } else {
            "#dac99c"
        }
        .into();
        self.objects.push(o);
    }
}
pub fn arena_maps() -> Vec<Map> {
    use BuildingPiece::*;
    ["Terraces", "Crosscut", "Courtyard"]
        .into_iter()
        .enumerate()
        .map(|(variant, name)| {
            let mut b = Builder { objects: vec![] };
            b.deck(-32, -40, 64, 80, 0);
            // The perimeter is structural enclosure, without trim or prop dressing.
            for y in (0..20).step_by(4) {
                for x in [-36, 32] {
                    for z in [-44, 40] {
                        b.brick(SquareBox, x, y, z, 0);
                    }
                }
                for x in (-32..32).step_by(8) {
                    b.brick(RectangleBox, x, y, -44, 1);
                    b.brick(RectangleBox, x, y, 40, 1);
                }
                for z in (-40..40).step_by(8) {
                    b.brick(RectangleBox, -36, y, z, 0);
                    b.brick(RectangleBox, 32, y, z, 0);
                }
            }
            // Opposite side routes have the same four 4 m rises. Each is 8 m wide.
            // Their landings join the decks by complete faces, with 7 m underpasses.
            for (side, x) in [(0, -24), (1, 12)] {
                let reflect = |z: i32, depth: i32| if side == 0 { z } else { -z - depth };
                let turn = if side == 0 { 0 } else { 2 };
                for (z, y) in [(-28, 0), (-16, 4), (8, 8), (20, 12)] {
                    let (piece, start, depth) = if variant == 1 && y == 0 {
                        (LongRamp, -36, 16)
                    } else {
                        (ShortRamp, z, 8)
                    };
                    for lane in [0, 4] {
                        b.brick(piece, x + lane, y, reflect(start, depth), turn);
                    }
                }
                for (z, level) in [(-20, 4), (16, 12)] {
                    let z = reflect(z, 4);
                    for lane in [0, 4] {
                        b.brick(SquareSlab, x + lane, level - 1, z, 0);
                    }
                    // One support leaves half the landing open underneath.
                    b.column(x, z, level - 1);
                }
                b.deck(x, -8, 12, 16, 8);
                for z in [-8, 4] {
                    b.column(x + 8, z, 7);
                }
                let top_z = reflect(28, 8);
                b.deck(x, top_z, 12, 8, 16);
                for offset in [0, 8] {
                    b.column(x + offset, top_z + 4, 15);
                }
                b.marker(
                    &format!("precision-{side}"),
                    Primitive::WeaponSpawn,
                    [x + 4, 8, 0],
                    None,
                    "PRECISION",
                );
                b.objects.last_mut().unwrap().properties.weapon = Weapon::Precision;
                b.marker(
                    &format!("sniper-{side}"),
                    Primitive::WeaponSpawn,
                    [x + 4, 16, top_z],
                    None,
                    "SNIPER",
                );
                b.objects.last_mut().unwrap().properties.weapon = Weapon::Sniper;
                b.marker(
                    &format!("shield-{side}"),
                    Primitive::ShieldPickup,
                    [x + 8, 16, top_z],
                    None,
                    "SHIELD",
                );
            }
            // Different middle routes: one broad crossing, twin crossings, or a ring.
            match variant {
                0 => b.deck(-12, -4, 24, 8, 8),
                1 => {
                    for x in (-12..12).step_by(4) {
                        for z in [-8, 4] {
                            b.brick(SquareSlab, x, 7, z, 0);
                        }
                    }
                }
                _ => {
                    for x in (-12..12).step_by(4) {
                        for z in [-8, 4] {
                            b.brick(SquareSlab, x, 7, z, 0);
                        }
                    }
                    for x in [-4, 0] {
                        b.deck(x, -4, 4, 8, 8);
                    }
                }
            }
            // Only cover that divides a combat lane; no scattered decorative props.
            for (x, z) in [(-8, -24), (4, 16), (-8, 16), (4, -24)] {
                b.brick(RectangleBox, x, 0, z, 0);
            }
            // Low cover on both overlooks; slab stacks retain the same proportions.
            for x in [-24, 20] {
                for y in [8, 9] {
                    b.brick(SquareSlab, x, y, -4, 0);
                }
            }
            for team in 0..2 {
                let z = if team == 0 { -36 } else { 32 };
                for (i, x) in [-4, 0, 4].into_iter().enumerate() {
                    b.marker(
                        &format!("spawn{team}-{i}"),
                        Primitive::TeamSpawn,
                        [x, 0, z],
                        Some(team),
                        "",
                    );
                }
                b.marker(
                    &format!("flag-{team}"),
                    Primitive::Flag,
                    [0, 0, z],
                    Some(team),
                    "FLAG",
                );
                b.marker(
                    &format!("base-{team}"),
                    Primitive::FlagBase,
                    [0, 0, z],
                    Some(team),
                    "BASE",
                );
                b.marker(
                    &format!("health-{team}"),
                    Primitive::HealthPickup,
                    [-16, 0, z],
                    None,
                    "HEALTH",
                );
                b.marker(
                    &format!("rifle-{team}"),
                    Primitive::WeaponSpawn,
                    [8, 0, z],
                    None,
                    "RIFLE",
                );
                b.marker(
                    &format!("shotgun-{team}"),
                    Primitive::WeaponSpawn,
                    [0, 0, if team == 0 { -16 } else { 12 }],
                    None,
                    "SHOTGUN",
                );
                b.objects.last_mut().unwrap().properties.weapon = Weapon::Shotgun;
            }
            // The hill sits on a complete 8 m cell; Crosscut contests the open floor.
            let hill_y = if variant == 1 { 0 } else { 8 };
            b.marker(
                "hill",
                Primitive::Hill,
                [-4, hill_y, -4],
                None,
                "CONTROL POINT",
            );
            b.marker("rocket", Primitive::WeaponSpawn, [0, 0, 0], None, "ROCKET");
            b.objects.last_mut().unwrap().properties.weapon = Weapon::Rocket;
            b.marker("ammo", Primitive::AmmoPickup, [-4, 0, 0], None, "AMMO");
            Map {
                grid: true,
                name: name.into(),
                skybox: "#94a6b1".into(),
                objects: b.objects,
            }
        })
        .collect()
}
