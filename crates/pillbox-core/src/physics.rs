//! Box3D's capsule mover, convex narrow phase and plane solver. The immutable
//! collider cache is built once per arena, shared by server and WASM prediction.
use crate::{HEIGHT, Map, Primitive, RADIUS, Vec3};
use box3d_rust::{
    distance::{DistanceInput, SimplexCache, make_proxy, shape_distance},
    geometry::{Capsule, CollisionPlane, PlaneResult, collide_mover_and_hull},
    hull::{HullData, create_hull, make_box_hull},
    math_functions::{TRANSFORM_IDENTITY, Vec3 as BVec3},
    mover::{clip_vector, solve_planes},
};
fn b(v: Vec3) -> BVec3 {
    BVec3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}
fn g(v: BVec3) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}
fn rotate(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}
#[derive(Clone, Debug)]
struct Collider {
    hull: HullData,
    position: Vec3,
    yaw: f32,
    min: Vec3,
    max: Vec3,
}
#[derive(Clone, Debug, Default)]
pub struct PhysicsMap {
    colliders: Vec<Collider>,
}
impl PhysicsMap {
    pub fn new(map: &Map) -> Self {
        let mut result = Self::default();
        for o in map.objects.iter().filter(|o| o.kind.solid()) {
            let h = o.scale * 0.5;
            let mut add = |hull: HullData, offset: Vec3| {
                let position = o.position + rotate(offset, o.rotation);
                let mut min = Vec3::splat(f32::MAX);
                let mut max = Vec3::splat(f32::MIN);
                for p in &hull.points {
                    let w = position + rotate(g(*p), o.rotation);
                    min = min.min(w);
                    max = max.max(w);
                }
                result.colliders.push(Collider {
                    hull,
                    position,
                    yaw: o.rotation,
                    min,
                    max,
                });
            };
            match o.kind {
                Primitive::Arch => {
                    add(
                        make_box_hull(h.x * 0.175, h.y, h.z).base,
                        Vec3::new(-o.scale.x * 0.4125, 0., 0.),
                    );
                    add(
                        make_box_hull(h.x * 0.175, h.y, h.z).base,
                        Vec3::new(o.scale.x * 0.4125, 0., 0.),
                    );
                    add(
                        make_box_hull(h.x, h.y * 0.225, h.z).base,
                        Vec3::new(0., o.scale.y * 0.3875, 0.),
                    );
                }
                Primitive::Stairs => {
                    for i in 0..8 {
                        let height = o.scale.y * (i + 1) as f32 / 8.;
                        add(
                            make_box_hull(h.x, height * 0.5, o.scale.z / 16.).base,
                            Vec3::new(
                                0.,
                                -h.y + height * 0.5,
                                -h.z + o.scale.z * (i as f32 + 0.5) / 8.,
                            ),
                        );
                    }
                }
                Primitive::Ramp | Primitive::Wedge => {
                    let points = [
                        [-h.x, -h.y, -h.z],
                        [h.x, -h.y, -h.z],
                        [-h.x, -h.y, h.z],
                        [h.x, -h.y, h.z],
                        [-h.x, h.y, h.z],
                        [h.x, h.y, h.z],
                    ]
                    .map(|p| b(Vec3::from_array(p)));
                    add(create_hull(&points, 6).expect("valid wedge"), Vec3::ZERO);
                }
                Primitive::Cylinder | Primitive::Pillar | Primitive::Sphere => {
                    let mut points = vec![];
                    if o.kind == Primitive::Sphere {
                        points.push(b(Vec3::new(0., h.y, 0.)));
                        points.push(b(Vec3::new(0., -h.y, 0.)));
                        for ring in 1..6 {
                            let phi = std::f32::consts::PI * ring as f32 / 6.;
                            for i in 0..16 {
                                let a = std::f32::consts::TAU * i as f32 / 16.;
                                points.push(b(Vec3::new(
                                    h.x * phi.sin() * a.cos(),
                                    h.y * phi.cos(),
                                    h.z * phi.sin() * a.sin(),
                                )));
                            }
                        }
                    } else {
                        for y in [-h.y, h.y] {
                            for i in 0..16 {
                                let a = std::f32::consts::TAU * i as f32 / 16.;
                                points.push(b(Vec3::new(h.x * a.cos(), y, h.z * a.sin())));
                            }
                        }
                    }
                    add(
                        create_hull(&points, points.len() as i32).expect("valid convex primitive"),
                        Vec3::ZERO,
                    );
                }
                _ => add(make_box_hull(h.x, h.y, h.z).base, Vec3::ZERO),
            }
        }
        result
    }
    fn nearby(&self, feet: Vec3) -> impl Iterator<Item = &Collider> {
        let min = feet - Vec3::new(RADIUS, 0., RADIUS);
        let max = feet + Vec3::new(RADIUS, HEIGHT, RADIUS);
        self.colliders
            .iter()
            .filter(move |c| min.cmple(c.max).all() && max.cmpge(c.min).all())
    }
    fn capsule(c: &Collider, feet: Vec3) -> Capsule {
        let local = rotate(feet - c.position, -c.yaw);
        Capsule {
            center1: b(local + Vec3::Y * RADIUS),
            center2: b(local + Vec3::Y * (HEIGHT - RADIUS)),
            radius: RADIUS,
        }
    }
    pub fn clear(&self, feet: Vec3) -> bool {
        self.nearby(feet).all(|c| {
            let capsule = Self::capsule(c, feet);
            // Reject a capsule core that intersects the convex hull before GJK.
            // Box3D's zero-distance witness assertion loses precision for deeply
            // embedded segments. Plane clipping also avoids querying such cores
            // during navigation probes inside tall stacked construction.
            let start = g(capsule.center1);
            let delta = g(capsule.center2) - start;
            let mut lower = 0_f32;
            let mut upper = 1_f32;
            let mut intersects = true;
            for plane in &c.hull.planes {
                let normal = g(plane.normal);
                let distance = plane.offset + 0.000001 - normal.dot(start);
                let denominator = normal.dot(delta);
                if denominator.abs() < 0.000001 {
                    if distance < 0. {
                        intersects = false;
                        break;
                    }
                } else {
                    let fraction = distance / denominator;
                    if denominator < 0. {
                        lower = lower.max(fraction);
                    } else {
                        upper = upper.min(fraction);
                    }
                    if lower > upper {
                        intersects = false;
                        break;
                    }
                }
            }
            if intersects {
                return false;
            }
            let input = DistanceInput {
                proxy_a: make_proxy(&c.hull.points, 0.),
                proxy_b: make_proxy(&[capsule.center1, capsule.center2], RADIUS),
                transform: TRANSFORM_IDENTITY,
                use_radii: false,
            };
            shape_distance(&input, &mut SimplexCache::default(), None).distance >= RADIUS - 0.008
        })
    }
    fn planes(&self, feet: Vec3) -> Vec<CollisionPlane> {
        let mut planes = vec![];
        for c in self.nearby(feet) {
            let mut p = PlaneResult::default();
            if collide_mover_and_hull(&mut p, &c.hull, &Self::capsule(c, feet)) > 0 {
                p.plane.normal = b(rotate(g(p.plane.normal), c.yaw));
                planes.push(CollisionPlane {
                    plane: p.plane,
                    push_limit: f32::MAX,
                    push: 0.,
                    clip_velocity: true,
                });
            }
        }
        planes
    }
    /// Small capsule sweeps prevent the mover core from entering a hull (Box3D
    /// deliberately does not emit a plane for a deeply embedded capsule core).
    pub fn move_capsule(
        &self,
        feet: &mut Vec3,
        velocity: &mut Vec3,
        dt: f32,
        was_grounded: bool,
    ) -> bool {
        let steps = ((velocity.length() * dt / 0.1).ceil() as usize).clamp(1, 16);
        let mut grounded = false;
        for _ in 0..steps {
            let old = *feet;
            let candidate = old + *velocity * (dt / steps as f32);
            let mut planes = self.planes(candidate);
            let wall = planes.iter().any(|p| p.plane.normal.y < 0.55);
            if wall && was_grounded && velocity.y <= 0. {
                let raised = candidate + Vec3::Y * 0.42;
                if self.clear(raised) {
                    *feet = raised;
                    // Keep horizontal progress while the capsule's rounded foot
                    // crosses the riser. Immediate descent pushes it back down
                    // the rounded edge onto the approach floor on taller steps.
                    // Gravity settles it onto the tread on subsequent sweeps.
                    velocity.y = 0.;
                    continue;
                }
            }
            let correction = g(solve_planes(b(Vec3::ZERO), &mut planes).delta);
            *feet = candidate + correction;
            grounded |= planes
                .iter()
                .any(|p| p.plane.normal.y > 0.6 && velocity.y <= 0.5);
            *velocity = g(clip_vector(b(*velocity), &planes));
        }
        if was_grounded && !grounded && velocity.y <= 0. {
            // Follow descending slopes and small stairs instead of hopping over them.
            let down = *feet - Vec3::Y * 0.16;
            let mut planes = self.planes(down);
            if planes.iter().any(|p| p.plane.normal.y > 0.6) {
                *feet = down + g(solve_planes(b(Vec3::ZERO), &mut planes).delta);
                velocity.y = 0.;
                grounded = true;
            }
        }
        if grounded {
            velocity.y = velocity.y.max(0.);
        }
        grounded
    }
}
