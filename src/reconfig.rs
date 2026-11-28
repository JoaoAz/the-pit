//! Reconfiguration of the shaft between the two states.
//!
//! Mechanical: the wall's giant panels stand open like doors, turned edge-on towards
//! the shaft centre, so from the platform they are thin slivers and the industrial
//! machinery behind them is exposed. Clinical: they swing shut into a smooth white
//! skin with lines of light. The closing travels as a wave from both
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
/// Open angle range of the shutters in the mechanical state (each one differs a little).
const OPEN_MIN: f32 = 78.0 * PI / 180.0;
const OPEN_MAX: f32 = 100.0 * PI / 180.0;
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
    basis: Quat,
    delay: f32,
    /// signed open angle around the hinge (sign depends on the hinge side)
    open: f32,
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
        let closed = smooth(((rc.progress - s.delay) / SWING).clamp(0.0, 1.0));
        t.rotation = s.basis * Quat::from_rotation_y((1.0 - closed) * s.open);
    }
}

/// Builds the shutter meshes (panel and its light line) in hinge space: the hinge is
/// the panel's top corner on the left (`right = false`, panel along +X) or on the
/// right (panel along -X); the panel hangs down along -Y and faces +Z (the centre).
fn shutter_meshes(w: f32, h: f32, right: bool) -> (Mesh, Mesh) {
    let mut p = MeshBuilder::default();
    let mut l = MeshBuilder::default();
    let ax = [Vec3::X, Vec3::Y, Vec3::Z];
    let th = 0.4 * S;
    let gap = 0.15 * S;
    let sx = if right { -1.0 } else { 1.0 };
    p.cuboid(Vec3::new(sx * w / 2.0, -h / 2.0, th / 2.0), ax, [w / 2.0 - gap, h / 2.0 - gap, th / 2.0], [0.95, 0.95, 0.95, 1.0]);
    // a recessed seam in the middle and a vertical strip of light near the free edge
    p.cuboid(Vec3::new(sx * w / 2.0, -h * 0.5, th + 0.01), ax, [w / 2.0 - gap, 0.04 * S, 0.02], [0.4, 0.4, 0.4, 1.0]);
    l.cuboid(Vec3::new(sx * (w - 1.2 * S), -h / 2.0, th + 0.03), ax, [0.12 * S, h / 2.0 - 1.5 * S, 0.03], [1.0, 1.0, 1.0, 1.0]);
    (p.build(), l.build())
}

/// Spawns the shutters for a shaft of the given apothem. Column widths, the number of
/// rows per module, the open angle and which panels carry a light line all vary.
pub fn spawn_shutters(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32, z_off: f32) {
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let ymax = SHUTTER_MODULES as f32 * MODULE_H;
    let mut rng = Rng::new(42);
    let mut cache: HashMap<(u32, u32, bool), (Handle<Mesh>, Handle<Mesh>)> = HashMap::new();
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
            for r in 0..rows {
                let top = z_off + k as f32 * MODULE_H + (r + 1) as f32 * ph;
                // far panels shut first, the ones level with the platform last
                let dist = ((top - ph / 2.0).abs() / ymax).min(1.0);
                for &(x0, pw) in &cols {
                    let right = rng.chance(0.5);
                    let (pm, lm) = cache
                        .entry((pw as u32, rows, right))
                        .or_insert_with(|| {
                            let (a, b) = shutter_meshes(pw, ph, right);
                            (meshes.add(a), meshes.add(b))
                        })
                        .clone();
                    // hinge on the left or right edge of the column
                    let u = -w_face / 2.0 + x0 + if right { pw } else { 0.0 };
                    let delay = ((1.0 - dist) * (1.0 - SWING - 0.1) + rng.range(0.0, 0.1)).clamp(0.0, 1.0 - SWING);
                    // rotating about Y by -90 deg turns +X into +Z (towards the centre)
                    let open = rng.range(OPEN_MIN, OPEN_MAX) * if right { 1.0 } else { -1.0 };
                    let pivot = d * apothem + t * u + Vec3::Y * top + n * SKIN_W;
                    let skin = mats.skins[rng.int(0, mats.skins.len() as u32 - 1) as usize].clone();
                    let mut e = commands.spawn((
                        ShaftPart,
                        far(),
                        Shutter { basis, delay, open },
                        Mesh3d(pm),
                        MeshMaterial3d(skin),
                        Transform::from_translation(pivot).with_rotation(basis * Quat::from_rotation_y(open)),
                    ));
                    if rng.chance(0.45) {
                        e.with_children(|ch| {
                            ch.spawn((far(), Mesh3d(lm), MeshMaterial3d(mats.skin_line.clone())));
                        });
                    }
                }
            }
        }
    }
}
