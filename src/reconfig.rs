//! Reconfiguration of the shaft between the two states.
//!
//! Mechanical: the wall's giant shutters stand open like horizontal fins, the
//! industrial machinery behind them exposed. Clinical: the shutters swing shut into a
//! smooth white skin with lines of light. The closing travels as a wave from both
//! far ends of the shaft towards the platform, so the last panels to shut are the
//! nearest ones. Opening runs the other way (nearest first).
//!
//! Sequence: idle -> alarm (lights pulse red) -> motion -> idle in the other state.
//! It runs on its own (autoplay) or on demand (A button / L key).

use crate::meshbuild::{MeshBuilder, Rng};
use crate::shaft::{MODULE_H, Mats, S, SIDES, ShaftPart};
use crate::{Mood, RingMotion};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

const IDLE_S: f32 = 60.0;
const ALARM_S: f32 = 8.0;
const MOTION_S: f32 = 50.0;
/// Fraction of the motion timeline one shutter takes to swing.
const SWING: f32 = 0.22;
/// Open angle of the shutters in the mechanical state.
const OPEN_ANGLE: f32 = 82.0 * PI / 180.0;
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
        t.rotation = s.basis * Quat::from_rotation_x(-(1.0 - closed) * OPEN_ANGLE);
    }
}

/// Builds the shutter meshes (panel and its light line) in pivot space: the pivot is
/// the panel's top edge, the panel hangs down along -Y and faces +Z (the shaft centre).
fn shutter_meshes(w: f32, h: f32) -> (Mesh, Mesh) {
    let mut p = MeshBuilder::default();
    let mut l = MeshBuilder::default();
    let ax = [Vec3::X, Vec3::Y, Vec3::Z];
    let th = 0.4 * S;
    let gap = 0.15 * S;
    p.cuboid(Vec3::new(0.0, -h / 2.0, th / 2.0), ax, [w / 2.0 - gap, h / 2.0 - gap, th / 2.0], [0.95, 0.95, 0.95, 1.0]);
    // a recessed seam in the middle and a strip of light along the lower edge
    p.cuboid(Vec3::new(0.0, -h * 0.5, th + 0.01), ax, [w / 2.0 - gap, 0.04 * S, 0.02], [0.4, 0.4, 0.4, 1.0]);
    l.cuboid(Vec3::new(0.0, -h + 0.5 * S, th + 0.03), ax, [w / 2.0 - 1.5 * S, 0.12 * S, 0.03], [1.0, 1.0, 1.0, 1.0]);
    (p.build(), l.build())
}

/// Spawns the shutters for a shaft of the given apothem.
pub fn spawn_shutters(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32, z_off: f32) {
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let cols = (w_face / (15.0 * S)).round().max(2.0) as i32;
    let pw = w_face / cols as f32;
    let rows = 2;
    let ph = MODULE_H / rows as f32;
    let (pm, lm) = shutter_meshes(pw, ph);
    let (pm, lm) = (meshes.add(pm), meshes.add(lm));
    let ymax = SHUTTER_MODULES as f32 * MODULE_H;
    let mut rng = Rng::new(42);
    for k in -SHUTTER_MODULES..SHUTTER_MODULES {
        for i in 0..SIDES {
            let th = TAU * i as f32 / SIDES as f32;
            let d = Vec3::new(th.cos(), 0.0, th.sin());
            let t = Vec3::new(-th.sin(), 0.0, th.cos());
            let n = -d;
            let basis = Quat::from_mat3(&Mat3::from_cols(t, Vec3::Y, n));
            for r in 0..rows {
                let top = z_off + k as f32 * MODULE_H + (r + 1) as f32 * ph;
                // far panels shut first, the ones level with the platform last
                let dist = ((top - ph / 2.0).abs() / ymax).min(1.0);
                for c in 0..cols {
                    let u = -w_face / 2.0 + (c as f32 + 0.5) * pw;
                    let delay = ((1.0 - dist) * (1.0 - SWING - 0.06) + rng.range(0.0, 0.06)).clamp(0.0, 1.0 - SWING);
                    let pivot = d * apothem + t * u + Vec3::Y * top + n * SKIN_W;
                    commands
                        .spawn((
                            ShaftPart,
                            Shutter { basis, delay },
                            Mesh3d(pm.clone()),
                            MeshMaterial3d(mats.skin.clone()),
                            Transform::from_translation(pivot).with_rotation(basis * Quat::from_rotation_x(-OPEN_ANGLE)),
                        ))
                        .with_children(|ch| {
                            ch.spawn((Mesh3d(lm.clone()), MeshMaterial3d(mats.skin_line.clone())));
                        });
                }
            }
        }
    }
}
