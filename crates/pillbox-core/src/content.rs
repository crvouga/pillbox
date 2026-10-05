use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const TICK_HZ: u32 = 60;
pub const DT: f32 = 1.0 / TICK_HZ as f32;
pub const RADIUS: f32 = 0.4;
pub const HEIGHT: f32 = 1.8;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Primitive {
    Box,
    Wall,
    Floor,
    Platform,
    Ramp,
    Wedge,
    Arch,
    Pillar,
    Cylinder,
    Sphere,
    Stairs,
    Bridge,
    Barrier,
    PlayerSpawn,
    TeamSpawn,
    KillVolume,
    JumpPad,
    TeleporterEntrance,
    TeleporterExit,
    SpeedBoost,
    GravityModifier,
    HealthPickup,
    ShieldPickup,
    WeaponSpawn,
    AmmoPickup,
    Flag,
    FlagBase,
    ScoringZone,
    Hill,
    Light,
}
impl Primitive {
    pub fn solid(self) -> bool {
        matches!(
            self,
            Self::Box
                | Self::Wall
                | Self::Floor
                | Self::Platform
                | Self::Ramp
                | Self::Wedge
                | Self::Arch
                | Self::Pillar
                | Self::Cylinder
                | Self::Sphere
                | Self::Stairs
                | Self::Bridge
                | Self::Barrier
        )
    }
    pub fn spawn(self) -> bool {
        matches!(self, Self::PlayerSpawn | Self::TeamSpawn)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Weapon {
    #[default]
    Pistol,
    Rifle,
    Precision,
    Shotgun,
    Sniper,
    Rocket,
}
impl Weapon {
    pub const ALL: [Self; 6] = [
        Self::Pistol,
        Self::Rifle,
        Self::Precision,
        Self::Shotgun,
        Self::Sniper,
        Self::Rocket,
    ];
    pub fn stats(self) -> WeaponStats {
        match self {
            Self::Pistol => WeaponStats {
                damage: 28.,
                cooldown: 18,
                magazine: 12,
                reload: 80,
                spread: 0.005,
                pellets: 1,
                speed: 0.,
                splash: 0.,
            },
            Self::Rifle => WeaponStats {
                damage: 14.,
                cooldown: 6,
                magazine: 32,
                reload: 100,
                spread: 0.018,
                pellets: 1,
                speed: 0.,
                splash: 0.,
            },
            Self::Precision => WeaponStats {
                damage: 45.,
                cooldown: 28,
                magazine: 10,
                reload: 110,
                spread: 0.001,
                pellets: 1,
                speed: 0.,
                splash: 0.,
            },
            Self::Shotgun => WeaponStats {
                damage: 17.,
                cooldown: 55,
                magazine: 6,
                reload: 130,
                spread: 0.12,
                pellets: 10,
                speed: 0.,
                splash: 0.,
            },
            Self::Sniper => WeaponStats {
                damage: 160.,
                cooldown: 80,
                magazine: 4,
                reload: 150,
                spread: 0.,
                pellets: 1,
                speed: 0.,
                splash: 0.,
            },
            Self::Rocket => WeaponStats {
                damage: 180.,
                cooldown: 70,
                magazine: 2,
                reload: 150,
                spread: 0.,
                pellets: 1,
                speed: 25.,
                splash: 5.,
            },
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct WeaponStats {
    pub damage: f32,
    pub cooldown: u64,
    pub magazine: u16,
    pub reload: u64,
    pub spread: f32,
    pub pellets: u8,
    pub speed: f32,
    pub splash: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Properties {
    pub team: Option<u8>,
    pub target: Option<String>,
    pub weapon: Weapon,
    pub strength: f32,
    pub respawn_seconds: u16,
    pub color: String,
    pub label: String,
    /// Optional licensed station model, scaled to match this object collider.
    pub asset: Option<String>,
}
impl Default for Properties {
    fn default() -> Self {
        Self {
            team: None,
            target: None,
            weapon: Weapon::Rifle,
            strength: 14.,
            respawn_seconds: 15,
            color: "#8263ed".into(),
            label: String::new(),
            asset: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapObject {
    pub id: String,
    pub kind: Primitive,
    pub position: Vec3,
    /// Yaw in radians. Rotation is deliberately restricted to yaw so collision and rendering agree.
    pub rotation: f32,
    pub scale: Vec3,
    #[serde(default)]
    pub properties: Properties,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map {
    pub name: String,
    pub skybox: String,
    pub objects: Vec<MapObject>,
}
fn valid_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn valid_name(s: &str) -> bool {
    !s.trim().is_empty() && s.chars().count() <= 64 && !s.chars().any(char::is_control)
}
impl Map {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_name(&self.name) || !valid_color(&self.skybox) || self.objects.len() > 512 {
            return Err("Invalid map name, skybox, or object count (maximum 512)".into());
        }
        let mut ids = BTreeSet::new();
        for o in &self.objects {
            if !valid_name(&o.id)
                || !ids.insert(&o.id)
                || !o.position.is_finite()
                || o.position.abs().max_element() > 250.
                || !o.scale.is_finite()
                || o.scale.min_element() < 0.1
                || o.scale.max_element() > 150.
                || !o.rotation.is_finite()
                || o.rotation.abs() > 100.
                || !o.properties.strength.is_finite()
                || !(0.1..=40.).contains(&o.properties.strength)
                || o.properties.team.is_some_and(|t| t > 1)
                || !(1..=300).contains(&o.properties.respawn_seconds)
                || (!o.properties.label.is_empty() && !valid_name(&o.properties.label))
                || o.properties.asset.as_deref().is_some_and(|a| {
                    !matches!(
                        a,
                        "container"
                            | "container-wide"
                            | "container-tall"
                            | "computer"
                            | "computer-wide"
                            | "pipe"
                            | "pipe-bend"
                            | "structure-panel"
                    )
                })
                || !valid_color(&o.properties.color)
            {
                return Err(format!("Invalid object {}", o.id));
            }
            if o.kind == Primitive::TeleporterEntrance
                && !self.objects.iter().any(|x| {
                    Some(&x.id) == o.properties.target.as_ref()
                        && x.kind == Primitive::TeleporterExit
                })
            {
                return Err(format!("Teleporter {} needs an exit target", o.id));
            }
            if matches!(
                o.kind,
                Primitive::TeamSpawn | Primitive::Flag | Primitive::FlagBase
            ) && o.properties.team.is_none()
            {
                return Err("Team spawns and flags require a team".into());
            }
        }
        let spawns: Vec<_> = self.objects.iter().filter(|o| o.kind.spawn()).collect();
        if spawns.len() < 2 {
            return Err("Place at least two player/team spawns".into());
        }
        for s in spawns {
            if !crate::position_clear(self, s.position) {
                return Err(format!("Spawn {} intersects geometry", s.id));
            }
        }
        Ok(())
    }
    pub fn compatible(&self, mode: &Mode) -> Result<(), String> {
        if mode.teams
            && (0..2).any(|t| {
                !self.objects.iter().any(|o| {
                    o.kind.spawn() && (o.properties.team.is_none() || o.properties.team == Some(t))
                })
            })
        {
            return Err("Each team needs a usable spawn".into());
        }
        if mode.objective == Objective::Flags {
            for t in 0..2 {
                if self
                    .objects
                    .iter()
                    .filter(|o| o.kind == Primitive::Flag && o.properties.team == Some(t))
                    .count()
                    > 1
                {
                    return Err("At most one flag spawn per team".into());
                }
                if self
                    .objects
                    .iter()
                    .filter(|o| o.kind == Primitive::FlagBase && o.properties.team == Some(t))
                    .count()
                    != 1
                {
                    return Err("CTF requires exactly one flag base for each team".into());
                }
            }
        }
        if mode.objective == Objective::Hill
            && !self.objects.iter().any(|o| o.kind == Primitive::Hill)
        {
            return Err("Place a hill for this mode".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    None,
    Flags,
    Hill,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    MatchStarted,
    PlayerSpawned,
    PlayerKilled,
    PlayerEnteredZone,
    PlayerLeftZone,
    PlayerPickedUpItem,
    PlayerDroppedItem,
    TimerElapsed,
    ScoreChanged,
    FlagCaptured,
    HillHeld,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Condition {
    Opponent,
    Alive,
    CarryingFlag,
    Team { team: u8 },
    ScoreAtLeast { value: i32 },
    TimeAtLeast { seconds: u32 },
    ZoneOwned,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    AddScore { amount: i32, team: bool },
    EndMatch,
    Respawn,
    GiveWeapon { weapon: Weapon },
    RemoveItem,
    ResetFlags,
    Announce { text: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub event: EventKind,
    pub conditions: Vec<Condition>,
    pub effects: Vec<Effect>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    pub name: String,
    pub teams: bool,
    pub objective: Objective,
    pub score_limit: i32,
    pub time_limit: u32,
    pub respawn_seconds: u16,
    pub friendly_fire: bool,
    pub rules: Vec<Rule>,
}
impl Mode {
    pub fn validate(&self) -> Result<(), String> {
        if !valid_name(&self.name)
            || !(1..=1000).contains(&self.score_limit)
            || !(10..=3600).contains(&self.time_limit)
            || !(1..=30).contains(&self.respawn_seconds)
            || self.rules.len() > 32
            || (self.objective != Objective::None && !self.teams)
        {
            return Err("Invalid mode limits or objective/team combination".into());
        }
        for r in &self.rules {
            if r.conditions.len() > 8 || r.effects.is_empty() || r.effects.len() > 8 {
                return Err("Rules need 1–8 effects and at most 8 conditions".into());
            }
            for c in &r.conditions {
                if matches!(c, Condition::Team {team} if *team > 1)
                    || matches!(c, Condition::ScoreAtLeast { value } if !(0..=1000).contains(value))
                    || matches!(c, Condition::TimeAtLeast { seconds } if *seconds > 3600)
                {
                    return Err("Invalid condition".into());
                }
            }
            for e in &r.effects {
                match e {
                    Effect::AddScore { amount, .. } if !(-100..=100).contains(amount) => {
                        return Err("Score effect out of range".into());
                    }
                    Effect::Announce { text } if !valid_name(text) => {
                        return Err("Invalid announcement".into());
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    }
    pub fn builtin(name: &str, objective: Objective, teams: bool) -> Self {
        let event = match objective {
            Objective::None => EventKind::PlayerKilled,
            Objective::Flags => EventKind::FlagCaptured,
            Objective::Hill => EventKind::HillHeld,
        };
        Self {
            name: name.into(),
            teams,
            objective,
            score_limit: if objective == Objective::Flags { 3 } else { 25 },
            time_limit: 600,
            respawn_seconds: 3,
            friendly_fire: false,
            rules: vec![Rule {
                event,
                conditions: if objective == Objective::None {
                    vec![Condition::Opponent]
                } else {
                    vec![]
                },
                effects: vec![Effect::AddScore {
                    amount: 1,
                    team: teams,
                }],
            }],
        }
    }
}
/// Curated vertical arenas use exactly the primitives available in the editor.
pub fn starter_map() -> Map {
    arena_maps().remove(0)
}
pub fn arena_maps() -> Vec<Map> {
    [
        ("Foundry Nine", "#101c2c", "#54dfdf", "#ee9970"),
        ("Helix Station", "#1a1734", "#ba99ff", "#ffbd69"),
        ("Blacksite Relay", "#0d2427", "#82dcbc", "#ef7796"),
    ]
    .into_iter()
    .enumerate()
    .map(|(variant, (name, sky, accent, warm))| {
        let mut objects = vec![];
        let mut add = |id: &str,
                       kind: Primitive,
                       p: [f32; 3],
                       s: [f32; 3],
                       yaw: f32,
                       team: Option<u8>,
                       color: &str,
                       label: &str,
                       asset: Option<&str>| {
            objects.push(MapObject {
                id: id.into(),
                kind,
                position: Vec3::from_array(p),
                rotation: yaw,
                scale: Vec3::from_array(s),
                properties: Properties {
                    team,
                    color: color.into(),
                    label: label.into(),
                    asset: asset.map(str::to_owned),
                    ..Default::default()
                },
            });
        };
        add(
            "ground",
            Primitive::Floor,
            [0., -0.5, 0.],
            [64., 1., 64.],
            0.,
            None,
            "#344451",
            "LOWER CONCOURSE",
            None,
        );
        for (id, x, z, sx, sz) in [
            ("north", 0., -32., 64., 1.),
            ("south", 0., 32., 64., 1.),
            ("west", -32., 0., 1., 64.),
            ("east", 32., 0., 1., 64.),
        ] {
            add(
                id,
                Primitive::Wall,
                [x, 5., z],
                [sx, 10., sz],
                0.,
                None,
                "#263440",
                "",
                None,
            );
        }
        // Three-level central control point with four approaches and covered underpass.
        add(
            "reactor-deck",
            Primitive::Platform,
            [0., 2.75, 0.],
            [12., 0.5, 12.],
            0.,
            None,
            "#536675",
            "REACTOR DECK",
            None,
        );
        add(
            "hill",
            Primitive::Hill,
            [0., 3.5, 0.],
            [6., 1., 6.],
            0.,
            None,
            "#ffc879",
            "CONTROL POINT",
            None,
        );
        for (i, x, z) in [(0, -5., -5.), (1, 5., -5.), (2, -5., 5.), (3, 5., 5.)] {
            add(
                &format!("reactor-leg-{i}"),
                Primitive::Pillar,
                [x, 1.25, z],
                [1., 2.5, 1.],
                0.,
                None,
                "#263440",
                "",
                None,
            );
        }
        for (t, z, dir) in [(0, -25., 1.), (1, 25., -1.)] {
            let c = if t == 0 { accent } else { warm };
            let yaw = if t == 0 { 0. } else { std::f32::consts::PI };
            add(
                &format!("base-deck-{t}"),
                Primitive::Platform,
                [0., 2.75, z],
                [24., 0.5, 10.],
                0.,
                None,
                "#536675",
                "BASE DECK",
                None,
            );
            for (i, x) in [-8., 0., 8.].into_iter().enumerate() {
                add(
                    &format!("spawn{t}-{i}"),
                    Primitive::TeamSpawn,
                    [x, 3.05, z],
                    [1., 1., 1.],
                    0.,
                    Some(t),
                    c,
                    "SPAWN",
                    None,
                );
            }
            add(
                &format!("base{t}"),
                Primitive::FlagBase,
                [0., 3.1, z - dir * 3.],
                [3., 2., 3.],
                0.,
                Some(t),
                c,
                if t == 0 { "CYAN BASE" } else { "CORAL BASE" },
                None,
            );
            // Spawn rooms remain open toward the arena and through two flank doors.
            for x in [-12., 12.] {
                add(
                    &format!("base-wall-{t}-{x}"),
                    Primitive::Wall,
                    [x, 4.5, z],
                    [0.5, 3., 10.],
                    0.,
                    None,
                    "#384b5c",
                    "",
                    None,
                );
            }
            add(
                &format!("base-roof-{t}"),
                Primitive::Platform,
                [0., 5.75, z],
                [24., 0.5, 10.],
                0.,
                None,
                "#4e6170",
                "COMMAND ROOF",
                None,
            );
            add(
                &format!("base-front-{t}"),
                Primitive::Arch,
                [0., 4.5, z + dir * 5.],
                [12., 3., 0.5],
                0.,
                None,
                c,
                "",
                None,
            );
            for x in [-8., 8.] {
                add(
                    &format!("base-ramp-{t}-{x}"),
                    Primitive::Ramp,
                    [x, 1.5, z + dir * 10.],
                    [5., 3., 10.],
                    yaw + std::f32::consts::PI,
                    None,
                    "#556b77",
                    "BASE RAMP",
                    None,
                );
            }
            add(
                &format!("mid-ramp-{t}"),
                Primitive::Ramp,
                [0., 1.5, -dir * 11.],
                [6., 3., 10.],
                yaw,
                None,
                "#556b77",
                "REACTOR RAMP",
                None,
            );
            for x in [-20_f32, 20.] {
                add(
                    &format!("upper-deck-{t}-{x}"),
                    Primitive::Platform,
                    [x.signum() * 22., 5.75, z],
                    [18., 0.5, 10.],
                    0.,
                    None,
                    "#516573",
                    "SKY WALK",
                    None,
                );
                add(
                    &format!("upper-ramp-{t}-{x}"),
                    Primitive::Ramp,
                    [x.signum() * 29., 3., z + dir * 11.],
                    [4., 6., 12.],
                    yaw + std::f32::consts::PI,
                    None,
                    "#627b85",
                    "SKY RAMP",
                    None,
                );
                add(
                    &format!("base-bridge-{t}-{x}"),
                    Primitive::Bridge,
                    [x.signum() * 14., 5.75, z],
                    [8., 0.5, 4.],
                    0.,
                    None,
                    "#536675",
                    "",
                    None,
                );
                add(
                    &format!("upper-rail-{t}-{x}"),
                    Primitive::Barrier,
                    [x.signum() * 20., 6.55, z - dir * 4.7],
                    [12., 1.1, 0.25],
                    0.,
                    None,
                    c,
                    "",
                    None,
                );
                add(
                    &format!("light-base-{t}-{x}"),
                    Primitive::Light,
                    [x, 8., z],
                    [0.5, 0.2, 0.5],
                    0.,
                    None,
                    c,
                    "",
                    None,
                );
            }
            add(
                &format!("jump-{t}"),
                Primitive::JumpPad,
                [14., 0.15, z + dir * 7.],
                [3., 0.3, 3.],
                0.,
                None,
                c,
                "GRAVITY LIFT",
                None,
            );
            add(
                &format!("base-light-{t}"),
                Primitive::Light,
                [0., 5.8, z],
                [4., 0.15, 1.],
                0.,
                None,
                c,
                "",
                None,
            );
            add(
                &format!("med-{t}"),
                Primitive::HealthPickup,
                [-6., 3.6, z - dir * 2.],
                [1., 1., 1.],
                0.,
                None,
                "#80ebad",
                "MEDKIT",
                None,
            );
            add(
                &format!("armor-{t}"),
                Primitive::ShieldPickup,
                [6., 3.6, z - dir * 2.],
                [1., 1., 1.],
                0.,
                None,
                "#68cafa",
                "SHIELD CELL",
                None,
            );
            add(
                &format!("console-{t}"),
                Primitive::Box,
                [10., 3.7, z - dir * 2.],
                [1.5, 1.4, 1.3],
                yaw,
                None,
                c,
                "",
                Some("computer"),
            );
        }
        // Lateral rooms provide sheltered ground routes under a pair of bridges.
        for (i, x) in [-22., 22.].into_iter().enumerate() {
            add(
                &format!("side-roof-{i}"),
                Primitive::Platform,
                [x, 3.75, 0.],
                [12., 0.5, 16.],
                0.,
                None,
                "#455967",
                "MAINTENANCE",
                None,
            );
            add(
                &format!("side-back-{i}"),
                Primitive::Wall,
                [x + x.signum() * 6., 1.75, 0.],
                [0.5, 3.5, 16.],
                0.,
                None,
                "#334654",
                "",
                None,
            );
            for z in [-8., 8.] {
                add(
                    &format!("side-door-{i}-{z}"),
                    Primitive::Arch,
                    [x, 1.75, z],
                    [12., 3.5, 0.5],
                    0.,
                    None,
                    "#627888",
                    "",
                    None,
                );
            }
            add(
                &format!("side-light-{i}"),
                Primitive::Light,
                [x, 3.4, 0.],
                [3., 0.15, 1.],
                0.,
                None,
                accent,
                "",
                None,
            );
            add(
                &format!("side-bridge-{i}"),
                Primitive::Bridge,
                [x, 5.75, 0.],
                [5., 0.5, 40.],
                0.,
                None,
                "#566e7e",
                "UPPER GANTRY",
                None,
            );
            for z in [-9., 9.] {
                add(
                    &format!("connector-{i}-{z}"),
                    Primitive::Bridge,
                    [x * 0.5, 5.75, z],
                    [19., 0.5, 3.],
                    0.,
                    None,
                    "#566e7e",
                    "CROSS WALK",
                    None,
                );
            }
            // Center upper landing reachable from lower reactor via stairs.
            add(
                &format!("upper-mid-{i}"),
                Primitive::Platform,
                [x.signum() * 10., 5.75, 7.],
                [3., 0.5, 4.],
                0.,
                None,
                "#627987",
                "OVERLOOK",
                None,
            );
            add(
                &format!("mid-stairs-{i}"),
                Primitive::Stairs,
                [x.signum() * 10., 4.5, 0.],
                [3., 3., 12.],
                0.,
                None,
                "#708997",
                "OVERLOOK STAIRS",
                None,
            );
            add(
                &format!("stair-approach-{i}"),
                Primitive::Platform,
                [x.signum() * 8., 2.75, -8.],
                [7., 0.5, 4.],
                0.,
                None,
                "#627987",
                "",
                None,
            );
            // Low cover is deliberately staggered, leaving a continuous lower lane.
            for (j, z) in [-14., -5., 5., 14.].into_iter().enumerate() {
                add(
                    &format!("crate-{i}-{j}"),
                    Primitive::Box,
                    [x.signum() * 12., 1., z],
                    [2.5, 2., 2.5],
                    j as f32 * 0.22,
                    None,
                    "#657986",
                    "CARGO",
                    Some("container-wide"),
                );
            }
            add(
                &format!("room-crate-{i}"),
                Primitive::Box,
                [x + 2., 1.2, 2.],
                [2., 2.4, 2.],
                0.,
                None,
                "#6e8394",
                "CARGO",
                Some("container"),
            );
            add(
                &format!("terminal-{i}"),
                Primitive::Box,
                [x + x.signum() * 4., 0.9, -3.],
                [2.4, 1.8, 1.4],
                0.,
                None,
                "#657986",
                "",
                Some("computer-wide"),
            );
        }
        for (i, weapon, p) in [
            (0, Weapon::Shotgun, [-22., 0.7, 0.]),
            (1, Weapon::Rocket, [0., 3.7, 0.]),
            (2, Weapon::Sniper, [20., 6.7, -25.]),
            (3, Weapon::Precision, [-20., 6.7, 25.]),
            (4, Weapon::Rifle, [10., 0.7, 0.]),
        ] {
            add(
                &format!("weapon-{i}"),
                Primitive::WeaponSpawn,
                p,
                [1.5, 1.5, 1.5],
                0.,
                None,
                "#ffca78",
                "POWER WEAPON",
                None,
            );
            let _ = weapon;
        }
        for (i, w) in [
            Weapon::Shotgun,
            Weapon::Rocket,
            Weapon::Sniper,
            Weapon::Precision,
            Weapon::Rifle,
        ]
        .into_iter()
        .enumerate()
        {
            objects
                .iter_mut()
                .find(|o| o.id == format!("weapon-{i}"))
                .unwrap()
                .properties
                .weapon = w;
        }
        for (i, x, z, y) in [
            (0, 0., 0., 0.7),
            (1, -22., 5., 0.7),
            (2, 22., -5., 0.7),
            (3, -20., -23., 6.7),
            (4, 20., 23., 6.7),
        ] {
            objects.push(MapObject {
                id: format!("ammo-{i}"),
                kind: Primitive::AmmoPickup,
                position: Vec3::new(x, y, z),
                rotation: 0.,
                scale: Vec3::ONE,
                properties: Properties {
                    color: "#ffca78".into(),
                    label: "AMMO CACHE".into(),
                    ..Default::default()
                },
            });
        }
        for o in &mut objects {
            if o.kind == Primitive::JumpPad {
                o.properties.strength = 20.;
            }
        }
        // Theme variations rotate the whole plan and change cover distribution;
        // all variants retain readable team colors and navigable three-level routes.
        if variant > 0 {
            for o in &mut objects {
                let a = variant as f32 * std::f32::consts::FRAC_PI_2;
                let (sn, cs) = a.sin_cos();
                o.position = Vec3::new(
                    cs * o.position.x + sn * o.position.z,
                    o.position.y,
                    -sn * o.position.x + cs * o.position.z,
                );
                o.rotation += a;
            }
        }
        Map {
            name: name.into(),
            skybox: sky.into(),
            objects,
        }
    })
    .collect()
}
