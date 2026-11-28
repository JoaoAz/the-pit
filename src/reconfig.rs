//! Reconfiguration of the shaft between the two states.
//!
//! Mechanical: the wall's giant panels are slid aside and stacked at the corners of
//! each facet, where they read as massive pillars, and the industrial machinery is
//! exposed. Clinical: they slide out like sliding gates, each one along the wall and
//! then back into the skin plane, closing a smooth white skin with lines of light. The closing travels as a wave from both
//! far ends of the shaft towards the platform, so the last panels to shut are the
//! nearest ones. Opening runs the other way (nearest first).
//!
//! Sequence: idle -> alarm (lights pulse red) -> motion -> idle in the other state.
//! It runs on its own (autoplay) or on demand (A button / L key).

use crate::meshbuild::{MeshBuilder, Rng};
use crate::panorama::far;
use crate::shaft::{MODULE_H, Mats, S, SIDES, ShaftPart};
use std::collections::HashMap;
use crate::{Mood, RingMotion};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

const IDLE_S: f32 = 60.0;
const ALARM_S: f32 = 8.0;
const MOTION_S: f32 = 50.0;
/// Fraction of the motion timeline one shutter takes to swing.
const SWING: f32 = 0.22;
/// Thickness of one panel and spacing of the layers in a corner stack.
const PANEL_T: f32 = 0.6 * S;
const LAYER: f32 = 1.2 * S;
/// Distance of the skin from the wall face: clears the corner columns and ring beams.
const SKIN_W: f32 = 6.6 * S;
/// Modules each way that carry shutters (beyond this the fog hides the wall anyway).
const SHUTTER_MODULES: i32 = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Idle,
    Alarm,
    Motion,
}

#[derive(Resource)]
pub struct Reconfig {
    pub phase: Phase,
    pub timer: f32,
    /// 0 = mechanical (open), 1 = clinical (closed)
    pub progress: f32,
    /// where the current motion is heading
    pub heading: f32,
    pub autoplay: bool,
    /// 0..1 strength of the red alarm pulse
    pub alarm: f32,
    pub frozen: bool,
}

impl Reconfig {
    pub fn new(start: f32) -> Self {
        Reconfig { phase: Phase::Idle, timer: 0.0, progress: start, heading: start, autoplay: true, alarm: 0.0, frozen: false }
    }
    /// Start the transition to the other state (ignored while one is running).
    pub fn request(&mut self) {
        if self.phase == Phase::Idle {
            self.heading = if self.progress < 0.5 { 1.0 } else { 0.0 };
            self.phase = Phase::Alarm;
            self.timer = 0.0;
        }
    }
}

#[derive(Component)]
pub struct Shutter {
    delay: f32,
    /// position in the corner stack (mechanical)
    open: Vec3,
    /// position in the skin (clinical)
    closed: Vec3,
    /// outward offset of its stack layer
    out: Vec3,
}

pub struct ReconfigPlugin;

impl Plugin for ReconfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (run_sequence, swing_shutters).chain());
    }
}

fn run_sequence(time: Res<Time>, mut rc: ResMut<Reconfig>, mut mood: ResMut<Mood>, mut ring: ResMut<RingMotion>) {
    if rc.frozen {
        mood.current = rc.progress;
        return;
    }
    let dt = time.delta_secs();
    rc.timer += dt;
    // manual requests arrive through Mood::target (A button / L key)
    let want = if mood.target > 0.5 { 1.0 } else { 0.0 };
    if rc.phase == Phase::Idle && want != rc.progress.round() {
        rc.request();
    }
    match rc.phase {
        Phase::Idle => {
            rc.alarm = (rc.alarm - dt).max(0.0);
            if rc.autoplay && rc.timer > IDLE_S {
                rc.request();
                mood.target = rc.heading;
            }
        }
        Phase::Alarm => {
            rc.alarm = (rc.timer / 1.5).min(1.0);
            if rc.timer > ALARM_S {
                rc.phase = Phase::Motion;
                rc.timer = 0.0;
                ring.moving = true;
            }
        }
        Phase::Motion => {
            let step = dt / MOTION_S;
            rc.progress += (rc.heading - rc.progress).clamp(-step, step);
            // the alarm fades during the first third of the motion
            rc.alarm = (1.0 - rc.timer / (MOTION_S * 0.33)).max(0.0);
            if rc.progress == rc.heading {
                rc.phase = Phase::Idle;
                rc.timer = 0.0;
                mood.target = rc.heading;
            }
        }
    }
    // materials follow the wave: they change mostly while the near panels move
    mood.current = smooth(((rc.progress - 0.35) / 0.6).clamp(0.0, 1.0));
}

fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

fn swing_shutters(rc: Res<Reconfig>, mut q: Query<(&Shutter, &mut Transform)>, mut last: Local<Option<f32>>) {
    if *last == Some(rc.progress) {
        return;
    }
    *last = Some(rc.progress);
    for (s, mut t) in &mut q {
        let c = ((rc.progress - s.delay) / SWING).clamp(0.0, 1.0);
        // slide along the wall in its stack layer first, then sink back into the skin
        let slide = smooth((c / 0.75).min(1.0));
        let sink = smooth(((c - 0.75) / 0.25).max(0.0));
        t.translation = s.open.lerp(s.closed, slide) + s.out * (1.0 - sink);
    }
}

/// Builds a panel (and its light line) centred on its own origin: a thick slab facing
/// +Z (the shaft centre) with a raised frame, ribs and a recessed seam, so it still
/// reads as a solid object when it stands in a corner stack.
fn shutter_meshes(w: f32, h: f32) -> (Mesh, Mesh) {
    let mut p = MeshBuilder::default();
    let mut l = MeshBuilder::glowing();
    let ax = [Vec3::X, Vec3::Y, Vec3::Z];
    let gap = 0.15 * S;
    let (hw, hh) = (w / 2.0 - gap, h / 2.0 - gap);
    let g = [0.95, 0.95, 0.95, 1.0];
    let d = [0.6, 0.6, 0.6, 1.0];
    p.cuboid(Vec3::ZERO, ax, [hw, hh, PANEL_T / 2.0], g);
    // deep frame: proud enough (about 1.5 m) to cast a visible edge from hundreds of metres
    let f = 0.6 * S;
    let fz = PANEL_T / 2.0 + 0.75;
    for y in [-hh + f / 2.0, hh - f / 2.0] {
        p.cuboid(Vec3::new(0.0, y, fz), ax, [hw, f / 2.0, 0.75], d);
    }
    for x in [-hw + f / 2.0, hw - f / 2.0] {
        p.cuboid(Vec3::new(x, 0.0, fz), ax, [f / 2.0, hh, 0.75], d);
    }
    // the field inside the frame is split into bays: recessed plates, ventilation
    // grilles (deep slats), service hatches and rivet rows, chosen per panel size
    let mut rng = Rng::new((w * 131.0 + h * 7.0) as u64);
    let (ix0, ix1) = (-hw + f, hw - f);
    let (iy0, iy1) = (-hh + f, hh - f);
    let mut y = iy0;
    let face = PANEL_T / 2.0;
    while y < iy1 - 1.0 {
        let bh = rng.range(6.0, 16.0).min(iy1 - y);
        let (b0, b1) = (y + 0.3, y + bh - 0.3);
        let cy = (b0 + b1) / 2.0;
        let bw = (ix1 - ix0) / 2.0 - 0.3;
        match rng.int(0, 3) {
            0 => {
                // grille: thick horizontal slats with dark gaps between them
                let n = ((b1 - b0) / 0.9) as i32;
                for k in 0..n {
                    let sy = b0 + 0.45 + k as f32 * 0.9;
                    p.cuboid(Vec3::new(0.0, sy, face + 0.35), ax, [bw - 0.4, 0.22, 0.35], d);
                }
                p.cuboid(Vec3::new(0.0, cy, face + 0.02), ax, [bw, (b1 - b0) / 2.0, 0.02], [0.25, 0.25, 0.25, 1.0]);
            }
            1 => {
                // raised plate with chamfer-like double step and a rivet row
                p.cuboid(Vec3::new(0.0, cy, face + 0.3), ax, [bw, (b1 - b0) / 2.0, 0.3], g);
                p.cuboid(Vec3::new(0.0, cy, face + 0.75), ax, [bw - 0.8, (b1 - b0) / 2.0 - 0.8, 0.15], g);
                let nr = (bw / 1.2) as i32;
                for k in 0..nr {
                    let rx = -bw + 0.6 + k as f32 * 2.0 * bw / nr as f32;
                    for ry in [b0 + 0.35, b1 - 0.35] {
                        p.cuboid(Vec3::new(rx, ry, face + 0.66), ax, [0.12, 0.12, 0.08], d);
                    }
                }
            }
            2 => {
                // two hatches side by side with handles and hinges
                for side in [-1.0f32, 1.0] {
                    let hx = side * bw / 2.0;
                    let hwid = bw / 2.0 - 0.6;
                    p.cuboid(Vec3::new(hx, cy, face + 0.4), ax, [hwid, (b1 - b0) / 2.0 - 0.3, 0.4], g);
                    p.cuboid(Vec3::new(hx + side * (hwid - 0.8), cy, face + 0.95), ax, [0.15, 0.9, 0.15], d);
                    for hy in [b0 + 1.2, b1 - 1.2] {
                        p.cuboid(Vec3::new(hx - side * hwid, hy, face + 0.6), ax, [0.25, 0.5, 0.25], d);
                    }
                }
            }
            _ => {
                // recessed bay: dark back plate behind a thin lip
                p.cuboid(Vec3::new(0.0, cy, face + 0.02), ax, [bw, (b1 - b0) / 2.0, 0.02], [0.3, 0.3, 0.3, 1.0]);
                p.cuboid(Vec3::new(0.0, b1 - 0.2, face + 0.5), ax, [bw, 0.2, 0.5], d);
                p.cuboid(Vec3::new(0.0, b0 + 0.2, face + 0.5), ax, [bw, 0.2, 0.5], d);
                // a pipe crossing the bay
                p.cylinder(Vec3::new(-bw, cy, face + 0.8), Vec3::new(bw, cy, face + 0.8), 0.35, 8, d);
            }
        }
        y += bh;
    }
    l.cuboid(Vec3::new(hw - f / 2.0, 0.0, PANEL_T / 2.0 + 1.55), ax, [0.12 * S, hh - 2.0 * S, 0.03], [1.0, 1.0, 1.0, 1.0]);
    (p.build(), l.build())
}

/// Spawns the panels for a shaft of the given apothem. Column widths, the number of
/// rows per module and which panels carry a light line all vary. In the open
/// (mechanical) state each facet's panels stand stacked in layers at its two corners.
pub fn spawn_shutters(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32, z_off: f32) {
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let ymax = SHUTTER_MODULES as f32 * MODULE_H;
    let mut rng = Rng::new(42);
    let mut cache: HashMap<(u32, u32), (Handle<Mesh>, Handle<Mesh>)> = HashMap::new();
    for k in -SHUTTER_MODULES..SHUTTER_MODULES {
        for i in 0..SIDES {
            let th = TAU * i as f32 / SIDES as f32;
            let d = Vec3::new(th.cos(), 0.0, th.sin());
            let t = Vec3::new(-th.sin(), 0.0, th.cos());
            let n = -d;
            let basis = Quat::from_mat3(&Mat3::from_cols(t, Vec3::Y, n));
            let rows = [1u32, 2, 2, 3][rng.int(0, 3) as usize];
            let ph = MODULE_H / rows as f32;
            // column edges on whole metres so panels of equal width share a mesh
            let mut cols = Vec::new();
            let mut x = 0.0f32;
            while x < w_face - 1.0 {
                let mut w = rng.range(25.0, 70.0).round();
                if w_face - (x + w) < 25.0 {
                    w = (w_face - x).round().max(1.0);
                }
                cols.push((x, w));
                x += w;
            }
            let ncols = cols.len();
            for r in 0..rows {
                let centre_y = z_off + k as f32 * MODULE_H + (r as f32 + 0.5) * ph;
                // far panels shut first, the ones level with the platform last
                let dist = (centre_y.abs() / ymax).min(1.0);
                for (ci, &(x0, pw)) in cols.iter().enumerate() {
                    let (pm, lm) = cache
                        .entry((pw as u32, rows))
                        .or_insert_with(|| {
                            let (a, b) = shutter_meshes(pw, ph);
                            (meshes.add(a), meshes.add(b))
                        })
                        .clone();
                    let u = -w_face / 2.0 + x0 + pw / 2.0;
                    // left half stacks at the left corner, right half at the right one;
                    // the panel nearest the corner stands at the back of the stack
                    let left = ci < ncols / 2;
                    let layer = if left { ci } else { ncols - 1 - ci } as f32;
                    let u_open = if left { -w_face / 2.0 + pw / 2.0 + 2.0 * S } else { w_face / 2.0 - pw / 2.0 - 2.0 * S };
                    let base = d * apothem + Vec3::Y * centre_y + n * (SKIN_W + PANEL_T / 2.0);
                    let closed = base + t * u;
                    let open = base + t * u_open;
                    let out = n * (layer + 1.0) * LAYER;
                    let delay = ((1.0 - dist) * (1.0 - SWING - 0.1) + rng.range(0.0, 0.1)).clamp(0.0, 1.0 - SWING);
                    let skin = mats.skins[rng.int(0, mats.skins.len() as u32 - 1) as usize].clone();
                    let mut e = commands.spawn((
                        ShaftPart,
                        far(),
                        Shutter { delay, open, closed, out },
                        Mesh3d(pm),
                        MeshMaterial3d(skin),
                        Transform::from_translation(open + out).with_rotation(basis),
                    ));
                    if rng.chance(0.05) {
                        e.with_children(|ch| {
                            ch.spawn((far(), Mesh3d(lm), MeshMaterial3d(mats.skin_line.clone())));
                        });
                    }
                }
            }
        }
    }
}
