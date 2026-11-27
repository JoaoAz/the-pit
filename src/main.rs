//! THE PIT - scale prototype.
//!
//! Run without arguments to start in VR (OpenXR, e.g. Quest 3 over Link).
//! `--desktop` opens a normal window with a mouse-look camera.
//! `--shot <file.png> --view <up|down|wide|ring|clinical>` renders one desktop frame and exits.

mod meshbuild;
mod shaft;
mod xr_input;

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{CursorGrabMode, CursorOptions};
use shaft::{Mats, MovingRing, ShaftPart, SIZES};

#[derive(Resource)]
struct Args {
    desktop: bool,
    shot: Option<String>,
    view: String,
}

/// 0.0 = mechanical/oppressive, 1.0 = clinical/high-tech.
#[derive(Resource)]
pub struct Mood {
    pub current: f32,
    pub target: f32,
}

#[derive(Resource)]
pub struct Size(pub usize);

#[derive(Resource)]
pub struct RingMotion {
    pub moving: bool,
    pub dir: f32,
}

#[derive(Resource)]
struct Materials {
    m: Mats,
}

#[derive(Component)]
struct DesktopCam {
    yaw: f32,
    pitch: f32,
}

#[derive(Resource, Default)]
struct FrameCount(u32);

fn parse_args() -> Args {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1).cloned());
    let shot = get("--shot");
    Args {
        desktop: a.iter().any(|x| x == "--desktop") || shot.is_some(),
        view: get("--view").unwrap_or_else(|| "wide".into()),
        shot,
    }
}

fn main() -> AppExit {
    let args = parse_args();
    let mut app = App::new();
    if args.desktop {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "THE PIT - prototipo de escala".into(),
                resolution: (1600u32, 900u32).into(),
                ..default()
            }),
            ..default()
        }));
    } else {
        app.add_plugins(bevy_mod_openxr::add_xr_plugins(
            DefaultPlugins
                .build()
                .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
        ));
        app.add_systems(Update, add_fog_to_xr_cameras);
        app.add_plugins(xr_input::XrInputPlugin);
    }
    let start_mood = if args.view == "clinical" { 1.0 } else { 0.0 };
    app.insert_resource(Mood { current: start_mood, target: start_mood })
        .insert_resource(Size(1))
        .insert_resource(RingMotion { moving: args.view == "ring", dir: -1.0 })
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(FrameCount::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (keys, apply_mood, move_ring, resize_shaft));
    if args.desktop {
        app.add_systems(Update, desktop_camera);
    }
    if args.shot.is_some() {
        app.add_systems(Update, take_shot);
    }
    app.insert_resource(args);
    app.run()
}

fn lamp(materials: &mut Assets<StandardMaterial>, c: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: c, unlit: true, ..default() })
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    args: Res<Args>,
    size: Res<Size>,
) {
    let m = Mats {
        wall: materials.add(StandardMaterial { base_color: Color::srgb(0.3, 0.28, 0.24), perceptual_roughness: 0.8, metallic: 0.2, ..default() }),
        lamp: lamp(&mut materials, Color::srgb(1.0, 0.75, 0.45)),
        lamp_alt: lamp(&mut materials, Color::srgb(1.0, 0.45, 0.15)),
        fixed: lamp(&mut materials, Color::srgb(1.0, 0.9, 0.7)),
        figure: materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.6, ..default() }),
        cap_top: lamp(&mut materials, Color::srgb(0.4, 0.6, 1.0)),
        cap_bottom: lamp(&mut materials, Color::srgb(1.0, 0.35, 0.08)),
    };
    shaft::spawn_shaft(&mut commands, &mut meshes, &m, SIZES[size.0].0);
    commands.insert_resource(Materials { m });

    if args.desktop {
        let (pos, yaw, pitch) = match args.view.as_str() {
            "up" => (Vec3::new(0.5, 1.7, 2.6), 0.2, 1.35),
            "down" => (Vec3::new(1.6, 1.6, 6.4), 0.3, -1.2),
            "ring" => (Vec3::new(2.0, 1.7, 3.5), 0.1, 0.35),
            "clinical" => (Vec3::new(-4.0, 1.7, 3.0), 0.5, 0.25),
            _ => (Vec3::new(6.0, 17.0, apothem_cam()), 0.12, -0.38),
        };
        commands.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection { fov: 100f32.to_radians(), far: 3000.0, ..default() }),
            Transform::from_translation(pos),
            DesktopCam { yaw, pitch },
            fog(0.0),
        ));
    }
}

fn fog(mood: f32) -> DistanceFog {
    let mech = Vec3::new(0.10, 0.07, 0.04);
    let clin = Vec3::new(0.62, 0.70, 0.76);
    let c = mech.lerp(clin, mood);
    DistanceFog {
        color: Color::srgb(c.x, c.y, c.z),
        falloff: FogFalloff::Exponential { density: 0.006 + (0.0035 - 0.006) * mood },
        ..default()
    }
}

fn add_fog_to_xr_cameras(
    mut commands: Commands,
    cams: Query<Entity, (With<Camera3d>, Without<DistanceFog>)>,
    mood: Res<Mood>,
) {
    for e in &cams {
        commands.entity(e).insert(fog(mood.current));
    }
}

fn keys(
    kb: Res<ButtonInput<KeyCode>>,
    mut mood: ResMut<Mood>,
    mut size: ResMut<Size>,
    mut ring: ResMut<RingMotion>,
    mut exit: MessageWriter<AppExit>,
) {
    if kb.just_pressed(KeyCode::KeyL) {
        mood.target = if mood.target < 0.5 { 1.0 } else { 0.0 };
        info!("estado: {}", if mood.target > 0.5 { "clinico" } else { "mecanico" });
    }
    for (k, i) in [(KeyCode::Digit1, 0), (KeyCode::Digit2, 1), (KeyCode::Digit3, 2)] {
        if kb.just_pressed(k) && size.0 != i {
            size.0 = i;
            info!("diametro do poco: {}", SIZES[i].1);
        }
    }
    if kb.just_pressed(KeyCode::KeyR) {
        ring.moving = !ring.moving;
    }
    if kb.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn apply_mood(
    time: Res<Time>,
    mut mood: ResMut<Mood>,
    mats: Res<Materials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fogs: Query<&mut DistanceFog>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut first: Local<bool>,
) {
    let step = time.delta_secs() / 4.0; // 4 s transition
    let before = mood.current;
    mood.current += (mood.target - mood.current).clamp(-step, step);
    if before == mood.current && *first {
        return;
    }
    *first = true;
    let t = mood.current;
    let mix = |a: Vec3, b: Vec3| {
        let c = a.lerp(b, t);
        Color::srgb(c.x, c.y, c.z)
    };
    if let Some(mut m) = materials.get_mut(&mats.m.wall) {
        m.base_color = mix(Vec3::new(0.30, 0.28, 0.24), Vec3::new(0.95, 0.96, 0.97));
        m.perceptual_roughness = 0.8 + (0.35 - 0.8) * t;
        m.metallic = 0.2 * (1.0 - t);
    }
    if let Some(mut m) = materials.get_mut(&mats.m.lamp) {
        m.base_color = mix(Vec3::new(1.0, 0.75, 0.45), Vec3::new(0.85, 0.95, 1.0));
    }
    if let Some(mut m) = materials.get_mut(&mats.m.lamp_alt) {
        m.base_color = mix(Vec3::new(1.0, 0.45, 0.15), Vec3::new(0.55, 0.85, 1.0));
    }
    if let Some(mut m) = materials.get_mut(&mats.m.cap_bottom) {
        m.base_color = mix(Vec3::new(1.0, 0.35, 0.08), Vec3::new(0.9, 0.95, 1.0));
    }
    if let Some(mut m) = materials.get_mut(&mats.m.cap_top) {
        m.base_color = mix(Vec3::new(0.4, 0.6, 1.0), Vec3::new(1.0, 1.0, 1.0));
    }
    for mut f in &mut fogs {
        *f = fog(t);
    }
    ambient.color = mix(Vec3::new(1.0, 0.8, 0.6), Vec3::new(0.9, 0.95, 1.0));
    ambient.brightness = 40.0 + (900.0 - 40.0) * t;
}

fn move_ring(time: Res<Time>, mut ring: ResMut<RingMotion>, mut q: Query<&mut Transform, With<MovingRing>>) {
    if !ring.moving {
        return;
    }
    // 1.5 m/s: slow enough to read as a machine, not as the world moving
    for mut t in &mut q {
        t.translation.y += ring.dir * 1.5 * time.delta_secs();
        if t.translation.y < 12.0 {
            ring.dir = 1.0;
        }
        if t.translation.y > 150.0 {
            ring.dir = -1.0;
        }
    }
}

fn resize_shaft(
    mut commands: Commands,
    size: Res<Size>,
    parts: Query<Entity, With<ShaftPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Materials>,
) {
    if !size.is_changed() || size.is_added() {
        return;
    }
    for e in &parts {
        commands.entity(e).despawn();
    }
    shaft::spawn_shaft(&mut commands, &mut meshes, &mats.m, SIZES[size.0].0);
}

fn desktop_camera(
    time: Res<Time>,
    kb: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<bevy::input::mouse::AccumulatedMouseMotion>,
    mut cursor: Query<&mut CursorOptions>,
    mut q: Query<(&mut Transform, &mut DesktopCam)>,
) {
    let looking = mouse.pressed(MouseButton::Right);
    if let Ok(mut c) = cursor.single_mut() {
        c.grab_mode = if looking { CursorGrabMode::Locked } else { CursorGrabMode::None };
        c.visible = !looking;
    }
    for (mut t, mut cam) in &mut q {
        if looking {
            cam.yaw -= motion.delta.x * 0.003;
            cam.pitch = (cam.pitch - motion.delta.y * 0.003).clamp(-1.55, 1.55);
        }
        t.rotation = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0);
        let mut v = Vec3::ZERO;
        let fwd = t.rotation * -Vec3::Z;
        let fwd = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
        if kb.pressed(KeyCode::KeyW) { v += fwd; }
        if kb.pressed(KeyCode::KeyS) { v -= fwd; }
        if kb.pressed(KeyCode::KeyD) { v += right; }
        if kb.pressed(KeyCode::KeyA) { v -= right; }
        if kb.pressed(KeyCode::KeyE) { v += Vec3::Y; }
        if kb.pressed(KeyCode::KeyQ) { v -= Vec3::Y; }
        let speed = if kb.pressed(KeyCode::ShiftLeft) { 20.0 } else { 2.0 };
        t.translation += v * speed * time.delta_secs();
    }
}

fn take_shot(
    mut commands: Commands,
    args: Res<Args>,
    mut n: ResMut<FrameCount>,
    mut exit: MessageWriter<AppExit>,
) {
    n.0 += 1;
    if n.0 == 40 {
        let path = args.shot.clone().unwrap();
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if n.0 == 70 {
        exit.write(AppExit::Success);
    }
}

/// Desktop "wide" view stands on the ring beam opposite the bridge.
fn apothem_cam() -> f32 {
    SIZES[1].0 - 8.0
}
