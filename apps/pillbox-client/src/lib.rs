//! Bevy is presentation only; authoritative and predicted movement share pillbox-core.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use pillbox_core::{Input, Map, Player, Primitive, Snapshot};
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[derive(Default)]
struct Bridge {
    map: Option<Map>,
    frame: Option<Frame>,
    prediction_map: Option<Map>,
    prediction: Option<Player>,
}
static BRIDGE: OnceLock<Mutex<Bridge>> = OnceLock::new();
fn bridge() -> &'static Mutex<Bridge> {
    BRIDGE.get_or_init(|| Mutex::new(Bridge::default()))
}
#[derive(Clone, Deserialize)]
struct Frame {
    state: Option<Snapshot>,
    local_id: String,
    predicted: Option<Player>,
    camera: Option<[f32; 3]>,
    target: Option<[f32; 3]>,
}
#[derive(Resource, Default)]
struct Scene {
    frame: Option<Frame>,
}
#[derive(Component)]
struct Geometry {
    id: String,
    kind: Primitive,
}
#[derive(Component)]
struct Pill(String);
#[derive(Component)]
struct Dynamic;
#[derive(Component)]
struct ViewCamera;
#[derive(Resource)]
struct Palette {
    pill: Handle<Mesh>,
    blue: Handle<StandardMaterial>,
    orange: Handle<StandardMaterial>,
    rocket: Handle<Mesh>,
    glow: Handle<StandardMaterial>,
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn set_map(json: &str) -> Result<(), String> {
    let map: Map = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut b = bridge().lock().unwrap();
    b.prediction_map = Some(map.clone());
    b.map = Some(map);
    Ok(())
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn set_frame(json: &str) -> Result<(), String> {
    bridge().lock().unwrap().frame = Some(serde_json::from_str(json).map_err(|e| e.to_string())?);
    Ok(())
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn reconcile(player: &str, pending: &str, tick: u32) -> Result<String, String> {
    let mut p: Player = serde_json::from_str(player).map_err(|e| e.to_string())?;
    let inputs: Vec<Input> = serde_json::from_str(pending).map_err(|e| e.to_string())?;
    let mut b = bridge().lock().unwrap();
    if let Some(map) = &b.prediction_map {
        for (i, input) in inputs.into_iter().take(120).enumerate() {
            pillbox_core::move_player(map, &mut p, input, u64::from(tick) + i as u64 + 1);
        }
    }
    let result = serde_json::to_string(&p).unwrap();
    b.prediction = Some(p);
    Ok(result)
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn predict(input: &str, tick: u32) -> Result<String, String> {
    let input: Input = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let mut b = bridge().lock().unwrap();
    let Bridge {
        prediction_map,
        prediction,
        ..
    } = &mut *b;
    if let (Some(map), Some(p)) = (prediction_map, prediction) {
        pillbox_core::move_player(map, p, input, u64::from(tick));
        return Ok(serde_json::to_string(p).unwrap());
    }
    Err("No local player".into())
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Pillbox".into(),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.16)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 500.,
            ..default()
        })
        .init_resource::<Scene>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (receive, render_players, render_camera, render_pickups).chain(),
        )
        .run();
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Msaa::Off,
        Transform::from_xyz(34., 30., 34.).looking_at(Vec3::ZERO, Vec3::Y),
        ViewCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.6, 0.)),
    ));
    commands.insert_resource(Palette {
        pill: meshes.add(Capsule3d::new(0.4, 1.0)),
        blue: materials.add(Color::srgb_u8(74, 226, 239)),
        orange: materials.add(Color::srgb_u8(255, 126, 100)),
        rocket: meshes.add(Sphere::new(0.16)),
        glow: materials.add(StandardMaterial {
            base_color: Color::srgb_u8(255, 218, 98),
            unlit: true,
            ..default()
        }),
    });
}
fn color(s: &str) -> Color {
    Color::Srgba(Srgba::hex(s).unwrap_or(Srgba::new(0.5, 0.4, 0.9, 1.)))
}
fn wedge() -> Mesh {
    let v = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [0.5, 0.5, 0.5],
    ];
    let faces = [
        [0, 4, 1],
        [1, 4, 5],
        [0, 2, 4],
        [1, 5, 3],
        [2, 3, 4],
        [3, 5, 4],
        [0, 1, 2],
        [1, 3, 2],
    ];
    let mut positions = vec![];
    let mut normals = vec![];
    for f in faces {
        let a = Vec3::from(v[f[0]]);
        let b = Vec3::from(v[f[1]]);
        let c = Vec3::from(v[f[2]]);
        let n = (b - a).cross(c - a).normalize().to_array();
        for i in f {
            positions.push(v[i]);
            normals.push(n);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.]; 24])
    .with_inserted_indices(Indices::U32((0..24).collect()))
}
fn receive(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    geometry: Query<Entity, With<Geometry>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clear: ResMut<ClearColor>,
) {
    let mut bridge = bridge().lock().unwrap();
    if let Some(frame) = bridge.frame.take() {
        scene.frame = Some(frame);
    }
    let Some(map) = bridge.map.take() else {
        return;
    };
    for e in &geometry {
        commands.entity(e).despawn();
    }
    clear.0 = color(&map.skybox);
    // Cache a small shared mesh palette for every map rebuild.
    let cube = meshes.add(Cuboid::new(1., 1., 1.));
    let sphere = meshes.add(Sphere::new(0.5));
    let cylinder = meshes.add(Cylinder::new(0.5, 1.));
    let ramp = meshes.add(wedge());
    for o in map.objects {
        let tint = if o.kind.spawn() {
            if o.properties.team == Some(1) {
                Color::srgb_u8(255, 126, 100)
            } else {
                Color::srgb_u8(74, 226, 239)
            }
        } else {
            color(&o.properties.color)
        };
        let material = materials.add(StandardMaterial {
            base_color: tint,
            perceptual_roughness: 0.9,
            unlit: !o.kind.solid(),
            ..default()
        });
        let base = Vec3::from_array(o.position.to_array());
        let scale = Vec3::from_array(o.scale.to_array());
        let rotation = Quat::from_rotation_y(o.rotation);
        let mut spawn = |mesh: Handle<Mesh>, offset: Vec3, size: Vec3| {
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform {
                    translation: base + rotation * offset,
                    rotation,
                    scale: size,
                },
                Geometry {
                    id: o.id.clone(),
                    kind: o.kind,
                },
            ));
        };
        match o.kind {
            Primitive::Arch => {
                spawn(
                    cube.clone(),
                    Vec3::new(-scale.x * 0.4125, 0., 0.),
                    Vec3::new(scale.x * 0.175, scale.y, scale.z),
                );
                spawn(
                    cube.clone(),
                    Vec3::new(scale.x * 0.4125, 0., 0.),
                    Vec3::new(scale.x * 0.175, scale.y, scale.z),
                );
                spawn(
                    cube.clone(),
                    Vec3::new(0., scale.y * 0.3875, 0.),
                    Vec3::new(scale.x, scale.y * 0.225, scale.z),
                );
            }
            Primitive::Stairs => {
                for i in 0..8 {
                    let h = scale.y * (i + 1) as f32 / 8.;
                    spawn(
                        cube.clone(),
                        Vec3::new(
                            0.,
                            -scale.y * 0.5 + h * 0.5,
                            -scale.z * 0.5 + scale.z * (i as f32 + 0.5) / 8.,
                        ),
                        Vec3::new(scale.x, h, scale.z / 8.),
                    );
                }
            }
            Primitive::Ramp | Primitive::Wedge => spawn(ramp.clone(), Vec3::ZERO, scale),
            Primitive::Sphere => spawn(sphere.clone(), Vec3::ZERO, scale),
            Primitive::Cylinder | Primitive::Pillar => spawn(cylinder.clone(), Vec3::ZERO, scale),
            Primitive::Flag => {
                spawn(
                    cylinder.clone(),
                    Vec3::new(0., 0.9, 0.),
                    Vec3::new(0.12, 2., 0.12),
                );
                spawn(
                    cube.clone(),
                    Vec3::new(0.45, 1.6, 0.),
                    Vec3::new(0.9, 0.5, 0.08),
                );
                spawn(
                    cylinder.clone(),
                    Vec3::ZERO,
                    Vec3::new(scale.x, 0.1, scale.z),
                );
            }
            Primitive::FlagBase => spawn(
                cylinder.clone(),
                Vec3::ZERO,
                Vec3::new(scale.x, 0.12, scale.z),
            ),
            kind if kind.solid() => spawn(cube.clone(), Vec3::ZERO, scale),
            Primitive::WeaponSpawn
            | Primitive::AmmoPickup
            | Primitive::HealthPickup
            | Primitive::ShieldPickup => spawn(sphere.clone(), Vec3::ZERO, Vec3::splat(0.65)),
            _ => spawn(
                cube.clone(),
                Vec3::new(0., -scale.y * 0.5 + 0.06, 0.),
                Vec3::new(scale.x, 0.12, scale.z),
            ),
        }
    }
}
fn render_players(
    mut commands: Commands,
    scene: Res<Scene>,
    palette: Res<Palette>,
    mut pills: Query<(Entity, &Pill, &mut Transform, &mut Visibility)>,
    dynamic: Query<Entity, With<Dynamic>>,
) {
    let Some(frame) = &scene.frame else {
        return;
    };
    let Some(state) = &frame.state else {
        for (e, _, _, _) in &mut pills {
            commands.entity(e).despawn();
        }
        return;
    };
    for (entity, pill, mut transform, mut visibility) in &mut pills {
        if let Some(p) = state.players.iter().find(|p| p.id == pill.0) {
            transform.translation = Vec3::from_array(p.position.to_array()) + Vec3::Y * 0.9;
            *visibility = if p.alive && p.id != frame.local_id {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        } else {
            commands.entity(entity).despawn();
        }
    }
    for p in &state.players {
        if !pills.iter().any(|(_, tag, _, _)| tag.0 == p.id) {
            commands.spawn((
                Mesh3d(palette.pill.clone()),
                MeshMaterial3d(if p.team == 0 {
                    palette.blue.clone()
                } else {
                    palette.orange.clone()
                }),
                Transform::from_translation(
                    Vec3::from_array(p.position.to_array()) + Vec3::Y * 0.9,
                ),
                if p.alive && p.id != frame.local_id {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
                Pill(p.id.clone()),
            ));
        }
    }
    for e in &dynamic {
        commands.entity(e).despawn();
    }
    for r in &state.projectiles {
        commands.spawn((
            Mesh3d(palette.rocket.clone()),
            MeshMaterial3d(palette.glow.clone()),
            Transform::from_translation(Vec3::from_array(r.position.to_array())),
            Dynamic,
        ));
    }
    for f in &state.flags {
        commands.spawn((
            Mesh3d(palette.rocket.clone()),
            MeshMaterial3d(if f.team == 0 {
                palette.blue.clone()
            } else {
                palette.orange.clone()
            }),
            Transform::from_translation(Vec3::from_array(f.position.to_array()) + Vec3::Y * 2.2)
                .with_scale(Vec3::splat(2.5)),
            Dynamic,
        ));
    }
}
fn render_camera(scene: Res<Scene>, mut camera: Query<&mut Transform, With<ViewCamera>>) {
    let Some(frame) = &scene.frame else {
        return;
    };
    let Ok(mut camera) = camera.single_mut() else {
        return;
    };
    if let Some(position) = frame.camera {
        *camera = Transform::from_translation(Vec3::from_array(position)).looking_at(
            Vec3::from_array(frame.target.unwrap_or([0., 0., 0.])),
            Vec3::Y,
        );
    } else if let Some(p) = &frame.predicted {
        let eye = Vec3::from_array(p.position.to_array()) + Vec3::Y * 1.5;
        let aim = Vec3::new(
            p.yaw.sin() * p.pitch.cos(),
            p.pitch.sin(),
            -p.yaw.cos() * p.pitch.cos(),
        );
        *camera = Transform::from_translation(eye).looking_at(eye + aim, Vec3::Y);
    }
}

fn render_pickups(scene: Res<Scene>, mut geometry: Query<(&Geometry, &mut Visibility)>) {
    for (object, mut visibility) in &mut geometry {
        let hidden = scene
            .frame
            .as_ref()
            .and_then(|f| f.state.as_ref())
            .is_some_and(|state| {
                state
                    .pickups
                    .get(&object.id)
                    .is_some_and(|until| *until > state.tick)
                    || object.kind == Primitive::Flag
            });
        *visibility = if hidden {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}
