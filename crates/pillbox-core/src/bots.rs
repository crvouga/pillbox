//! Deterministic server opponents. They use the same inputs, weapons, collision,
//! damage and objectives as humans; a surface graph lets them traverse ramps.
use crate::*;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
};
#[derive(Clone, Copy)]
struct Visit {
    cost: f32,
    index: usize,
}
impl PartialEq for Visit {
    fn eq(&self, other: &Self) -> bool {
        self.cost.to_bits() == other.cost.to_bits() && self.index == other.index
    }
}
impl Eq for Visit {}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then_with(|| other.index.cmp(&self.index))
    }
}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Clone, Debug, Default)]
pub struct Navigation {
    pub nodes: Vec<Vec3>,
    edges: Vec<Vec<usize>>,
}
impl Navigation {
    pub fn new(map: &Map, physics: &PhysicsMap) -> Self {
        let mut nodes = vec![];
        let mut low = Vec3::splat(48.);
        let mut high = Vec3::splat(-48.);
        for o in map.objects.iter().filter(|o| o.kind.solid()) {
            let radius = o.scale.x.max(o.scale.z) * 0.5;
            low = low.min(o.position - Vec3::splat(radius));
            high = high.max(o.position + Vec3::splat(radius));
        }
        let cells =
            |a: f32, b: f32| (a.max(-48.) / 2.).floor() as i32..=(b.min(48.) / 2.).ceil() as i32;
        'cells: for x in cells(low.x, high.x) {
            for z in cells(low.z, high.z) {
                let p = Vec3::new(x as f32 * 2., 0., z as f32 * 2.);
                let mut heights = vec![];
                for o in map.objects.iter().filter(|o| o.kind.solid()) {
                    let (s, c) = (-o.rotation).sin_cos();
                    let v = p - o.position;
                    let local = Vec3::new(c * v.x + s * v.z, 0., -s * v.x + c * v.z);
                    if let Some(top) = crate::simulation::local_top(o, local, 0.001) {
                        heights.push(top + o.position.y);
                    }
                }
                heights.sort_by(f32::total_cmp);
                heights.dedup_by(|a, b| (*a - *b).abs() < 0.05);
                // Creator maps can stack hundreds of platforms. Bound both
                // surface probes and the spatial edge graph independently
                // of hostile-but-valid geometry; bots are a local-arena aid.
                let stride = heights.len().div_ceil(8).max(1);
                for h in heights.into_iter().step_by(stride) {
                    let point = Vec3::new(p.x, h + 0.06, p.z);
                    if physics.clear(point) {
                        nodes.push(point);
                        if nodes.len() >= 4096 {
                            break 'cells;
                        }
                    }
                }
            }
        }
        let mut edges = vec![vec![]; nodes.len()];
        let mut columns: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
        for (i, p) in nodes.iter().enumerate() {
            columns
                .entry(((p.x / 2.).round() as i32, (p.z / 2.).round() as i32))
                .or_default()
                .push(i);
        }
        for (i, a) in nodes.iter().enumerate() {
            let cell = ((a.x / 2.).round() as i32, (a.z / 2.).round() as i32);
            let neighbors = (-1..=1).flat_map(|x| (-1..=1).map(move |z| (cell.0 + x, cell.1 + z)));
            for j in neighbors
                .filter_map(|key| columns.get(&key))
                .flatten()
                .copied()
                .filter(|j| *j > i)
            {
                let b = &nodes[j];
                let d = *b - *a;
                if d.x.abs() <= 2.01
                    && d.z.abs() <= 2.01
                    && d.y.abs() <= 1.05
                    && d.length_squared() < 9.5
                    && (1..8).all(|n| physics.clear(a.lerp(*b, n as f32 / 8.) + Vec3::Y * 0.4))
                {
                    edges[i].push(j);
                    edges[j].push(i);
                }
            }
        }
        Self { nodes, edges }
    }
    fn nearest(&self, p: Vec3) -> Option<usize> {
        self.nodes
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.distance_squared(p).total_cmp(&b.distance_squared(p)))
            .map(|(i, _)| i)
    }
    pub fn route(&self, from: Vec3, to: Vec3) -> Vec<Vec3> {
        let (Some(start), Some(end)) = (self.nearest(from), self.nearest(to)) else {
            return vec![];
        };
        let mut costs = vec![f32::MAX; self.nodes.len()];
        let mut previous = vec![usize::MAX; self.nodes.len()];
        let mut closed = vec![false; self.nodes.len()];
        costs[start] = 0.;
        let mut queue = BinaryHeap::new();
        queue.push(Visit {
            cost: 0.,
            index: start,
        });
        while let Some(Visit { index: i, .. }) = queue.pop() {
            if closed[i] {
                continue;
            }
            if i == end {
                let mut path = vec![self.nodes[end]];
                let mut j = end;
                while j != start {
                    j = previous[j];
                    if j == usize::MAX {
                        return vec![];
                    }
                    path.push(self.nodes[j]);
                }
                path.reverse();
                return path;
            }
            closed[i] = true;
            for j in &self.edges[i] {
                let cost = costs[i] + self.nodes[i].distance(self.nodes[*j]);
                if cost < costs[*j] {
                    costs[*j] = cost;
                    previous[*j] = i;
                    queue.push(Visit {
                        cost: cost + self.nodes[*j].distance(self.nodes[end]),
                        index: *j,
                    });
                }
            }
        }
        vec![]
    }
}
#[derive(Clone, Debug, Default)]
pub struct BotBrain {
    path: Vec<Vec3>,
    next_plan: u64,
    stuck: u32,
    last: Vec3,
}
impl World {
    pub fn bot_inputs(&self, brains: &mut BTreeMap<String, BotBrain>) -> BTreeMap<String, Input> {
        let mut inputs = BTreeMap::new();
        for (id, brain) in brains {
            let Some(p) = self.players.get(id).filter(|p| p.alive) else {
                continue;
            };
            let target = self
                .players
                .values()
                .filter(|q| q.id != *id && q.alive && (!self.mode.teams || q.team != p.team))
                .min_by(|a, b| {
                    a.position
                        .distance_squared(p.position)
                        .total_cmp(&b.position.distance_squared(p.position))
                });
            let Some(target) = target else {
                continue;
            };
            let goal = if self.mode.objective == Objective::Hill {
                self.map
                    .objects
                    .iter()
                    .find(|o| o.kind == Primitive::Hill)
                    .map(|o| o.position)
                    .unwrap_or(target.position)
            } else if self.mode.objective == Objective::Flags {
                self.flags
                    .iter()
                    .find(|f| {
                        f.team
                            == if p.carrying.is_some() {
                                p.team
                            } else {
                                1 - p.team
                            }
                    })
                    .map(|f| f.position)
                    .unwrap_or(target.position)
            } else {
                target.position
            };
            if self.tick >= brain.next_plan {
                brain.path = self.navigation.route(p.position, goal);
                brain.next_plan = self.tick + 90;
            }
            while brain.path.first().is_some_and(|point| {
                let d = *point - p.position;
                d.x * d.x + d.z * d.z < 0.5 && d.y.abs() < 0.5
            }) {
                brain.path.remove(0);
            }
            let eye = p.position + Vec3::Y * 1.5;
            let delta = target.position + Vec3::Y * 1.15 - eye;
            let distance = delta.length();
            let visible = distance < 38.
                && crate::simulation::wall_distance(
                    &self.map,
                    eye,
                    delta.normalize_or_zero(),
                    distance,
                ) >= distance - 0.1;
            let move_to = brain.path.first().copied().unwrap_or(goal) - p.position;
            let aim = if visible { delta } else { move_to };
            let desired = aim.x.atan2(-aim.z);
            let error = (desired - p.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let yaw = p.yaw + error.clamp(-0.07, 0.07);
            let pitch = if visible {
                (delta.y / distance.max(0.1)).asin() + 0.018 * (self.tick as f32 * 0.06).sin()
            } else {
                0.
            };
            let movement = if visible && distance < 10. {
                Vec3::new((self.tick as f32 * 0.017).sin(), 0., 0.)
            } else {
                let world = Vec3::new(move_to.x, 0., move_to.z).normalize_or_zero();
                let (s, c) = yaw.sin_cos();
                Vec3::new(c * world.x + s * world.z, 0., -s * world.x + c * world.z)
            };
            brain.stuck = if p.position.distance_squared(brain.last) < 0.001 {
                brain.stuck + 1
            } else {
                0
            };
            brain.last = p.position;
            inputs.insert(
                id.clone(),
                Input {
                    seq: p.ack + 1,
                    x: movement.x.clamp(-1., 1.),
                    z: movement.z.clamp(-1., 1.),
                    yaw,
                    pitch: pitch.clamp(-1.5, 1.5),
                    fire: visible && error.abs() < 0.12 && self.tick % 120 > 25,
                    reload: p.ammo() == 0,
                    weapon: Some(Weapon::Rifle),
                    jump: brain.stuck > 25 && p.grounded,
                    view_tick: self.tick,
                },
            );
        }
        inputs
    }
}
