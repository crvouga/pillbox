//! Bevy is presentation only; authoritative and predicted movement share pillbox-core.
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use bevy::{
    camera::visibility::RenderLayers,
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster},
};
use pillbox_core::{Input, Map, Player, Primitive, Snapshot, Weapon};
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[derive(Default)]
struct Bridge {
    map: Option<Map>,
    frame: Option<Frame>,
    prediction_map: Option<Map>,
    physics: Option<pillbox_core::PhysicsMap>,
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
    #[serde(default)]
    recoil: f32,
    #[serde(default)]
    ads: bool,
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
#[derive(Component)]
struct GunCamera;
#[derive(Component)]
struct HeldWeapon(Weapon);
#[derive(Component)]
struct RemoteWeapon {
    id: String,
    weapon: Weapon,
}
#[derive(Component)]
struct Flash(f32);
#[derive(Resource, Default)]
struct Effects {
    last_event: u64,
}
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
    b.physics = Some(pillbox_core::PhysicsMap::new(&map));
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
    if let (Some(map), Some(physics)) = (&b.prediction_map, &b.physics) {
        for (i, input) in inputs.into_iter().take(120).enumerate() {
            pillbox_core::move_player_with_physics(
                map,
                physics,
                &mut p,
                input,
                u64::from(tick) + i as u64 + 1,
            );
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
        physics,
        prediction,
        ..
    } = &mut *b;
    if let (Some(map), Some(physics), Some(p)) = (prediction_map, physics, prediction) {
        pillbox_core::move_player_with_physics(map, physics, p, input, u64::from(tick));
        return Ok(serde_json::to_string(p).unwrap());
    }
    Err("No local player".into())
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn run() {
    build_app(false).run();
}
fn build_app(headless: bool) -> App {
    let mut app = App::new();
    let plugins = DefaultPlugins
        .set(AssetPlugin {
            meta_check: bevy::asset::AssetMetaCheck::Never,
            file_path: if cfg!(target_arch = "wasm32") {
                ".".into()
            } else {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../web").into()
            },
            ..default()
        })
        .set(WindowPlugin {
            primary_window: (!headless).then_some(Window {
                title: "Pillbox".into(),
                resolution: bevy::window::WindowResolution::new(1280, 720)
                    .with_scale_factor_override(1.),
                canvas: Some("#game".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            exit_condition: bevy::window::ExitCondition::DontExit,
            ..default()
        });
    if headless {
        app.add_plugins(plugins.disable::<bevy::winit::WinitPlugin>());
    } else {
        app.add_plugins(plugins);
    }
    app.insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.16)))
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 500.,
            ..default()
        })
        .insert_resource(DirectionalLightShadowMap { size: 2048 })
        .init_resource::<Effects>()
        .init_resource::<Scene>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                receive,
                render_players,
                render_camera,
                render_weapon,
                remote_weapons,
                weapon_layers,
                render_pickups,
                combat_effects,
            )
                .chain(),
        );
    app
}
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Projection::Perspective(PerspectiveProjection {
            fov: 85_f32.to_radians(),
            near: 0.05,
            ..default()
        }),
        DistanceFog {
            color: Color::srgb(0.58, 0.65, 0.69),
            falloff: FogFalloff::Linear {
                start: 70.,
                end: 180.,
            },
            ..default()
        },
        Msaa::Off,
        Transform::from_xyz(34., 30., 34.).looking_at(Vec3::ZERO, Vec3::Y),
        ViewCamera,
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.,
            color: Color::srgb(1., 0.96, 0.9),
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, -0.5, 0.)),
        CascadeShadowConfigBuilder {
            num_cascades: 2,
            maximum_distance: 85.,
            ..default()
        }
        .build(),
        RenderLayers::from_layers(&[0, 1]),
    ));
    commands.spawn((
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 70_f32.to_radians(),
            near: 0.02,
            ..default()
        }),
        Transform::default(),
        RenderLayers::layer(1),
        Msaa::Off,
        GunCamera,
    ));
    commands.spawn((
        PointLight {
            intensity: 2500.,
            range: 6.,
            color: Color::srgb(0.8, 0.9, 1.),
            ..default()
        },
        Transform::from_xyz(0., 1., 2.),
        RenderLayers::layer(1),
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
    assets: Res<AssetServer>,
    background: (ResMut<ClearColor>, ResMut<Effects>),
) {
    let (mut clear, mut effects) = background;
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
    effects.last_event = 0;
    // Cache a small shared mesh palette for every map rebuild.
    let cube = meshes.add(Cuboid::new(1., 1., 1.));
    let sphere = meshes.add(Sphere::new(0.5));
    let cylinder = meshes.add(Cylinder::new(0.5, 1.));
    let ramp = meshes.add(wedge());
    // Share surface handles so repeated kit meshes can be instanced together.
    let mut surface_materials = std::collections::BTreeMap::new();
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
        let material = surface_materials
            .entry(o.properties.color.clone())
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: tint,
                    perceptual_roughness: 0.9,
                    metallic: 0.,
                    emissive: LinearRgba::BLACK,
                    ..default()
                })
            })
            .clone();
        let base = Vec3::from_array(o.position.to_array());
        let scale = Vec3::from_array(o.scale.to_array());
        let rotation = Quat::from_rotation_y(o.rotation);
        if o.kind == Primitive::Light {
            commands.spawn((
                PointLight {
                    color: tint,
                    intensity: 9000.,
                    range: 13.,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(base),
                Geometry {
                    id: o.id.clone(),
                    kind: o.kind,
                },
            ));
        }
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
            | Primitive::ShieldPickup => {
                if o.kind == Primitive::WeaponSpawn {
                    commands.spawn((
                        SceneRoot(assets.load(weapon_path(o.properties.weapon))),
                        Transform::from_translation(base)
                            .with_rotation(Quat::from_rotation_y(0.5))
                            .with_scale(Vec3::splat(0.65)),
                        Geometry {
                            id: o.id.clone(),
                            kind: o.kind,
                        },
                    ));
                } else {
                    spawn(cube.clone(), Vec3::ZERO, Vec3::splat(0.55));
                }
            }
            Primitive::JumpPad
            | Primitive::TeleporterEntrance
            | Primitive::SpeedBoost
            | Primitive::GravityModifier => spawn(
                cube.clone(),
                Vec3::new(0., -scale.y * 0.5 + 0.06, 0.),
                Vec3::new(scale.x, 0.12, scale.z),
            ),
            _ => (),
        }
    }
}
fn render_players(
    mut commands: Commands,
    scene: Res<Scene>,
    palette: Res<Palette>,
    mut pills: Query<(Entity, &Pill, &mut Transform, &mut Visibility)>,
    dynamic: Query<Entity, With<Dynamic>>,
    assets: Res<AssetServer>,
) {
    let Some(frame) = &scene.frame else {
        return;
    };
    let Some(state) = &frame.state else {
        for (e, _, _, _) in &mut pills {
            commands.entity(e).despawn();
        }
        for e in &dynamic {
            commands.entity(e).despawn();
        }
        return;
    };
    for (entity, pill, mut transform, mut visibility) in &mut pills {
        if let Some(p) = state.players.iter().find(|p| p.id == pill.0) {
            transform.translation = Vec3::from_array(p.position.to_array()) + Vec3::Y * 0.9;
            transform.rotation = Quat::from_rotation_y(-p.yaw);
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
            let body = commands
                .spawn((
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
                ))
                .id();
            let visor = commands
                .spawn((
                    Mesh3d(palette.rocket.clone()),
                    MeshMaterial3d(palette.glow.clone()),
                    Transform::from_xyz(0., 0.45, -0.34).with_scale(Vec3::new(1.9, 0.65, 0.55)),
                ))
                .id();
            let gun = commands
                .spawn((
                    SceneRoot(assets.load(weapon_path(p.weapon))),
                    RemoteWeapon {
                        id: p.id.clone(),
                        weapon: p.weapon,
                    },
                    Transform::from_xyz(0.42, -0.08, -0.3)
                        .with_scale(Vec3::splat(0.55))
                        .with_rotation(Quat::IDENTITY),
                ))
                .id();
            commands.entity(body).add_children(&[visor, gun]);
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
fn render_camera(
    scene: Res<Scene>,
    mut camera: Query<(&mut Transform, &mut Projection), With<ViewCamera>>,
) {
    let Some(frame) = &scene.frame else {
        return;
    };
    let Ok((mut camera, mut projection)) = camera.single_mut() else {
        return;
    };
    if let Projection::Perspective(p) = &mut *projection {
        p.fov = if frame.ads || frame.camera.is_some() {
            50_f32.to_radians()
        } else {
            85_f32.to_radians()
        };
    }
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

fn weapon_path(w: Weapon) -> String {
    let model = match w {
        Weapon::Pistol => "i",
        Weapon::Rifle => "a",
        Weapon::Precision => "p",
        Weapon::Shotgun => "m",
        Weapon::Sniper => "f",
        Weapon::Rocket => "e",
    };
    format!("assets/models/weapons/blaster-{model}.glb#Scene0")
}
fn render_weapon(
    mut commands: Commands,
    scene: Res<Scene>,
    assets: Res<AssetServer>,
    time: Res<Time>,
    mut guns: Query<(Entity, &HeldWeapon, &mut Transform, &mut Visibility)>,
) {
    let player = scene.frame.as_ref().and_then(|f| f.predicted.as_ref());
    let visible =
        player.is_some_and(|p| p.alive) && scene.frame.as_ref().is_some_and(|f| f.camera.is_none());
    let Some(p) = player else {
        for (_, _, _, mut v) in &mut guns {
            *v = Visibility::Hidden;
        }
        return;
    };
    let frame = scene.frame.as_ref().unwrap();
    let bob = if p.grounded {
        p.velocity.length().min(8.) * 0.0015
    } else {
        0.
    };
    let t = time.elapsed_secs();
    let reload = if p.reload_at > 0 { 0.65 } else { 0. };
    let recoil = frame.recoil;
    let transform = Transform::from_xyz(
        if frame.ads { 0.05 } else { 0.26 } + bob * (t * 10.).cos(),
        -0.22 - reload * 0.15 + bob * (t * 20.).sin(),
        (if p.weapon == Weapon::Rocket {
            -1.05
        } else {
            -0.6
        }) + recoil * 0.09,
    )
    .with_rotation(Quat::from_euler(
        EulerRot::XYZ,
        reload - recoil * 0.13,
        0.,
        0.,
    ))
    .with_scale(Vec3::splat(0.62));
    let mut exists = false;
    for (e, g, mut tr, mut v) in &mut guns {
        if g.0 != p.weapon {
            commands.entity(e).despawn();
        } else {
            exists = true;
            *tr = transform;
            *v = if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
        }
    }
    if !exists {
        commands.spawn((
            SceneRoot(assets.load(weapon_path(p.weapon))),
            transform,
            if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            HeldWeapon(p.weapon),
            RenderLayers::layer(1),
            NotShadowCaster,
        ));
    }
}
fn weapon_layers(
    mut commands: Commands,
    meshes: Query<Entity, (With<Mesh3d>, Without<RenderLayers>)>,
    parents: Query<&ChildOf>,
    guns: Query<(), With<HeldWeapon>>,
) {
    for e in &meshes {
        let mut current = e;
        for _ in 0..12 {
            if guns.contains(current) {
                commands
                    .entity(e)
                    .insert((RenderLayers::layer(1), NotShadowCaster));
                break;
            }
            let Ok(parent) = parents.get(current) else {
                break;
            };
            current = parent.parent();
        }
    }
}
fn combat_effects(
    mut commands: Commands,
    scene: Res<Scene>,
    mut effects: ResMut<Effects>,
    time: Res<Time>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut flashes: Query<(Entity, &mut Flash, &mut Transform)>,
) {
    for (e, mut f, mut t) in &mut flashes {
        f.0 -= time.delta_secs();
        if f.0 <= 0. {
            commands.entity(e).despawn();
        } else {
            t.scale *= 0.92;
        }
    }
    let Some(state) = scene.frame.as_ref().and_then(|f| f.state.as_ref()) else {
        return;
    };
    for event in &state.events {
        if event.id <= effects.last_event {
            continue;
        }
        effects.last_event = event.id;
        if let Some(pos) = event.position {
            let pos = Vec3::from_array(pos.to_array());
            if event.kind == "shot" {
                let Some(p) = state
                    .players
                    .iter()
                    .find(|p| Some(&p.id) == event.actor.as_ref())
                else {
                    continue;
                };
                let aim = Vec3::new(
                    p.yaw.sin() * p.pitch.cos(),
                    p.pitch.sin(),
                    -p.yaw.cos() * p.pitch.cos(),
                );
                let aim = event
                    .endpoint
                    .map(|end| (Vec3::from_array(end.to_array()) - pos).normalize_or_zero())
                    .unwrap_or(aim);
                let len = if event.weapon == Some(Weapon::Rocket) {
                    1.5
                } else {
                    event
                        .endpoint
                        .map(|end| Vec3::from_array(end.to_array()).distance(pos))
                        .unwrap_or(12.)
                };
                commands.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.025, 0.025, len))),
                    MeshMaterial3d(palette.glow.clone()),
                    Transform::from_translation(pos + aim * len * 0.5).looking_to(aim, Vec3::Y),
                    Flash(0.085),
                ));
            }
            if event.kind == "explosion" {
                commands.spawn((
                    Mesh3d(palette.rocket.clone()),
                    MeshMaterial3d(palette.glow.clone()),
                    Transform::from_translation(pos).with_scale(Vec3::splat(14.)),
                    Flash(0.28),
                ));
            }
        }
    }
}

fn remote_weapons(
    scene: Res<Scene>,
    assets: Res<AssetServer>,
    mut weapons: Query<(&mut RemoteWeapon, &mut SceneRoot)>,
) {
    let Some(state) = scene.frame.as_ref().and_then(|f| f.state.as_ref()) else {
        return;
    };
    for (mut tag, mut root) in &mut weapons {
        if let Some(p) = state.players.iter().find(|p| p.id == tag.id)
            && tag.weapon != p.weapon
        {
            tag.weapon = p.weapon;
            root.0 = assets.load(weapon_path(p.weapon));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct Capture {
    path: String,
    image: Option<Handle<Image>>,
    frames: u32,
}
/// Render the actual client scene to a GPU image, without opening a window.
/// Used to verify asset loading, shadows, guns and scene registration in CI/local.
#[cfg(not(target_arch = "wasm32"))]
pub fn capture(path: &str) {
    let mut app = build_app(true);
    app.insert_resource(Capture {
        path: path.into(),
        image: None,
        frames: 0,
    })
    .add_plugins(bevy::app::ScheduleRunnerPlugin::run_loop(
        std::time::Duration::from_secs_f64(1. / 60.),
    ))
    .add_systems(Startup, capture_target.after(setup))
    .add_systems(Update, capture_frame)
    .run();
}
#[cfg(not(target_arch = "wasm32"))]
fn capture_target(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut capture: ResMut<Capture>,
    cameras: Query<Entity, With<Camera3d>>,
) {
    let mut image = Image::new_target_texture(
        1280,
        720,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
    let handle = images.add(image);
    for camera in &cameras {
        commands
            .entity(camera)
            .insert(bevy::camera::RenderTarget::Image(handle.clone().into()));
    }
    capture.image = Some(handle);
}
#[cfg(not(target_arch = "wasm32"))]
fn capture_frame(
    mut commands: Commands,
    mut capture: ResMut<Capture>,
    assets: Res<AssetServer>,
    mut exit: MessageWriter<AppExit>,
    scenes: Query<&SceneRoot>,
) {
    capture.frames += 1;
    if capture.frames >= 120
        && capture.image.is_some()
        && scenes
            .iter()
            .all(|root| assets.is_loaded_with_dependencies(&root.0))
    {
        let handle = capture.image.take().unwrap();
        commands
            .spawn(bevy::render::view::screenshot::Screenshot::image(handle))
            .observe(bevy::render::view::screenshot::save_to_disk(
                capture.path.clone(),
            ));
    }
    if capture.frames >= 150
        && capture.image.is_none()
        && std::path::Path::new(&capture.path).exists()
    {
        exit.write(AppExit::Success);
    }
    if capture.frames > 600 {
        panic!("GPU capture did not finish or assets failed to load");
    }
}
