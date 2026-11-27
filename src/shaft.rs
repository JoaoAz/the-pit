//! Procedural blockout of the shaft, the floating platform and scale references.
//! Units are metres, Y is up, the platform surface is at y = 0.
//!
//! Scale strategy: the shaft diameter is huge (kilometres) but the *detail* keeps a
//! fixed real-world size (ring beam every 120 m, panels of 10-30 m, windows of ~1 m,
//! human catwalks). A bigger shaft therefore shows many more elements, and the fog
//! has a real density in metres, so the far wall fades like a distant mountain.

use crate::meshbuild::{MeshBuilder, Rng};
use bevy::camera::visibility::VisibilityRange;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

pub const SIDES: usize = 12;
pub const PLATFORM_R: f32 = 2.0;
const RAIL_H: f32 = 1.1;

/// Shaft sizes the participant can compare (apothem, i.e. centre to wall face).
pub const SIZES: [(f32, &str); 3] = [(500.0, "1 km"), (1000.0, "2 km"), (2000.0, "4 km")];
pub const DEFAULT_SIZE: usize = 1;

/// Fixed structural scale (relative to the 30 m module of the concept art).
pub const S: f32 = 4.0;
/// Fixed module height: one ring beam every 120 m, whatever the diameter.
pub const MODULE_H: f32 = 30.0 * S;
/// Module origin offset so the platform floats between two ring beams.
const Z_OFF: f32 = -0.53 * MODULE_H;
/// Enough modules that both ends dissolve completely in the fog (~13 km each way).
const MODULES_EACH_WAY: i32 = 110;

/// LOD distances (metres from the camera to the module origin, i.e. vertical distance).
const LOD_FINE: f32 = 1300.0;
const LOD_MID: f32 = 3500.0;

/// Fog density in 1/m: real atmospheric perspective, independent of shaft size.
pub fn fog_density(mood: f32) -> f32 {
    0.00045 + (0.0003 - 0.00045) * mood
}

#[derive(Component)]
pub struct ShaftPart;

#[derive(Component)]
pub struct MovingRing {
    pub low: f32,
    pub high: f32,
}

pub struct Mats {
    pub wall: Handle<StandardMaterial>,
    pub lamp: Handle<StandardMaterial>,
    pub lamp_alt: Handle<StandardMaterial>,
    pub figure: Handle<StandardMaterial>,
    pub platform: Handle<StandardMaterial>,
    pub skin: Handle<StandardMaterial>,
    pub skin_line: Handle<StandardMaterial>,
}

const GREY: [f32; 4] = [0.75, 0.75, 0.73, 1.0];
const DARK: [f32; 4] = [0.35, 0.35, 0.34, 1.0];
const HAZ_Y: [f32; 4] = [1.0, 0.72, 0.05, 1.0];
const HAZ_K: [f32; 4] = [0.08, 0.08, 0.08, 1.0];
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// Local frame of one wall facet.
#[derive(Clone, Copy)]
struct Facet {
    o: Vec3,
    t: Vec3,
    n: Vec3,
}

impl Facet {
    fn new(i: usize, apothem: f32) -> Self {
        let th = TAU * i as f32 / SIDES as f32;
        let d = Vec3::new(th.cos(), 0.0, th.sin());
        Facet { o: d * apothem, t: Vec3::new(-th.sin(), 0.0, th.cos()), n: -d }
    }
    /// u along the wall, v up, w away from the wall into the shaft.
    fn p(&self, u: f32, v: f32, w: f32) -> Vec3 {
        self.o + self.t * u + Vec3::Y * v + self.n * w
    }
    fn ax(&self) -> [Vec3; 3] {
        [self.t, Vec3::Y, self.n]
    }
}

fn face_width(apothem: f32) -> f32 {
    2.0 * apothem * (PI / SIDES as f32).tan()
}

fn tone(rng: &mut Rng) -> [f32; 4] {
    let g = rng.range(0.55, 1.0);
    let warm = rng.range(-0.04, 0.04);
    [g + warm, g, g - warm, 1.0]
}

#[allow(clippy::too_many_arguments)]
fn hazard_band(mb: &mut MeshBuilder, f: &Facet, u0: f32, u1: f32, v: f32, w: f32, h: f32, stripe: f32) {
    let n = ((u1 - u0) / stripe).max(1.0) as i32;
    let step = (u1 - u0) / n as f32;
    for k in 0..n {
        let u = u0 + (k as f32 + 0.5) * step;
        let c = if k % 2 == 0 { HAZ_Y } else { HAZ_K };
        mb.cuboid(f.p(u, v, w), f.ax(), [step / 2.0, h, 0.05 * h.max(1.0)], c);
    }
}

/// All LOD meshes of one module variant.
struct ModuleMeshes {
    /// far: back wall + ring beam, one emissive strip per facet
    shell: Mesh,
    shell_lamp: Mesh,
    /// mid: panel grid, pipes, beam lamp fixtures, lit windows and vents
    mid: Mesh,
    mid_lamp: Mesh,
    mid_lamp_alt: Mesh,
    /// near only: greebles, machinery, human catwalks, doors, ladders
    fine: Mesh,
    fine_lamp: Mesh,
}

fn build_module(apothem: f32, var: u64) -> ModuleMeshes {
    let s = S;
    let h = MODULE_H;
    let w_face = face_width(apothem);
    let per = w_face / 24.0 / s; // how many "concept-art facets" fit in this one
    let mut shell = MeshBuilder::default();
    let mut shell_lamp = MeshBuilder::default();
    let mut mid = MeshBuilder::default();
    let mut mid_lamp = MeshBuilder::default();
    let mut mid_lamp_alt = MeshBuilder::default();
    let mut fine = MeshBuilder::default();
    let mut fine_lamp = MeshBuilder::default();
    let mut rng = Rng::new(100 + var);
    let mut spine = Rng::new(7);

    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        // ---- shell (always visible)
        shell.cuboid(f.p(0.0, h / 2.0, -0.5 * s), ax, [w_face / 2.0 + 3.0 * s, h / 2.0, 0.5 * s], GREY);
        shell.cuboid(f.p(0.0, h - 1.3 * s, 3.0 * s), ax, [w_face / 2.0 + 1.5 * s, 1.3 * s, 3.0 * s], GREY);
        shell.cuboid(f.p(w_face / 2.0, h / 2.0, 2.6 * s), ax, [2.4 * s, h / 2.0, 2.6 * s], DARK);
        shell_lamp.cuboid(f.p(0.0, h - 2.65 * s, 5.6 * s), ax, [w_face / 2.0 - 1.2 * s, 0.06 * s, 0.12 * s], [0.6, 0.6, 0.6, 1.0]);

        // ---- mid
        let mut c0 = -w_face / 2.0;
        while c0 < w_face / 2.0 - 1.0 {
            let cw = (rng.range(4.0, 8.0) * s).min(w_face / 2.0 - c0);
            let mut v = 0.0;
            while v < h - 0.5 * s {
                let ph = ([2.0f32, 3.0, 4.0, 5.0, 7.5][rng.int(0, 4) as usize] * s).min(h - v);
                let dep = rng.range(0.1, 0.9) * s;
                let gap = 0.06 * s;
                mid.cuboid(f.p(c0 + cw / 2.0, v + ph / 2.0, dep / 2.0), ax, [cw / 2.0 - gap, ph / 2.0 - gap, dep / 2.0], tone(&mut rng));
                v += ph;
            }
            c0 += cw;
        }
        for k in 0..7 {
            mid.cuboid(f.p(w_face / 2.0, (2.0 + k as f32 * 4.0) * s, 5.4 * s), ax, [1.5 * s, 0.25 * s, 0.25 * s], GREY);
        }
        hazard_band(&mut mid, &f, -w_face / 2.0, w_face / 2.0, h - 0.4 * s, 6.05 * s, 0.4 * s, 0.8 * s);
        let nl = (w_face / (2.4 * s)) as i32;
        for g in 0..nl {
            let u = -w_face / 2.0 + 1.2 * s + (g as f32 + 0.5) * (w_face - 2.4 * s) / nl as f32;
            if rng.chance(0.88) {
                mid_lamp.cuboid(f.p(u, h - 2.65 * s, 5.6 * s), ax, [0.55 * s, 0.06 * s, 0.12 * s], WHITE);
            }
        }
        // spine pipes run continuously through every module
        for _ in 0..((spine.int(3, 7) as f32 * per).ceil() as u32).min(40) {
            let r = [0.2, 0.3, 0.45, 0.7, 1.0, 1.4][spine.int(0, 5) as usize] * s;
            let u = spine.range(-w_face / 2.0 + 3.0 * s, w_face / 2.0 - 3.0 * s);
            let w = 1.2 * s + r + spine.range(0.0, 1.5) * s;
            let c = tone(&mut spine);
            mid.cylinder(f.p(u, 0.0, w), f.p(u, h, w), r, 8, c);
        }
        // lit window bands and readouts
        for _ in 0..((rng.int(2, 6) as f32 * per) as u32).min(60) {
            let u = rng.range(-w_face / 2.0 + 2.0 * s, w_face / 2.0 - 2.0 * s);
            let v = rng.range(s, h - 4.0 * s);
            let (nx, ny) = (rng.int(1, 6), rng.int(1, 3));
            let cell = rng.range(0.2, 0.45) * s;
            mid.cuboid(f.p(u, v, 1.05 * s), ax, [nx as f32 * cell * 0.6 + 0.15 * s, ny as f32 * cell * 0.6 + 0.15 * s, 0.15 * s], DARK);
            for a in 0..nx {
                for b in 0..ny {
                    let uu = u + (a as f32 - (nx - 1) as f32 / 2.0) * cell * 1.15;
                    let vv = v + (b as f32 - (ny - 1) as f32 / 2.0) * cell * 1.15;
                    let target = if rng.chance(0.6) { &mut mid_lamp_alt } else { &mut mid_lamp };
                    target.cuboid(f.p(uu, vv, 1.22 * s), ax, [cell * 0.45, cell * 0.4, 0.03 * s], WHITE);
                }
            }
        }
        // vents
        for _ in 0..((rng.int(0, 2) as f32 * per).round() as u32).min(20) {
            let (vw, vh) = [(5.0, 3.0), (7.0, 4.0), (4.0, 6.0)][rng.int(0, 2) as usize];
            let (vw, vh) = (vw * s, vh * s);
            let u = rng.range(-w_face / 2.0 + vw / 2.0 + 2.0 * s, w_face / 2.0 - vw / 2.0 - 2.0 * s);
            let vc = rng.range(3.0 * s, h - vh - 4.0 * s) + vh / 2.0;
            mid_lamp_alt.cuboid(f.p(u, vc, 0.75 * s), ax, [vw / 2.0, vh / 2.0, 0.05 * s], WHITE);
            for k in 0..7 {
                let vs = vc - vh / 2.0 + (k as f32 + 0.5) * vh / 7.0;
                mid.cuboid(f.p(u, vs, 1.2 * s), ax, [vw / 2.0, 0.04 * s, 0.25 * s], DARK);
            }
            hazard_band(&mut mid, &f, u - vw / 2.0 - 0.5 * s, u + vw / 2.0 + 0.5 * s, vc - vh / 2.0 - 0.3 * s, 1.25 * s, 0.3 * s, 0.8 * s);
        }

        // ---- fine (near modules only)
        for _ in 0..((rng.int(1, 3) as f32 * per) as u32).min(30) {
            let (hu, hv, hw) = (rng.range(2.5, 5.5) * s, rng.range(2.5, 7.0) * s, rng.range(1.5, 2.6) * s); // stays behind the clinical skin
            let u = rng.range(-w_face / 2.0 + hu + 2.5 * s, w_face / 2.0 - hu - 2.5 * s);
            let v = rng.range(hv + s, h - hv - 3.0 * s);
            fine.cuboid(f.p(u, v, 0.8 * s + hw), ax, [hu, hv, hw], tone(&mut rng));
            hazard_band(&mut fine, &f, u - hu, u + hu, v + hv + 0.15 * s, 0.8 * s + 2.0 * hw + 0.05 * s, 0.15 * s, 0.8 * s);
            if rng.chance(0.7) {
                fine_lamp.cuboid(f.p(u + hu * 0.55, v + hv * 0.5, 0.8 * s + 2.0 * hw + 0.1 * s), ax, [hu * 0.25, 0.25 * s, 0.05 * s], WHITE);
            }
        }
        for _ in 0..((w_face * 1.2) as u32).min(700) {
            let u = rng.range(-w_face / 2.0 + 0.5 * s, w_face / 2.0 - 0.5 * s);
            let v = rng.range(0.5 * s, h - 2.5 * s);
            let sz = rng.range(0.4f32.ln(), (2.5 * s).ln()).exp();
            let (hu, hv, hw) = (sz * rng.range(0.4, 1.4), sz * rng.range(0.3, 1.0), sz * rng.range(0.15, 0.6));
            let w0 = rng.range(0.3, 1.0) * s;
            fine.cuboid(f.p(u, v, w0 + hw), ax, [hu, hv, hw], tone(&mut rng));
        }
        // human-sized catwalk, doors and ladder: real size on purpose
        if var % 2 == 0 {
            let v = 0.4 * h;
            let wc = 1.2 * s + 1.0;
            fine.cuboid(f.p(0.0, v, wc + 1.0), ax, [w_face / 2.0 + 2.0 * s, 0.08, 1.0], DARK);
            hazard_band(&mut fine, &f, -w_face / 2.0, w_face / 2.0, v - 0.2, wc + 2.0, 0.12, 0.8);
            fine.cylinder(f.p(-w_face / 2.0, v + RAIL_H, wc + 1.95), f.p(w_face / 2.0, v + RAIL_H, wc + 1.95), 0.04, 4, HAZ_Y);
            let mut u = -w_face / 2.0;
            while u < w_face / 2.0 {
                fine.cylinder(f.p(u, v, wc + 1.95), f.p(u, v + RAIL_H, wc + 1.95), 0.035, 4, HAZ_Y);
                u += 1.8;
            }
            for _ in 0..((3.0 * per) as u32).max(3) {
                let du = rng.range(-w_face / 2.0 + 4.0, w_face / 2.0 - 4.0);
                fine.cuboid(f.p(du, v + 1.05, wc - 0.05), ax, [0.6, 1.05, 0.1], DARK);
                fine.cuboid(f.p(du, v + 2.25, wc), ax, [0.8, 0.15, 0.15], HAZ_Y);
                fine_lamp.cuboid(f.p(du + 0.9, v + 1.4, wc + 0.05), ax, [0.08, 0.08, 0.03], WHITE);
            }
            let lu = rng.range(-w_face / 3.0, w_face / 3.0);
            for side in [-0.25f32, 0.25] {
                fine.cylinder(f.p(lu + side, v, wc + 0.3), f.p(lu + side, h + v, wc + 0.3), 0.03, 4, GREY);
            }
            let mut rv = v + 0.3;
            while rv < h + v {
                fine.cylinder(f.p(lu - 0.25, rv, wc + 0.3), f.p(lu + 0.25, rv, wc + 0.3), 0.02, 4, GREY);
                rv += 0.3;
            }
        }
    }
    ModuleMeshes {
        shell: shell.build(),
        shell_lamp: shell_lamp.build(),
        mid: mid.build(),
        mid_lamp: mid_lamp.build(),
        mid_lamp_alt: mid_lamp_alt.build(),
        fine: fine.build(),
        fine_lamp: fine_lamp.build(),
    }
}

/// The platform: a plain floating disc, nothing on it.
fn build_platform() -> Mesh {
    let mut s = MeshBuilder::default();
    s.cylinder(Vec3::new(0.0, -0.3, 0.0), Vec3::ZERO, PLATFORM_R, 64, GREY);
    s.cylinder(Vec3::new(0.0, -0.55, 0.0), Vec3::new(0.0, -0.3, 0.0), PLATFORM_R - 0.25, 64, DARK);
    s.build()
}

fn build_ring(apothem: f32) -> (Mesh, Mesh) {
    let s = S;
    let w_face = face_width(apothem);
    let depth = 15.0 * s;
    let mut m = MeshBuilder::default();
    let mut l = MeshBuilder::default();
    let mut rng = Rng::new(77);
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        m.cuboid(f.p(0.0, 0.0, 4.5 * s + depth / 2.0), ax, [w_face / 2.0 - s, 3.0 * s, depth / 2.0], GREY);
        hazard_band(&mut m, &f, -w_face / 2.0 + s, w_face / 2.0 - s, -3.2 * s, 4.5 * s + depth + 0.06 * s, 0.4 * s, 0.8 * s);
        let n = (w_face / 6.0) as u32;
        for _ in 0..n {
            let u = rng.range(-w_face * 0.45, w_face * 0.45);
            let sz = rng.range(0.3, 1.6) * s;
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            m.cuboid(f.p(u, side * (3.0 * s + sz * 0.3), rng.range(5.0 * s, 4.5 * s + depth)), ax, [sz, sz * 0.3, sz * 0.8], tone(&mut rng));
        }
        let nl = (w_face / (3.0 * s)) as i32;
        for k in 0..nl {
            let u = -w_face * 0.45 + k as f32 * w_face * 0.9 / nl as f32;
            l.cuboid(f.p(u, -3.05 * s, 4.5 * s + depth * 0.7), ax, [0.5 * s, 0.05 * s, 0.5 * s], WHITE);
        }
        m.cuboid(f.p(w_face / 2.0 - 1.5 * s, 0.0, 3.4 * s), ax, [0.6 * s, 3.5 * s, 0.4 * s], DARK);
    }
    (m.build(), l.build())
}

fn figure(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    // 1.8 m tall person: legs, torso, head
    let mut m = MeshBuilder::default();
    let c = [0.9, 0.35, 0.1, 1.0];
    m.cylinder(Vec3::ZERO, Vec3::Y * 0.85, 0.15, 10, c);
    m.cylinder(Vec3::Y * 0.85, Vec3::Y * 1.5, 0.22, 12, c);
    m.cylinder(Vec3::Y * 1.55, Vec3::Y * 1.8, 0.11, 10, [0.9, 0.8, 0.7, 1.0]);
    meshes.add(m.build())
}

fn range(start: f32, end: f32) -> VisibilityRange {
    VisibilityRange { start_margin: start..start, end_margin: end..end, use_aabb: false }
}

/// Spawns everything that depends on the shaft size. Called again when the size changes.
pub fn spawn_shaft(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32) {
    const VARIANTS: u64 = 4;
    let h = MODULE_H;
    let mods: Vec<[Handle<Mesh>; 7]> = (0..VARIANTS)
        .map(|v| {
            let m = build_module(apothem, v);
            [m.shell, m.shell_lamp, m.mid, m.mid_lamp, m.mid_lamp_alt, m.fine, m.fine_lamp].map(|x| meshes.add(x))
        })
        .collect();
    let mut pick = Rng::new(3);
    for k in -MODULES_EACH_WAY..=MODULES_EACH_WAY {
        let v = if k == 0 { 0 } else { (pick.next_u64() % VARIANTS) as usize };
        let t = Transform::from_xyz(0.0, k as f32 * h + Z_OFF, 0.0);
        let m = &mods[v];
        let near = (k as f32 * h + Z_OFF).abs();
        // far shell everywhere beyond the mid range
        if near > LOD_MID - h {
            commands.spawn((ShaftPart, Mesh3d(m[0].clone()), MeshMaterial3d(mats.wall.clone()), t, range(LOD_MID, f32::MAX)));
            commands.spawn((ShaftPart, Mesh3d(m[1].clone()), MeshMaterial3d(mats.lamp.clone()), t, range(LOD_MID, f32::MAX)));
        }
        if near < LOD_MID + h {
            // the mid level reuses the shell's back wall and beam
            commands.spawn((ShaftPart, Mesh3d(m[0].clone()), MeshMaterial3d(mats.wall.clone()), t, range(0.0, LOD_MID)));
            commands.spawn((ShaftPart, Mesh3d(m[2].clone()), MeshMaterial3d(mats.wall.clone()), t, range(0.0, LOD_MID)));
            commands.spawn((ShaftPart, Mesh3d(m[3].clone()), MeshMaterial3d(mats.lamp.clone()), t, range(0.0, LOD_MID)));
            commands.spawn((ShaftPart, Mesh3d(m[4].clone()), MeshMaterial3d(mats.lamp_alt.clone()), t, range(0.0, LOD_MID)));
        }
        if near < LOD_FINE + h {
            commands.spawn((ShaftPart, Mesh3d(m[5].clone()), MeshMaterial3d(mats.wall.clone()), t, range(0.0, LOD_FINE)));
            commands.spawn((ShaftPart, Mesh3d(m[6].clone()), MeshMaterial3d(mats.lamp.clone()), t, range(0.0, LOD_FINE)));
        }
    }

    commands.spawn((ShaftPart, Mesh3d(meshes.add(build_platform())), MeshMaterial3d(mats.platform.clone())));

    crate::reconfig::spawn_shutters(commands, meshes, mats, apothem, Z_OFF);

    // 1.8 m figures on the wall catwalk of the module the platform floats in (k = 0)
    let fig = figure(meshes);
    let catwalk_y = Z_OFF + 0.4 * h + 0.08;
    let wc = 1.2 * S + 1.0;
    for i in [0usize, 3, 7] {
        let p = Facet::new(i, apothem).p(1.5, catwalk_y, wc + 1.0);
        commands.spawn((ShaftPart, Mesh3d(fig.clone()), MeshMaterial3d(mats.figure.clone()), Transform::from_translation(p).looking_at(Vec3::new(0.0, p.y, 0.0), Vec3::Y)));
    }

    // the moving ring used to test vection comfort
    let (r, rl) = build_ring(apothem);
    let (low, high) = (Z_OFF + h + 6.0 * S, Z_OFF + 6.0 * h);
    commands
        .spawn((ShaftPart, MovingRing { low, high }, Transform::from_xyz(0.0, high, 0.0), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(meshes.add(r)), MeshMaterial3d(mats.wall.clone())));
            c.spawn((Mesh3d(meshes.add(rl)), MeshMaterial3d(mats.lamp.clone())));
        });

    // a few real lights on the nearest ring beams, grazing the wall
    let reach = apothem / 45.0;
    for k in [0i32, 1] {
        for i in (0..SIDES).step_by(2) {
            let f = Facet::new(i + k as usize, apothem);
            let y = Z_OFF + k as f32 * h - 3.0 * S;
            commands.spawn((
                ShaftPart,
                PointLight { intensity: 1.0e7 * reach * reach, range: apothem * 2.5, radius: S, shadow_maps_enabled: false, ..default() },
                Transform::from_translation(f.p(0.0, y, 7.0 * S)),
            ));
        }
    }
}

pub fn ring_speed() -> f32 {
    2.0 * S.sqrt()
}
