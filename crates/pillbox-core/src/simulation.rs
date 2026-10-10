use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Input {
    pub seq: u64,
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub jump: bool,
    pub fire: bool,
    pub reload: bool,
    pub weapon: Option<Weapon>,
    pub view_tick: u64,
}
impl Input {
    pub fn valid(&self) -> bool {
        self.x.is_finite()
            && self.z.is_finite()
            && self.yaw.is_finite()
            && self.pitch.is_finite()
            && self.x.abs() <= 1.
            && self.z.abs() <= 1.
            && self.yaw.abs() < 100000.
            && self.pitch.abs() <= 1.55
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub team: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub grounded: bool,
    pub health: f32,
    pub shield: f32,
    pub weapon: Weapon,
    pub inventory: Vec<Weapon>,
    pub magazines: [u16; 6],
    pub reserve: [u16; 6],
    pub score: i32,
    pub kills: u32,
    pub deaths: u32,
    pub alive: bool,
    pub respawn_at: u64,
    pub last_damage: u64,
    pub next_fire: u64,
    pub reload_at: u64,
    pub teleport_at: u64,
    pub ack: u64,
    pub carrying: Option<u8>,
    pub streak: u32,
    pub last_kill: u64,
    pub multi_kills: u32,
}
impl Player {
    pub fn new(id: String, name: String, team: u8, position: Vec3) -> Self {
        Self {
            id,
            name,
            team,
            position,
            velocity: Vec3::ZERO,
            yaw: 0.,
            pitch: 0.,
            grounded: false,
            health: 100.,
            shield: 100.,
            weapon: Weapon::Pistol,
            inventory: vec![Weapon::Pistol, Weapon::Rifle],
            magazines: Weapon::ALL.map(|w| w.stats().magazine),
            reserve: [48, 128, 40, 24, 12, 6],
            score: 0,
            kills: 0,
            deaths: 0,
            alive: true,
            respawn_at: 0,
            last_damage: 0,
            next_fire: 0,
            reload_at: 0,
            teleport_at: 0,
            ack: 0,
            carrying: None,
            streak: 0,
            last_kill: 0,
            multi_kills: 0,
        }
    }
    pub fn ammo(&self) -> u16 {
        self.magazines[self.weapon as usize]
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Waiting,
    Starting,
    Playing,
    Finished,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameEvent {
    pub id: u64,
    pub tick: u64,
    pub kind: String,
    pub actor: Option<String>,
    pub target: Option<String>,
    pub weapon: Option<Weapon>,
    pub position: Option<Vec3>,
    #[serde(default)]
    pub endpoint: Option<Vec3>,
    #[serde(default)]
    pub announcement: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projectile {
    pub owner: String,
    pub team: u8,
    pub position: Vec3,
    pub velocity: Vec3,
    pub expires: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlagState {
    pub team: u8,
    pub home: Vec3,
    pub position: Vec3,
    pub carrier: Option<String>,
    pub return_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub tick: u64,
    pub phase: Phase,
    pub remaining: u32,
    pub players: Vec<Player>,
    pub team_scores: [i32; 2],
    pub events: Vec<GameEvent>,
    pub projectiles: Vec<Projectile>,
    pub flags: Vec<FlagState>,
    pub hill_owner: Option<u8>,
    pub hill_contested: bool,
    pub pickups: BTreeMap<String, u64>,
}
#[derive(Clone, Debug)]
pub struct World {
    pub map: Map,
    pub mode: Mode,
    pub tick: u64,
    pub phase: Phase,
    pub players: BTreeMap<String, Player>,
    pub team_scores: [i32; 2],
    pub events: VecDeque<GameEvent>,
    pub projectiles: Vec<Projectile>,
    pub flags: Vec<FlagState>,
    pub hill_owner: Option<u8>,
    pub hill_contested: bool,
    pub pickups: BTreeMap<String, u64>,
    pub departed: Vec<Player>,
    start_at: u64,
    end_at: u64,
    event_id: u64,
    history: VecDeque<(u64, BTreeMap<String, Vec3>)>,
    zones: BTreeSet<(String, String)>,
    physics: PhysicsMap,
    pub navigation: Navigation,
}

fn rotate(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}
fn local(o: &MapObject, p: Vec3) -> Vec3 {
    rotate(p - o.position, -o.rotation)
}
/// Surface in local coordinates. Arch uses two posts and a lintel, ramps are actual slopes.
pub(crate) fn local_top(o: &MapObject, p: Vec3, margin: f32) -> Option<f32> {
    let h = o.scale * 0.5;
    if p.x.abs() > h.x + margin || p.z.abs() > h.z + margin {
        return None;
    }
    match o.kind {
        Primitive::Ramp | Primitive::Wedge => {
            Some(-h.y + o.scale.y * ((p.z + h.z) / o.scale.z).clamp(0., 1.))
        }
        Primitive::Stairs => Some(
            -h.y + o.scale.y
                * ((((p.z + h.z) / o.scale.z).clamp(0., 1.) * 8.).floor() + 1.).min(8.)
                / 8.,
        ),
        Primitive::Cylinder | Primitive::Pillar
            if (p.x / (h.x + margin)).powi(2) + (p.z / (h.z + margin)).powi(2) > 1. =>
        {
            None
        }
        Primitive::Sphere => {
            let v = 1. - (p.x / (h.x + margin)).powi(2) - (p.z / (h.z + margin)).powi(2);
            if v < 0. { None } else { Some(h.y * v.sqrt()) }
        }
        _ => Some(h.y),
    }
}
pub fn position_clear(map: &Map, p: Vec3) -> bool {
    PhysicsMap::new(map).clear(p)
}
fn inside(o: &MapObject, p: Vec3) -> bool {
    let p = local(o, p);
    p.abs().cmple(o.scale * 0.5 + Vec3::splat(0.35)).all()
}
fn solid_point(o: &MapObject, p: Vec3) -> bool {
    let p = local(o, p);
    let h = o.scale * 0.5;
    if o.kind == Primitive::Arch && p.x.abs() < h.x * 0.65 && p.y < h.y * 0.55 {
        return false;
    }
    if o.kind == Primitive::Sphere {
        return (p / h).length_squared() <= 1.;
    }
    local_top(o, p, 0.).is_some_and(|top| p.y >= -h.y && p.y <= top)
}
pub(crate) fn wall_distance(map: &Map, origin: Vec3, dir: Vec3, max: f32) -> f32 {
    let mut closest = max;
    for o in map.objects.iter().filter(|o| o.kind.solid()) {
        let Some((near, far)) = ray_box(
            local(o, origin),
            rotate(dir, -o.rotation),
            o.scale * 0.5,
            closest,
        ) else {
            continue;
        };
        if matches!(
            o.kind,
            Primitive::Box
                | Primitive::Wall
                | Primitive::Floor
                | Primitive::Platform
                | Primitive::Bridge
                | Primitive::Barrier
        ) {
            closest = closest.min(near);
        } else {
            let mut t = near;
            while t <= far && t < closest {
                if solid_point(o, origin + dir * t) {
                    closest = t;
                    break;
                }
                t += 0.025;
            }
        }
    }
    closest
}
fn ray_box(p: Vec3, d: Vec3, h: Vec3, max: f32) -> Option<(f32, f32)> {
    let mut near = 0.0_f32;
    let mut far = max;
    for i in 0..3 {
        if d[i].abs() < 0.00001 {
            if p[i].abs() > h[i] {
                return None;
            }
        } else {
            let a = (-h[i] - p[i]) / d[i];
            let b = (h[i] - p[i]) / d[i];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
    }
    (near <= far).then_some((near, far))
}
/// Shared by the server and browser prediction. No wall-clock, IO, renderer or database dependency.
pub fn move_player(map: &Map, p: &mut Player, input: Input, tick: u64) {
    move_player_with_physics(map, &PhysicsMap::new(map), p, input, tick);
}
pub fn move_player_with_physics(
    map: &Map,
    physics: &PhysicsMap,
    p: &mut Player,
    input: Input,
    tick: u64,
) {
    if !p.alive || !input.valid() {
        return;
    }
    p.yaw = input.yaw;
    p.pitch = input.pitch;
    let mut speed = 8.;
    let mut gravity = 24.;
    for o in &map.objects {
        if inside(o, p.position + Vec3::Y * 0.5) {
            match o.kind {
                Primitive::SpeedBoost => speed = o.properties.strength.clamp(8., 24.),
                Primitive::GravityModifier => gravity = o.properties.strength,
                _ => (),
            }
        }
    }
    let wish = rotate(
        Vec3::new(input.x, 0., input.z).clamp_length_max(1.),
        -input.yaw,
    ) * speed;
    let accel = if p.grounded { 0.45 } else { 0.12 };
    p.velocity.x += (wish.x - p.velocity.x) * accel;
    p.velocity.z += (wish.z - p.velocity.z) * accel;
    if input.jump && p.grounded {
        p.velocity.y = 9.5;
        p.grounded = false;
    }
    p.velocity.y = (p.velocity.y - gravity * DT).max(-40.);
    p.grounded = physics.move_capsule(&mut p.position, &mut p.velocity, DT, p.grounded);
    for o in &map.objects {
        if !inside(o, p.position + Vec3::Y * 0.3) {
            continue;
        }
        match o.kind {
            Primitive::JumpPad if p.velocity.y <= 1. => {
                p.velocity.y = o.properties.strength;
                p.grounded = false;
            }
            Primitive::TeleporterEntrance if tick >= p.teleport_at => {
                if let Some(exit) = map.objects.iter().find(|e| {
                    Some(&e.id) == o.properties.target.as_ref()
                        && e.kind == Primitive::TeleporterExit
                }) {
                    let destination = exit.position + Vec3::Y * 0.1;
                    if position_clear(map, destination) {
                        p.position = destination;
                        p.velocity = Vec3::ZERO;
                        p.teleport_at = tick + 60;
                    }
                }
            }
            _ => (),
        }
    }
}
impl World {
    pub fn new(map: Map, mode: Mode) -> Result<Self, String> {
        map.validate()?;
        mode.validate()?;
        map.compatible(&mode)?;
        let physics = PhysicsMap::new(&map);
        let navigation = Navigation::new(&map, &physics);
        let mut w = Self {
            navigation,
            physics,
            map,
            mode,
            tick: 0,
            phase: Phase::Waiting,
            players: BTreeMap::new(),
            team_scores: [0, 0],
            events: VecDeque::new(),
            projectiles: vec![],
            flags: vec![],
            hill_owner: None,
            hill_contested: false,
            pickups: BTreeMap::new(),
            departed: vec![],
            start_at: 0,
            end_at: 0,
            event_id: 0,
            history: VecDeque::new(),
            zones: BTreeSet::new(),
        };
        w.reset_flags();
        Ok(w)
    }
    fn participant(&self, id: &str) -> Option<&Player> {
        self.players
            .get(id)
            .or_else(|| self.departed.iter().find(|p| p.id == id))
    }
    fn participant_mut(&mut self, id: &str) -> Option<&mut Player> {
        if self.players.contains_key(id) {
            self.players.get_mut(id)
        } else {
            self.departed.iter_mut().find(|p| p.id == id)
        }
    }
    fn spawn_position(&self, team: u8) -> Vec3 {
        self.map
            .objects
            .iter()
            .filter(|o| {
                o.kind.spawn()
                    && (!self.mode.teams
                        || o.properties.team.is_none()
                        || o.properties.team == Some(team))
            })
            .max_by(|a, b| {
                let safety = |p: Vec3| {
                    self.players
                        .values()
                        .filter(|p| p.alive)
                        .map(|v| v.position.distance_squared(p))
                        .fold(10000., f32::min)
                };
                safety(a.position).total_cmp(&safety(b.position))
            })
            .map(|o| o.position)
            .unwrap_or(Vec3::Y * 3.)
    }
    pub fn join(&mut self, id: String, name: String) -> Result<(), String> {
        if self.players.contains_key(&id) {
            return Err("Already joined".into());
        }
        if self.players.len() >= 16 || !valid_name(&name) {
            return Err("Invalid player or full server".into());
        }
        let returning = if matches!(self.phase, Phase::Starting | Phase::Playing) {
            self.departed.iter().rposition(|p| p.id == id)
        } else {
            None
        };
        let p = if let Some(index) = returning {
            let mut player = self.departed.remove(index);
            // Transport input sequences restart, but match state and timers do not.
            player.ack = 0;
            player
        } else {
            let counts = [
                self.players.values().filter(|p| p.team == 0).count(),
                self.players.values().filter(|p| p.team == 1).count(),
            ];
            let team = u8::from(counts[0] > counts[1]);
            Player::new(id.clone(), name, team, self.spawn_position(team))
        };
        self.players.insert(id.clone(), p);
        if returning.is_none() {
            self.rules(EventKind::PlayerSpawned, &id, None, None);
        }
        Ok(())
    }
    pub fn leave(&mut self, id: &str) {
        self.drop_flag(id);
        if let Some(p) = self.players.remove(id) {
            self.departed.push(p);
        }
        self.zones.retain(|(p, _)| p != id);
    }
    pub fn start(&mut self) -> Result<(), String> {
        if !matches!(self.phase, Phase::Waiting | Phase::Finished) || self.players.is_empty() {
            return Err("Match cannot start now".into());
        }
        self.phase = Phase::Starting;
        self.start_at = self.tick + 180;
        self.end_at = self.start_at + u64::from(self.mode.time_limit) * 60;
        self.team_scores = [0, 0];
        self.events.clear();
        self.departed.clear();
        self.pickups.clear();
        self.projectiles.clear();
        self.history.clear();
        self.zones.clear();
        self.hill_owner = None;
        self.hill_contested = false;
        self.reset_flags();
        for id in self.players.keys().cloned().collect::<Vec<_>>() {
            let old = &self.players[&id];
            let p = Player::new(
                id.clone(),
                old.name.clone(),
                old.team,
                self.spawn_position(old.team),
            );
            self.players.insert(id, p);
        }
        self.emit("match_starting", None, None, None, None);
        Ok(())
    }
    pub fn finish(&mut self) {
        if self.phase != Phase::Finished {
            self.phase = Phase::Finished;
            self.emit("match_over", None, None, None, None);
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            tick: self.tick,
            phase: self.phase,
            remaining: ((self.end_at.saturating_sub(self.tick)) / 60) as u32,
            players: self.players.values().cloned().collect(),
            team_scores: self.team_scores,
            events: self.events.iter().cloned().collect(),
            projectiles: self.projectiles.clone(),
            flags: self.flags.clone(),
            hill_owner: self.hill_owner,
            hill_contested: self.hill_contested,
            pickups: self.pickups.clone(),
        }
    }
    fn emit(
        &mut self,
        kind: &str,
        actor: Option<String>,
        target: Option<String>,
        weapon: Option<Weapon>,
        position: Option<Vec3>,
    ) {
        self.event_id += 1;
        self.events.push_back(GameEvent {
            id: self.event_id,
            tick: self.tick,
            kind: kind.into(),
            actor,
            target,
            weapon,
            position,
            endpoint: None,
            announcement: false,
        });
        while self.events.len() > 48 {
            self.events.pop_front();
        }
    }
    pub fn step(&mut self, inputs: &BTreeMap<String, Input>) {
        self.tick += 1;
        if self.phase == Phase::Starting && self.tick >= self.start_at {
            self.phase = Phase::Playing;
            self.emit("match_started", None, None, None, None);
            for id in self.players.keys().cloned().collect::<Vec<_>>() {
                self.rules(EventKind::MatchStarted, &id, None, None);
            }
        }
        if self.phase != Phase::Playing {
            return;
        }
        let ids: Vec<_> = self.players.keys().cloned().collect();
        for id in &ids {
            if !self.players[id].alive {
                if self.tick >= self.players[id].respawn_at {
                    self.respawn(id);
                } else {
                    continue;
                }
            }
            let p = self.players.get_mut(id).unwrap();
            let input = inputs
                .get(id)
                .copied()
                .filter(Input::valid)
                .filter(|i| i.seq > p.ack)
                .unwrap_or(Input {
                    yaw: p.yaw,
                    pitch: p.pitch,
                    ..Default::default()
                });
            if input.seq > 0 {
                p.ack = input.seq;
            }
            let old_grounded = p.grounded;
            let old_velocity = p.velocity;
            let old_position = p.position;
            let old_weapon = p.weapon;
            let old_reload = p.reload_at;
            let old_shield = p.shield;
            move_player_with_physics(&self.map, &self.physics, p, input, self.tick);
            if let Some(w) = input.weapon
                && p.inventory.contains(&w)
                && p.reload_at == 0
            {
                p.weapon = w;
            }
            let wi = p.weapon as usize;
            let stats = p.weapon.stats();
            if p.reload_at > 0 && self.tick >= p.reload_at {
                let count = (stats.magazine - p.magazines[wi]).min(p.reserve[wi]);
                p.magazines[wi] += count;
                p.reserve[wi] -= count;
                p.reload_at = 0;
            }
            if input.reload && p.reload_at == 0 && p.ammo() < stats.magazine && p.reserve[wi] > 0 {
                p.reload_at = self.tick + stats.reload;
            }
            if self.tick > p.last_damage + 300 {
                p.shield = (p.shield + 25. * DT).min(100.);
            }
            let mut sounds = vec![];
            if input.jump && old_grounded && !p.grounded {
                sounds.push("jump");
            }
            if !p.grounded && p.velocity.y > 10. && old_velocity.y <= 1. {
                sounds.push("jump_pad");
            }
            if !old_grounded && p.grounded && old_velocity.y < -1. {
                sounds.push("land");
            }
            if p.position.distance(old_position) > 3. {
                sounds.push("teleport");
            }
            if p.weapon != old_weapon {
                sounds.push("weapon_switch");
            }
            if old_reload == 0 && p.reload_at > 0 {
                sounds.push("reload");
            }
            if old_reload > 0 && p.reload_at == 0 {
                sounds.push("reload_complete");
            }
            if old_shield == 0. && p.shield > 0. {
                sounds.push("shield_recharge");
            }
            if p.grounded && p.velocity.length_squared() > 5. && self.tick.is_multiple_of(22) {
                sounds.push("footstep");
            }
            if input.fire && p.ammo() == 0 && self.tick.is_multiple_of(30) {
                sounds.push("dry_fire");
            }
            let sound_position = p.position;
            let sound_weapon = p.weapon;
            for sound in sounds {
                self.emit(
                    sound,
                    Some(id.clone()),
                    None,
                    Some(sound_weapon),
                    Some(sound_position),
                );
            }
            let p = &self.players[id];
            if p.position.y < -40.
                || p.position.abs().max_element() > 300.
                || self
                    .map
                    .objects
                    .iter()
                    .any(|o| o.kind == Primitive::KillVolume && inside(o, p.position + Vec3::Y))
            {
                self.damage(id, id, 1000., Weapon::Pistol);
                continue;
            }
            if input.fire {
                self.fire(id, input.view_tick);
            }
            self.pickup(id);
        }
        self.projectiles_step();
        self.objectives();
        self.zone_events();
        if self.tick.is_multiple_of(60) {
            for id in ids {
                self.rules(EventKind::TimerElapsed, &id, None, None);
            }
        }
        self.history.push_back((
            self.tick,
            self.players
                .iter()
                .filter(|(_, p)| p.alive)
                .map(|(id, p)| (id.clone(), p.position))
                .collect(),
        ));
        while self.history.len() > 13 {
            self.history.pop_front();
        }
        let remaining = self.end_at.saturating_sub(self.tick);
        if remaining == 3600 {
            self.emit("one_minute_remaining", None, None, None, None);
        }
        if remaining == 1800 {
            self.emit("thirty_seconds_remaining", None, None, None, None);
        }
        if self.tick >= self.end_at
            || (self.mode.teams && self.team_scores.iter().any(|s| *s >= self.mode.score_limit))
            || (!self.mode.teams
                && self
                    .players
                    .values()
                    .chain(self.departed.iter())
                    .any(|p| p.score >= self.mode.score_limit))
        {
            self.finish();
        }
    }
    fn respawn(&mut self, id: &str) {
        let old = &self.players[id];
        let mut p = Player::new(
            id.into(),
            old.name.clone(),
            old.team,
            self.spawn_position(old.team),
        );
        p.score = old.score;
        p.kills = old.kills;
        p.deaths = old.deaths;
        p.ack = old.ack;
        p.last_damage = self.tick;
        let position = p.position;
        self.players.insert(id.into(), p);
        self.emit("respawn", Some(id.into()), None, None, Some(position));
        self.rules(EventKind::PlayerSpawned, id, None, None);
    }
    fn fire(&mut self, id: &str, view_tick: u64) {
        let p = self.players.get_mut(id).unwrap();
        if !p.alive || self.tick < p.next_fire || p.reload_at > 0 || p.ammo() == 0 {
            return;
        }
        let weapon = p.weapon;
        let stats = weapon.stats();
        p.magazines[weapon as usize] -= 1;
        p.next_fire = self.tick + stats.cooldown;
        let origin = p.position + Vec3::Y * 1.5;
        let yaw = p.yaw;
        let pitch = p.pitch;
        let team = p.team;
        let aim = Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        );
        self.emit("shot", Some(id.into()), None, Some(weapon), Some(origin));
        let endpoint = origin + aim * wall_distance(&self.map, origin, aim, 80.);
        self.events.back_mut().unwrap().endpoint = Some(endpoint);
        if stats.speed > 0. {
            self.projectiles.push(Projectile {
                owner: id.into(),
                team,
                position: origin,
                velocity: aim * stats.speed,
                expires: self.tick + 240,
            });
            return;
        }
        // Rewind other players by at most 200ms; never accept a client-reported hit or damage.
        let rewind = view_tick.clamp(self.tick.saturating_sub(12), self.tick);
        let history = self.history.iter().rev().find(|(t, _)| *t <= rewind);
        let mut hits = vec![];
        for pellet in 0..stats.pellets {
            let noise = |salt: u64| {
                let n = (self
                    .tick
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(u64::from(pellet) * 7919 + salt))
                .rotate_left(17);
                (n % 10000) as f32 / 5000. - 1.
            };
            let dir =
                (aim + Vec3::new(noise(11), noise(983), noise(4923)) * stats.spread).normalize();
            let mut closest = wall_distance(&self.map, origin, dir, 120.);
            let mut victim = None;
            for (other, q) in &self.players {
                if other == id
                    || !q.alive
                    || (self.mode.teams && !self.mode.friendly_fire && q.team == team)
                {
                    continue;
                }
                let pos = history
                    .and_then(|(_, h)| h.get(other))
                    .copied()
                    .unwrap_or(q.position);
                // Capsule approximated by overlapping spheres along its spine.
                for y in [0.4, 0.9, 1.4] {
                    let delta = pos + Vec3::Y * y - origin;
                    let along = delta.dot(dir);
                    let perp = delta.length_squared() - along * along;
                    if along > 0. && perp <= RADIUS * RADIUS {
                        let t = along - (RADIUS * RADIUS - perp).sqrt();
                        if t < closest {
                            closest = t;
                            victim = Some(other.clone());
                        }
                    }
                }
            }
            if let Some(v) = victim {
                hits.push(v);
            }
        }
        for v in hits {
            self.damage(id, &v, stats.damage, weapon);
        }
    }
    pub fn damage(&mut self, attacker: &str, victim: &str, amount: f32, weapon: Weapon) {
        let Some(v) = self.players.get_mut(victim) else {
            return;
        };
        if !v.alive {
            return;
        }
        let previous_shield = v.shield;
        let shield = v.shield.min(amount);
        v.shield -= shield;
        v.health = (v.health - (amount - shield)).max(0.);
        v.last_damage = self.tick;
        if v.health > 0. {
            let kind = if previous_shield > 0. && v.shield == 0. {
                "shield_broken"
            } else {
                "hit"
            };
            let position = v.position;
            self.emit(
                kind,
                Some(attacker.into()),
                Some(victim.into()),
                Some(weapon),
                Some(position),
            );
            return;
        }
        v.alive = false;
        v.deaths += 1;
        v.streak = 0;
        v.respawn_at = self.tick + u64::from(self.mode.respawn_seconds) * 60;
        let death_position = v.position;
        self.drop_flag(victim);
        let kill_tick = self.tick;
        if attacker != victim
            && let Some(a) = self.participant_mut(attacker)
        {
            a.kills += 1;
            a.streak += 1;
            let spree = a.streak.is_multiple_of(5);
            a.multi_kills = if kill_tick.saturating_sub(a.last_kill) < 240 {
                a.multi_kills + 1
            } else {
                1
            };
            let multi = a.multi_kills;
            a.last_kill = kill_tick;
            if spree {
                self.emit("killing_spree", Some(attacker.into()), None, None, None);
            }
            if multi >= 2 {
                self.emit(
                    if multi >= 3 {
                        "triple_kill"
                    } else {
                        "double_kill"
                    },
                    Some(attacker.into()),
                    None,
                    None,
                    None,
                );
            }
        }
        self.emit(
            "player_killed",
            Some(attacker.into()),
            Some(victim.into()),
            Some(weapon),
            Some(death_position),
        );
        self.rules(EventKind::PlayerKilled, attacker, Some(victim), None);
    }
    fn projectiles_step(&mut self) {
        let mut keep = vec![];
        for mut r in std::mem::take(&mut self.projectiles) {
            let distance = r.velocity.length() * DT;
            let dir = r.velocity.normalize();
            let travel = wall_distance(&self.map, r.position, dir, distance);
            let blocked = travel < distance;
            r.position += dir
                * if blocked {
                    (travel - 0.03).max(0.)
                } else {
                    distance
                };
            let hit = self.players.values().any(|p| {
                p.alive && p.id != r.owner && p.position.distance(r.position - Vec3::Y * 0.9) < 0.85
            });
            if blocked || hit || self.tick >= r.expires {
                self.emit(
                    "explosion",
                    Some(r.owner.clone()),
                    None,
                    Some(Weapon::Rocket),
                    Some(r.position),
                );
                for id in self.players.keys().cloned().collect::<Vec<_>>() {
                    let p = &self.players[&id];
                    if !p.alive {
                        continue;
                    }
                    if id != r.owner
                        && self.mode.teams
                        && !self.mode.friendly_fire
                        && r.team == p.team
                    {
                        continue;
                    }
                    let target = p.position + Vec3::Y * 0.9;
                    let d = target.distance(r.position);
                    if d < 5.
                        && wall_distance(
                            &self.map,
                            r.position,
                            (target - r.position).normalize_or_zero(),
                            d,
                        ) >= d
                    {
                        self.damage(&r.owner, &id, 180. * (1. - d / 5.), Weapon::Rocket);
                    }
                }
            } else {
                keep.push(r);
            }
        }
        self.projectiles = keep;
    }
    fn pickup(&mut self, id: &str) {
        if !self.players[id].alive {
            return;
        }
        for o in self.map.objects.clone() {
            if self.pickups.get(&o.id).is_some_and(|t| *t > self.tick)
                || !inside(&o, self.players[id].position + Vec3::Y * 0.5)
            {
                continue;
            }
            let p = self.players.get_mut(id).unwrap();
            let used = match o.kind {
                Primitive::HealthPickup if p.health < 100. => {
                    p.health = 100.;
                    true
                }
                Primitive::ShieldPickup if p.shield < 100. => {
                    p.shield = 100.;
                    true
                }
                Primitive::WeaponSpawn => {
                    if !p.inventory.contains(&o.properties.weapon) {
                        p.inventory.push(o.properties.weapon);
                    }
                    p.weapon = o.properties.weapon;
                    p.reload_at = 0;
                    p.magazines[p.weapon as usize] = p.weapon.stats().magazine;
                    p.reserve[p.weapon as usize] = p.weapon.stats().magazine * 4;
                    true
                }
                Primitive::AmmoPickup => {
                    p.reserve[p.weapon as usize] =
                        (p.reserve[p.weapon as usize] + p.weapon.stats().magazine * 2).min(256);
                    true
                }
                _ => false,
            };
            if used {
                let kind = match o.kind {
                    Primitive::HealthPickup => "health_pickup",
                    Primitive::ShieldPickup => "shield_pickup",
                    Primitive::WeaponSpawn => "weapon_pickup",
                    _ => "ammo_pickup",
                };
                let weapon = p.weapon;
                self.emit(
                    kind,
                    Some(id.into()),
                    Some(o.id.clone()),
                    Some(weapon),
                    Some(o.position),
                );
                self.pickups.insert(
                    o.id.clone(),
                    self.tick + u64::from(o.properties.respawn_seconds) * 60,
                );
                self.rules(EventKind::PlayerPickedUpItem, id, None, Some(&o));
            }
        }
    }
    fn reset_flags(&mut self) {
        self.flags = self
            .map
            .objects
            .iter()
            .filter(|o| o.kind == Primitive::FlagBase)
            .filter_map(|o| {
                o.properties.team.map(|team| FlagState {
                    team,
                    home: self
                        .map
                        .objects
                        .iter()
                        .find(|f| f.kind == Primitive::Flag && f.properties.team == Some(team))
                        .map_or(o.position, |f| f.position),
                    position: self
                        .map
                        .objects
                        .iter()
                        .find(|f| f.kind == Primitive::Flag && f.properties.team == Some(team))
                        .map_or(o.position, |f| f.position),
                    carrier: None,
                    return_at: 0,
                })
            })
            .collect();
        for p in self.players.values_mut() {
            p.carrying = None;
        }
    }
    fn drop_flag(&mut self, id: &str) {
        for f in &mut self.flags {
            if f.carrier.as_deref() == Some(id) {
                f.carrier = None;
                f.return_at = self.tick + 900;
            }
        }
        if let Some(p) = self.players.get_mut(id)
            && p.carrying.take().is_some()
        {
            self.emit("flag_dropped", Some(id.into()), None, None, None);
            self.rules(EventKind::PlayerDroppedItem, id, None, None);
        }
    }
    fn objectives(&mut self) {
        if self.mode.objective == Objective::Flags {
            for i in 0..self.flags.len() {
                if let Some(id) = self.flags[i].carrier.clone() {
                    if let Some(p) = self.players.get(&id) {
                        self.flags[i].position = p.position;
                    }
                    continue;
                }
                if self.flags[i].return_at > 0 && self.tick >= self.flags[i].return_at {
                    self.flags[i].position = self.flags[i].home;
                    self.flags[i].return_at = 0;
                    self.emit("flag_returned", None, None, None, None);
                }
                for id in self.players.keys().cloned().collect::<Vec<_>>() {
                    let p = &self.players[&id];
                    if !p.alive || p.position.distance(self.flags[i].position) > 2.5 {
                        continue;
                    }
                    if p.team == self.flags[i].team {
                        if self.flags[i].return_at > 0 {
                            self.flags[i].position = self.flags[i].home;
                            self.flags[i].return_at = 0;
                            self.emit("flag_returned", Some(id), None, None, None);
                        }
                    } else if p.carrying.is_none() {
                        self.players.get_mut(&id).unwrap().carrying = Some(self.flags[i].team);
                        self.flags[i].carrier = Some(id.clone());
                        self.flags[i].return_at = 0;
                        self.emit("flag_taken", Some(id.clone()), None, None, None);
                        self.rules(EventKind::PlayerPickedUpItem, &id, None, None);
                        break;
                    }
                }
            }
            for id in self.players.keys().cloned().collect::<Vec<_>>() {
                let p = &self.players[&id];
                if p.carrying.is_none() || !p.alive {
                    continue;
                }
                if self.flags.iter().any(|f| {
                    f.team == p.team
                        && f.carrier.is_none()
                        && f.return_at == 0
                        && self.map.objects.iter().any(|o| {
                            o.kind == Primitive::FlagBase
                                && o.properties.team == Some(p.team)
                                && inside(o, p.position + Vec3::Y)
                        })
                }) {
                    let stolen = p.carrying.unwrap();
                    if let Some(f) = self.flags.iter_mut().find(|f| f.team == stolen) {
                        f.position = f.home;
                        f.carrier = None;
                        f.return_at = 0;
                    }
                    self.players.get_mut(&id).unwrap().carrying = None;
                    self.emit("flag_captured", Some(id.clone()), None, None, None);
                    self.rules(EventKind::FlagCaptured, &id, None, None);
                }
            }
        }
        if self.mode.objective == Objective::Hill {
            let occupants: Vec<_> =
                self.players
                    .values()
                    .filter(|p| {
                        p.alive
                            && self.map.objects.iter().any(|o| {
                                o.kind == Primitive::Hill && inside(o, p.position + Vec3::Y)
                            })
                    })
                    .map(|p| (p.id.clone(), p.team))
                    .collect();
            let teams: BTreeSet<_> = occupants.iter().map(|(_, t)| *t).collect();
            let contested = teams.len() > 1;
            let owner = if teams.len() == 1 {
                teams.first().copied()
            } else {
                None
            };
            if contested && !self.hill_contested {
                self.emit("hill_contested", None, None, None, None);
            }
            if owner.is_some() && owner != self.hill_owner {
                self.emit(
                    "hill_controlled",
                    occupants.first().map(|(id, _)| id.clone()),
                    None,
                    None,
                    None,
                );
            }
            self.hill_contested = contested;
            self.hill_owner = owner;
            if self.tick.is_multiple_of(60) && owner.is_some() {
                self.rules(EventKind::HillHeld, &occupants[0].0, None, None);
            }
        }
    }
    fn zone_events(&mut self) {
        let mut now = BTreeSet::new();
        let objects: Vec<_> = self
            .map
            .objects
            .iter()
            .filter(|o| {
                matches!(
                    o.kind,
                    Primitive::Hill | Primitive::ScoringZone | Primitive::FlagBase
                )
            })
            .cloned()
            .collect();
        for o in &objects {
            for p in self.players.values() {
                if p.alive && inside(o, p.position + Vec3::Y) {
                    now.insert((p.id.clone(), o.id.clone()));
                }
            }
        }
        let entered: Vec<_> = now.difference(&self.zones).cloned().collect();
        let left: Vec<_> = self.zones.difference(&now).cloned().collect();
        self.zones = now;
        for (pairs, event) in [
            (entered, EventKind::PlayerEnteredZone),
            (left, EventKind::PlayerLeftZone),
        ] {
            for (id, zone) in pairs {
                self.rules(event, &id, None, objects.iter().find(|o| o.id == zone));
            }
        }
    }
    fn rules(
        &mut self,
        event: EventKind,
        actor: &str,
        target: Option<&str>,
        zone: Option<&MapObject>,
    ) {
        // Effects do not recursively evaluate rules. ScoreChanged is queued once with a depth bound.
        self.rules_depth(event, actor, target, zone, 0);
    }
    fn rules_depth(
        &mut self,
        event: EventKind,
        actor: &str,
        target: Option<&str>,
        zone: Option<&MapObject>,
        depth: u8,
    ) {
        if depth > 1 {
            return;
        }
        let Some(p) = self.participant(actor) else {
            return;
        };
        let effects: Vec<_> = self
            .mode
            .rules
            .iter()
            .filter(|r| {
                r.event == event
                    && r.conditions.iter().all(|c| match c {
                        Condition::Opponent => {
                            target.and_then(|id| self.players.get(id)).is_some_and(|v| {
                                v.id != p.id && (!self.mode.teams || v.team != p.team)
                            })
                        }
                        Condition::Alive => p.alive,
                        Condition::CarryingFlag => p.carrying.is_some(),
                        Condition::Team { team } => p.team == *team,
                        Condition::ScoreAtLeast { value } => {
                            if self.mode.teams {
                                self.team_scores[p.team as usize] >= *value
                            } else {
                                p.score >= *value
                            }
                        }
                        Condition::TimeAtLeast { seconds } => {
                            self.tick.saturating_sub(self.start_at) >= u64::from(*seconds) * 60
                        }
                        Condition::ZoneOwned => {
                            zone.is_some_and(|z| z.properties.team == Some(p.team))
                                || self.hill_owner == Some(p.team)
                        }
                    })
            })
            .flat_map(|r| r.effects.clone())
            .collect();
        let mut changed = false;
        for effect in effects {
            match effect {
                Effect::AddScore { amount, team } => {
                    let p = self.participant_mut(actor).unwrap();
                    p.score = p.score.saturating_add(amount);
                    let player_team = p.team as usize;
                    if team {
                        self.team_scores[player_team] =
                            self.team_scores[player_team].saturating_add(amount);
                    }
                    changed = true;
                }
                Effect::EndMatch => self.finish(),
                Effect::Respawn => {
                    let team = self.participant(actor).unwrap().team;
                    let pos = self.spawn_position(team);
                    let p = self.participant_mut(actor).unwrap();
                    p.alive = true;
                    p.health = 100.;
                    p.shield = 100.;
                    p.position = pos;
                    p.velocity = Vec3::ZERO;
                }
                Effect::GiveWeapon { weapon } => {
                    let p = self.participant_mut(actor).unwrap();
                    if !p.inventory.contains(&weapon) {
                        p.inventory.push(weapon);
                    }
                    p.magazines[weapon as usize] = weapon.stats().magazine;
                }
                Effect::RemoveItem => self.drop_flag(actor),
                Effect::ResetFlags => self.reset_flags(),
                Effect::Announce { text } => {
                    self.emit(&text, Some(actor.into()), None, None, None);
                    self.events.back_mut().unwrap().announcement = true;
                }
            }
        }
        if changed {
            self.rules_depth(EventKind::ScoreChanged, actor, target, zone, depth + 1);
        }
    }
}
