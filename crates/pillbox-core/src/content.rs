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
    pub piece: Option<crate::BuildingPiece>,
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
            piece: None,
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
    /// Historical versions remain readable; all newly authored maps use the kit.
    #[serde(default)]
    pub grid: bool,
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
            if self.grid {
                o.validate_grid()?;
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
        if self.grid {
            let solids: Vec<_> = self.objects.iter().filter(|o| o.kind.solid()).collect();
            for (i, a) in solids.iter().enumerate() {
                for b in solids.iter().skip(i + 1) {
                    let overlap =
                        (a.grid_size() + b.grid_size()) * 0.5 - (a.position - b.position).abs();
                    if overlap.min_element() > 0.001 {
                        return Err(format!(
                            "Pieces {} and {} overlap; snap their faces together",
                            a.id, b.id
                        ));
                    }
                }
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
    crate::arena_maps().remove(0)
}
