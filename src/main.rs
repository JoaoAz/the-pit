//! THE PIT.
//!
//! Run without arguments to start in VR (OpenXR, e.g. Quest 3 over Link).
//! `--desktop` opens a normal window with a mouse-look camera.
//! `--shot <file.png> --view <wide|up|down|far|ring|clinical|transition>` renders one desktop frame and exits.
//! `--pano <pixels>` sets the size of each panorama face (default 2048).

mod meshbuild;
mod panorama;
mod reconfig;
mod shaft;
mod texture;
mod xr_input;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::math::Affine2;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::window::{CursorGrabMode, CursorOptions};
use shaft::{Mats, MovingRing, PitLight, ShaftPart, SIZES};

#[derive(Resource)]
struct Args {
    desktop: bool,
    shot: Option<String>,
    view: String,
    pano: u32,
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
        pano: get("--pano").and_then(|v| v.parse().ok()).unwrap_or(2048),
        shot,
    }
}

fn main() -> AppExit {
    let args = parse_args();
    let mut app = App::new();
    if args.desktop {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "THE PIT".into(),
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
        app.add_plugins(xr_input::XrInputPlugin);
    }
    app.add_plugins((FrameTimeDiagnosticsPlugin::default(), panorama::PanoramaPlugin { res: args.pano }))
        .add_systems(Update, (add_fog_to_cameras, animate_smoke, report_frame_time));
    let start_mood = if args.view == "clinical" { 1.0 } else { 0.0 };
    let mut rc = reconfig::Reconfig::new(start_mood);
    if args.shot.is_some() {
        // screenshots show a fixed moment of the sequence
        rc.frozen = true;
        if args.view == "transition" {
            rc.progress = 0.62;
        }
    }
    app.insert_resource(rc)
        .add_plugins(reconfig::ReconfigPlugin)
        .insert_resource(Mood { current: start_mood, target: start_mood })
        .insert_resource(Size(shaft::DEFAULT_SIZE))
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

/// Additive sprite material (halos, light beams): adds light, ignores depth order.
fn additive(materials: &mut Assets<StandardMaterial>, tex: Option<Handle<Image>>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color_texture: tex,
        unlit: true,
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        double_sided: true,
        ..default()
    })
}

/// Skin tints (mechanical colour, clinical colour, roughness) for the shutter variants.
const SKINS: [([f32; 3], [f32; 3], f32); 4] = [
    ([0.08, 0.075, 0.065], [0.86, 0.88, 0.9], 0.65),
    ([0.065, 0.06, 0.055], [0.8, 0.82, 0.84], 0.75),
    ([0.1, 0.09, 0.075], [0.9, 0.9, 0.88], 0.6),
    ([0.055, 0.055, 0.05], [0.74, 0.77, 0.8], 0.8),
];

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    args: Res<Args>,
    size: Res<Size>,
) {
    let wt = texture::wall_textures();
    let st = texture::skin_textures();
    let skin_albedo = images.add(st.albedo);
    let skin_mr = images.add(st.metal_rough);
    let skin_normal = images.add(st.normal);
    let glow = images.add(texture::glow_texture());
    let m = Mats {
        wall: materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.2, 0.17),
            base_color_texture: Some(images.add(wt.albedo)),
            emissive_texture: wt.emissive.map(|e| images.add(e)),
            emissive: LinearRgba::rgb(2.0, 1.4, 1.0),
            metallic_roughness_texture: Some(images.add(wt.metal_rough)),
            normal_map_texture: Some(images.add(wt.normal)),
            perceptual_roughness: 1.0,
            metallic: 1.0,
            reflectance: 0.3,
            ..default()
        }),
        lamp: lamp(&mut materials, Color::srgb(1.0, 0.75, 0.45)),
        lamp_alt: lamp(&mut materials, Color::srgb(1.0, 0.45, 0.15)),
        figure: materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.8, ..default() }),
        platform: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.3),
            base_color_texture: Some(skin_albedo.clone()),
            normal_map_texture: Some(skin_normal.clone()),
            perceptual_roughness: 0.85,
            reflectance: 0.3,
            ..default()
        }),
        skins: SKINS
            .iter()
            .map(|(c, _, r)| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(c[0], c[1], c[2]),
                    base_color_texture: Some(skin_albedo.clone()),
                    metallic_roughness_texture: Some(skin_mr.clone()),
                    normal_map_texture: Some(skin_normal.clone()),
                    perceptual_roughness: *r,
                    metallic: 0.0,
                    reflectance: 0.35,
                    ..default()
                })
            })
            .collect(),
        skin_line: lamp(&mut materials, Color::srgb(0.5, 0.3, 0.12)),
        halo: additive(&mut materials, Some(glow)),
        beam: additive(&mut materials, None),
        smoke: materials.add(StandardMaterial {
            base_color: Color::srgba(0.3, 0.27, 0.24, 0.6),
            base_color_texture: Some(images.add(texture::smoke_texture())),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),
    };
    // a soft light for the platform and the hands (the shaft lights only reach the panorama)
    commands.spawn((
        PointLight { color: Color::srgb(1.0, 0.85, 0.7), intensity: 600_000.0, range: 40.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(4.0, 9.0, -3.0),
    ));
    shaft::spawn_shaft(&mut commands, &mut meshes, &m, SIZES[size.0].0);
    commands.insert_resource(Materials { m });

    if args.desktop {
        let (pos, yaw, pitch) = match args.view.as_str() {
            "up" => (Vec3::new(0.5, 1.7, 2.6), 0.2, 1.35),
            "down" => (Vec3::new(1.2, 1.6, 1.2), 0.8, -1.0),
            "ring" => (Vec3::new(2.0, 1.7, 3.5), 0.1, 0.45),
            "clinical" => (Vec3::new(-2.0, 1.7, 2.0), 0.5, 0.15),
            "far" => (Vec3::new(0.0, 6.0, 14.0), 0.0, -0.35),
            _ => (Vec3::new(0.0, 1.7, 4.0), 0.0, 0.05),
        };
        commands.spawn((
            Camera3d::default(),
            Projection::Perspective(PerspectiveProjection { fov: 100f32.to_radians(), far: 50000.0, ..default() }),
            Transform::from_translation(pos),
            DesktopCam { yaw, pitch },
            fog(0.0, SIZES[size.0].0),
        ));
    }
}

fn fog_color(mood: f32) -> Color {
    let mech = Vec3::new(0.085, 0.064, 0.045);
    let clin = Vec3::new(0.30, 0.34, 0.37);
    let c = mech.lerp(clin, mood);
    Color::srgb(c.x, c.y, c.z)
}

fn fog(mood: f32, _apothem: f32) -> DistanceFog {
    DistanceFog {
        color: fog_color(mood),
        falloff: FogFalloff::Exponential { density: shaft::fog_density(mood) },
        ..default()
    }
}

fn add_fog_to_cameras(
    mut commands: Commands,
    cams: Query<Entity, (With<Camera3d>, Without<DistanceFog>)>,
    mood: Res<Mood>,
    size: Res<Size>,
) {
    for e in &cams {
        commands.entity(e).insert(fog(mood.current, SIZES[size.0].0));
    }
}

fn keys(
    kb: Res<ButtonInput<KeyCode>>,
    mut mood: ResMut<Mood>,
    mut size: ResMut<Size>,
    mut ring: ResMut<RingMotion>,
    mut rc: ResMut<reconfig::Reconfig>,
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
    if kb.just_pressed(KeyCode::KeyP) {
        rc.autoplay = !rc.autoplay;
        info!("autoplay: {}", rc.autoplay);
    }
    if kb.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
}

fn apply_mood(
    time: Res<Time>,
    mood: Res<Mood>,
    mats: Res<Materials>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fogs: Query<&mut DistanceFog>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    size: Res<Size>,
    rc: Res<reconfig::Reconfig>,
    mut lights: Query<(&mut SpotLight, &PitLight)>,
    mut first: Local<bool>,
) {
    // mood.current is driven by the reconfiguration sequence
    let pulse = rc.alarm * (0.5 + 0.5 * (time.elapsed_secs() * 5.0).sin());
    if !mood.is_changed() && rc.alarm == 0.0 && *first && !size.is_changed() {
        return;
    }
    *first = true;
    let t = mood.current;
    let mix = |a: Vec3, b: Vec3| {
        let c = a.lerp(b, t);
        Color::srgb(c.x, c.y, c.z)
    };
    if let Some(mut m) = materials.get_mut(&mats.m.wall) {
        m.base_color = mix(Vec3::new(0.3, 0.28, 0.24), Vec3::new(0.8, 0.81, 0.82));
        // roughness and metal come from the texture; the clinical wall is a bit smoother
        m.perceptual_roughness = 1.0 - 0.2 * t;
        m.metallic = 1.0 - 0.7 * t;
        // window lights: warm in the mechanical state, cool and softer when clinical
        let e = Vec3::new(2.6, 1.8, 1.3).lerp(Vec3::new(0.8, 1.0, 1.3), t);
        m.emissive = LinearRgba::rgb(e.x, e.y, e.z);
    }
    let red = Vec3::new(1.0, 0.04, 0.02);
    let alarm = |c: Color| {
        let l = c.to_srgba();
        let v = Vec3::new(l.red, l.green, l.blue).lerp(red, pulse);
        Color::srgb(v.x, v.y, v.z)
    };
    if let Some(mut m) = materials.get_mut(&mats.m.lamp) {
        m.base_color = alarm(mix(Vec3::new(1.0, 0.75, 0.45), Vec3::new(0.85, 0.95, 1.0)));
    }
    if let Some(mut m) = materials.get_mut(&mats.m.lamp_alt) {
        m.base_color = alarm(mix(Vec3::new(1.0, 0.45, 0.15), Vec3::new(0.55, 0.85, 1.0)));
    }
    for (h, (mc, cc, r)) in mats.m.skins.iter().zip(SKINS) {
        if let Some(mut m) = materials.get_mut(h) {
            m.base_color = mix(Vec3::from(mc), Vec3::from(cc));
            m.perceptual_roughness = r + 0.1 * (1.0 - t);
        }
    }
    let lamp_col = alarm(mix(Vec3::new(1.0, 0.62, 0.3), Vec3::new(0.85, 0.93, 1.0))).to_linear();
    let glow = |k: f32| LinearRgba::rgb(lamp_col.red * k, lamp_col.green * k, lamp_col.blue * k);
    if let Some(mut m) = materials.get_mut(&mats.m.halo) {
        m.base_color = glow(3.0).into();
    }
    if let Some(mut m) = materials.get_mut(&mats.m.beam) {
        m.base_color = glow(0.6).into();
    }
    if let Some(mut m) = materials.get_mut(&mats.m.smoke) {
        let c = mix(Vec3::new(0.3, 0.26, 0.22), Vec3::new(0.75, 0.8, 0.85)).to_srgba();
        m.base_color = Color::srgba(c.red, c.green, c.blue, 0.55 - 0.15 * t);
    }
    for (mut l, p) in &mut lights {
        let c = alarm(mix(p.mech, p.clin)).to_linear();
        l.color = Color::LinearRgba(c);
    }
    if let Some(mut m) = materials.get_mut(&mats.m.skin_line) {
        m.base_color = alarm(mix(Vec3::new(0.5, 0.3, 0.12), Vec3::new(0.8, 0.95, 1.0)));
    }
    for mut f in &mut fogs {
        *f = fog(t, SIZES[size.0].0);
    }
    // the empty space beyond the last module is the fog colour: no visible end
    clear.0 = fog_color(t);
    ambient.color = mix(Vec3::new(1.0, 0.8, 0.6), Vec3::new(0.9, 0.95, 1.0));
    ambient.brightness = 130.0 + (300.0 - 130.0) * t;
}

/// Smoke drifts upwards (the texture scrolls; the plume shape is in the vertex alpha).
fn animate_smoke(time: Res<Time>, mats: Res<Materials>, mut materials: ResMut<Assets<StandardMaterial>>) {
    if let Some(mut m) = materials.get_mut(&mats.m.smoke) {
        let t = time.elapsed_secs();
        m.uv_transform = Affine2::from_translation(Vec2::new(t * 0.004, t * 0.03));
    }
}

/// Prints the frame time every 5 s to the console (visible next to the .exe on Windows).
fn report_frame_time(time: Res<Time>, diags: Res<DiagnosticsStore>, mut last: Local<f32>) {
    let now = time.elapsed_secs();
    if now - *last < 5.0 {
        return;
    }
    *last = now;
    let get = |p| diags.get(p).and_then(|d| d.smoothed());
    if let (Some(fps), Some(ms)) = (get(&FrameTimeDiagnosticsPlugin::FPS), get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)) {
        info!("desempenho: {fps:.0} fps, {ms:.1} ms por frame");
    }
}

fn move_ring(time: Res<Time>, mut ring: ResMut<RingMotion>, size: Res<Size>, mut q: Query<(&mut Transform, &MovingRing)>) {
    if !ring.moving {
        return;
    }
    // slow for its size, so it reads as a machine and not as the world moving
    let _ = &size;
    let speed = shaft::ring_speed();
    for (mut t, r) in &mut q {
        t.translation.y += ring.dir * speed * time.delta_secs();
        if t.translation.y < r.low {
            ring.dir = 1.0;
        }
        if t.translation.y > r.high {
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
    if n.0 == 46 {
        exit.write(AppExit::Success);
    }
}
