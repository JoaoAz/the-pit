//! Procedural shaft, floating platform and scale references.
//! Units are metres, Y is up, the platform surface is at y = 0.
//!
//! Scale strategy: the shaft diameter is huge (kilometres) but the *detail* keeps a
//! fixed real-world size (ring beam every 120 m, panels of 10-30 m, windows of ~1 m,
//! human catwalks). A bigger shaft therefore shows many more elements, and the fog
//! has a real density in metres, so the far wall fades like a distant mountain.
//!
//! Irregularity: modules come in variants and every instance is turned by a random
//! multiple of 30 degrees (the shaft has 12 sides), the back wall is split into plates
//! with their own tone and texture offset, and on top of that each module near the
//! platform gets unique structures (tanks, sagging cable bundles, trusses, pipe runs,
//! smoking vents). Risers run through a random number of modules.
//!
//! Everything here is on the far render layer and reaches the headset only through
//! the panorama (see `panorama.rs`), except the platform.

use crate::meshbuild::{MeshBuilder, Rng};
use crate::panorama::{EYE, far};
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
/// Modules each way with unique structures.
const STRUCT_MODULES: i32 = 24;
/// Modules each way with smoking vents.
const SMOKE_MODULES: i32 = 10;
/// Structures must stay behind the clinical skin (26.4 m from the wall face).
const MAX_W: f32 = 23.0;

/// LOD distances (metres from the camera to the module origin, i.e. vertical distance).
const LOD_FINE: f32 = 1300.0;
const LOD_MID: f32 = 3500.0;

/// Fog density in 1/m: real atmospheric perspective, independent of shaft size.
pub fn fog_density(mood: f32) -> f32 {
    0.0008 + (0.0006 - 0.0008) * mood
}

#[derive(Component)]
pub struct ShaftPart;

#[derive(Component)]
pub struct MovingRing {
    pub low: f32,
    pub high: f32,
}

/// A wall light; its colour follows the mood and the alarm.
#[derive(Component)]
pub struct PitLight {
    pub mech: Vec3,
    pub clin: Vec3,
}

pub struct Mats {
    pub wall: Handle<StandardMaterial>,
    pub lamp: Handle<StandardMaterial>,
    pub lamp_alt: Handle<StandardMaterial>,
    pub figure: Handle<StandardMaterial>,
    pub platform: Handle<StandardMaterial>,
    /// a few slightly different whites, so the closed skin is not one flat surface
    pub skins: Vec<Handle<StandardMaterial>>,
    pub skin_line: Handle<StandardMaterial>,
    pub halo: Handle<StandardMaterial>,
    pub beam: Handle<StandardMaterial>,
    pub smoke: Handle<StandardMaterial>,
    pub grate: Handle<StandardMaterial>,
    /// big soft additive glows: the haze lit around each light
    pub haze: Handle<StandardMaterial>,
}

const GREY: [f32; 4] = [0.75, 0.75, 0.73, 1.0];
const DARK: [f32; 4] = [0.3, 0.3, 0.29, 1.0];
const HAZ_Y: [f32; 4] = [0.85, 0.6, 0.05, 1.0];
const HAZ_K: [f32; 4] = [0.06, 0.06, 0.06, 1.0];
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

/// Random surface tone: mostly greys, some darker, some rusty.
fn tone(rng: &mut Rng) -> [f32; 4] {
    let g = rng.range(0.35, 1.0);
    if rng.chance(0.15) {
        [g * 0.95, g * 0.7, g * 0.5, 1.0]
    } else {
        let warm = rng.range(-0.04, 0.04);
        [g + warm, g, g - warm, 1.0]
    }
}

fn shuffle_uv(mb: &mut MeshBuilder, rng: &mut Rng) {
    mb.uv_off = [rng.f(), rng.f()];
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

/// Splits [a, b] into random pieces between `lo` and `hi` long.
fn partition(rng: &mut Rng, a: f32, b: f32, lo: f32, hi: f32) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let mut x = a;
    while x < b - 0.01 {
        let mut w = rng.range(lo, hi);
        if b - (x + w) < lo {
            w = b - x;
        }
        out.push((x, x + w));
        x += w;
    }
    out
}

/// All LOD meshes of one module variant.
struct ModuleMeshes {
    /// far: back wall plates + ring beam, sparse lamp strips
    shell: Mesh,
    shell_lamp: Mesh,
    /// mid: panel grid, beam lamp fixtures, lit windows and vents
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
    let mut shell_lamp = MeshBuilder::glowing();
    let mut mid = MeshBuilder::default();
    let mut mid_lamp = MeshBuilder::glowing();
    let mut mid_lamp_alt = MeshBuilder::glowing();
    let mut fine = MeshBuilder::default();
    let mut fine_lamp = MeshBuilder::glowing();
    let mut rng = Rng::new(100 + var * 7919);

    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        // ---- shell (always visible): back wall in irregular plates
        for (v0, v1) in partition(&mut rng, 0.0, h, 15.0, 50.0) {
            for (u0, u1) in partition(&mut rng, -w_face / 2.0 - 3.0 * s, w_face / 2.0 + 3.0 * s, 30.0, 140.0) {
                shuffle_uv(&mut shell, &mut rng);
                let d = rng.range(0.4, 1.6) * s;
                shell.cuboid(f.p((u0 + u1) / 2.0, (v0 + v1) / 2.0, d / 2.0 - s), ax, [(u1 - u0) / 2.0 - 0.1, (v1 - v0) / 2.0 - 0.1, d / 2.0], tone(&mut rng));
            }
        }
        shell.uv_off = [0.0, 0.0];
        let beam_d = rng.range(2.4, 3.4) * s;
        shell.cuboid(f.p(0.0, h - 1.3 * s, beam_d), ax, [w_face / 2.0 + 1.5 * s, 1.3 * s, beam_d], GREY);
        shell.cuboid(f.p(w_face / 2.0, h / 2.0, 2.6 * s), ax, [2.4 * s, h / 2.0, 2.6 * s], DARK);
        for (u0, u1) in partition(&mut rng, -w_face / 2.0 + 2.0 * s, w_face / 2.0 - 2.0 * s, 20.0, 120.0) {
            if rng.chance(0.04) {
                shell_lamp.cuboid(f.p((u0 + u1) / 2.0, h - 2.65 * s, 2.0 * beam_d), ax, [(u1 - u0) / 2.0 - 2.0, 0.06 * s, 0.12 * s], [0.6, 0.6, 0.6, 1.0]);
            }
        }

        // ---- mid
        let mut c0 = -w_face / 2.0;
        while c0 < w_face / 2.0 - 1.0 {
            let cw = (rng.range(4.0, 8.0) * s).min(w_face / 2.0 - c0);
            let mut v = 0.0;
            // some columns are left bare, which breaks the regular grid
            let bare = rng.chance(0.2);
            while v < h - 0.5 * s {
                let ph = ([2.0f32, 3.0, 4.0, 5.0, 7.5, 12.0][rng.int(0, 5) as usize] * s).min(h - v);
                if !bare || rng.chance(0.15) {
                    let dep = rng.range(0.1, 1.2) * s;
                    let gap = 0.06 * s;
                    shuffle_uv(&mut mid, &mut rng);
                    mid.cuboid(f.p(c0 + cw / 2.0, v + ph / 2.0, dep / 2.0), ax, [cw / 2.0 - gap, ph / 2.0 - gap, dep / 2.0], tone(&mut rng));
                }
                v += ph;
            }
            c0 += cw;
        }
        mid.uv_off = [0.0, 0.0];
        for k in 0..7 {
            mid.cuboid(f.p(w_face / 2.0, (2.0 + k as f32 * 4.0) * s, 5.4 * s), ax, [1.5 * s, 0.25 * s, 0.25 * s], GREY);
        }
        if rng.chance(0.5) {
            hazard_band(&mut mid, &f, -w_face / 2.0, w_face / 2.0, h - 0.4 * s, 2.0 * beam_d + 0.05 * s, 0.4 * s, 0.8 * s);
        }
        let nl = (w_face / (2.4 * s)) as i32;
        // only some ring beams carry a lamp row: evenly dotted rows everywhere read as a starfield
        let lit = if rng.chance(0.06) { rng.range(0.3, 0.7) } else { 0.0 };
        for g in 0..nl {
            let u = -w_face / 2.0 + 1.2 * s + (g as f32 + 0.5) * (w_face - 2.4 * s) / nl as f32;
            if rng.chance(lit) {
                mid_lamp.cuboid(f.p(u, h - 2.65 * s, 2.0 * beam_d), ax, [0.55 * s, 0.06 * s, 0.12 * s], WHITE);
            }
        }
        // lit window bands and readouts, clustered rather than evenly spread
        let cluster_u = rng.range(-w_face / 2.0, w_face / 2.0);
        for _ in 0..((rng.int(0, 2) as f32 * per * 0.5) as u32).min(12) {
            let u = (cluster_u + rng.range(-1.0, 1.0) * rng.range(0.0, w_face * 0.5)).clamp(-w_face / 2.0 + 2.0 * s, w_face / 2.0 - 2.0 * s);
            let v = rng.range(s, h - 4.0 * s);
            let (nx, ny) = (rng.int(1, 8), rng.int(1, 3));
            let cell = rng.range(0.2, 0.45) * s;
            mid.cuboid(f.p(u, v, 1.05 * s), ax, [nx as f32 * cell * 0.6 + 0.15 * s, ny as f32 * cell * 0.6 + 0.15 * s, 0.15 * s], DARK);
            for a in 0..nx {
                for b in 0..ny {
                    if rng.chance(0.4) {
                        continue;
                    }
                    let uu = u + (a as f32 - (nx - 1) as f32 / 2.0) * cell * 1.15;
                    let vv = v + (b as f32 - (ny - 1) as f32 / 2.0) * cell * 1.15;
                    let target = if rng.chance(0.2) { &mut mid_lamp_alt } else { &mut mid_lamp };
                    target.cuboid(f.p(uu, vv, 1.22 * s), ax, [cell * 0.45, cell * 0.4, 0.03 * s], WHITE);
                }
            }
        }
        // vents
        for _ in 0..((rng.int(0, 2) as f32 * per).round() as u32).min(20) {
            let (vw, vh) = [(5.0, 3.0), (7.0, 4.0), (4.0, 6.0), (10.0, 2.0)][rng.int(0, 3) as usize];
            let (vw, vh) = (vw * s, vh * s);
            let u = rng.range(-w_face / 2.0 + vw / 2.0 + 2.0 * s, w_face / 2.0 - vw / 2.0 - 2.0 * s);
            let vc = rng.range(3.0 * s, h - vh - 4.0 * s) + vh / 2.0;
            if rng.chance(0.3) {
                mid_lamp_alt.cuboid(f.p(u, vc, 0.75 * s), ax, [vw / 2.0, vh / 2.0, 0.05 * s], WHITE);
            } else {
                mid.cuboid(f.p(u, vc, 0.75 * s), ax, [vw / 2.0, vh / 2.0, 0.05 * s], HAZ_K);
            }
            for k in 0..7 {
                let vs = vc - vh / 2.0 + (k as f32 + 0.5) * vh / 7.0;
                mid.cuboid(f.p(u, vs, 1.2 * s), ax, [vw / 2.0, 0.04 * s, 0.25 * s], DARK);
            }
            if rng.chance(0.5) {
                hazard_band(&mut mid, &f, u - vw / 2.0 - 0.5 * s, u + vw / 2.0 + 0.5 * s, vc - vh / 2.0 - 0.3 * s, 1.25 * s, 0.3 * s, 0.8 * s);
            }
        }

        // ---- fine (near modules only)
        for _ in 0..((rng.int(0, 4) as f32 * per) as u32).min(30) {
            let (hu, hv, hw) = (rng.range(2.5, 7.5) * s, rng.range(2.5, 9.0) * s, rng.range(1.0, 2.6) * s);
            let u = rng.range(-w_face / 2.0 + hu + 2.5 * s, w_face / 2.0 - hu - 2.5 * s);
            let v = rng.range(hv + s, (h - hv - 3.0 * s).max(hv + s + 0.1));
            shuffle_uv(&mut fine, &mut rng);
            fine.cuboid(f.p(u, v, 0.8 * s + hw), ax, [hu, hv, hw], tone(&mut rng));
            if rng.chance(0.4) {
                hazard_band(&mut fine, &f, u - hu, u + hu, v + hv + 0.15 * s, 0.8 * s + 2.0 * hw + 0.05 * s, 0.15 * s, 0.8 * s);
            }
            if rng.chance(0.12) {
                fine_lamp.cuboid(f.p(u + hu * 0.55, v + hv * 0.5, 0.8 * s + 2.0 * hw + 0.1 * s), ax, [hu * 0.25, 0.25 * s, 0.05 * s], WHITE);
            }
        }
        // greebles in patches of different density instead of an even sprinkle
        let patches = rng.int(2, 6);
        for _ in 0..patches {
            let pu = rng.range(-w_face / 2.0, w_face / 2.0);
            let pv = rng.range(0.0, h);
            let pr = rng.range(20.0, 120.0);
            for _ in 0..((w_face * 1.2 / patches as f32 * rng.range(0.3, 1.7)) as u32).min(400) {
                let u = (pu + rng.range(-pr, pr)).clamp(-w_face / 2.0 + 0.5 * s, w_face / 2.0 - 0.5 * s);
                let v = (pv + rng.range(-pr, pr) * 0.6).clamp(0.5 * s, h - 2.5 * s);
                let sz = rng.range(0.4f32.ln(), (2.5 * s).ln()).exp();
                let (hu, hv, hw) = (sz * rng.range(0.4, 1.4), sz * rng.range(0.3, 1.0), sz * rng.range(0.15, 0.6));
                let w0 = rng.range(0.3, 1.0) * s;
                fine.cuboid(f.p(u, v, w0 + hw), ax, [hu, hv, hw], tone(&mut rng));
            }
        }
        // human-sized catwalk, doors and ladder: real size on purpose
        if var % 2 == 0 && (i == 0 || i == 3 || i == 7 || rng.chance(0.5)) {
            fine.uv_off = [0.0, 0.0];
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

/// Unique large structures for one module (world space, module base at `y0`).
struct Structures {
    body: MeshBuilder,
    lamp: MeshBuilder,
    lamp_alt: MeshBuilder,
    smoke: MeshBuilder,
    halo: MeshBuilder,
}

/// Billboard quad facing the panorama eye (sprites only need to look right from there).
fn billboard(mb: &mut MeshBuilder, c: Vec3, half: f32, col: [f32; 4]) {
    let to_eye = (EYE - c).normalize();
    let right = Vec3::Y.cross(to_eye).normalize_or(Vec3::X);
    let up = to_eye.cross(right);
    let (r, u) = (right * half, up * half);
    mb.quad_uv([c - r - u, c + r - u, c + r + u, c - r + u], [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], to_eye, [col; 4]);
}

/// Smoke plume rising from `base`: a grid whose vertex alpha gives the shape (dense
/// at the vent, widening and fading upwards, soft sides). The texture scrolls up.
fn plume(mb: &mut MeshBuilder, rng: &mut Rng, base: Vec3, height: f32, w0: f32, w1: f32) {
    let to_eye = (EYE - base).with_y(0.0).normalize();
    let right = Vec3::Y.cross(to_eye).normalize();
    let drift = right * rng.range(-0.3, 0.3) * height;
    let (u_off, rows) = (rng.f(), 10);
    let point = |x: f32, t: f32| base + right * x * (w0 + (w1 - w0) * t) + Vec3::Y * t * height + drift * t * t;
    // squared falloff across, so the outline is a soft gradient and never the mesh edge
    let alpha = |x: f32, t: f32| (1.0 - x * x).powi(2) * (1.0 - t).powf(0.8) * (t * 8.0).min(1.0);
    let uvs = |x: f32, t: f32| [u_off + x * 0.5 * (w0 + (w1 - w0) * t) / 60.0, -t * height / 60.0];
    for r in 0..rows {
        let (t0, t1) = (r as f32 / rows as f32, (r + 1) as f32 / rows as f32);
        for c in 0..8 {
            let (x0, x1) = (-1.0 + c as f32 * 0.25, -0.75 + c as f32 * 0.25);
            let corners = [(x0, t0), (x1, t0), (x1, t1), (x0, t1)];
            mb.quad_uv(
                corners.map(|(x, t)| point(x, t)),
                corners.map(|(x, t)| uvs(x, t)),
                to_eye,
                // brighter at the bottom, where the vent light hits it
                corners.map(|(x, t)| {
                    let k = 1.0 - 0.55 * t;
                    [k, k * 0.97, k * 0.93, alpha(x, t)]
                }),
            );
        }
    }
}

fn build_structures(apothem: f32, y0: f32, seed: u64, smoke: f32) -> Structures {
    let h = MODULE_H;
    let w_face = face_width(apothem);
    let mut rng = Rng::new(seed);
    let mut st = Structures {
        body: MeshBuilder::default(),
        lamp: MeshBuilder::glowing(),
        lamp_alt: MeshBuilder::glowing(),
        smoke: MeshBuilder::default(),
        halo: MeshBuilder::default(),
    };
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        let b = &mut st.body;
        // storage tank, half sunk into the wall
        if rng.chance(0.14) {
            let r = rng.range(8.0, 19.0);
            let th = rng.range(40.0, 110.0).min(h - 10.0);
            let u = rng.range(-w_face / 2.0 + r + 10.0, w_face / 2.0 - r - 10.0);
            let w = (r * 0.25 + 2.0).min(MAX_W - r);
            let v = y0 + rng.range(4.0, h - th - 4.0);
            shuffle_uv(b, &mut rng);
            let c = tone(&mut rng);
            b.cylinder(f.p(u, v, w), f.p(u, v + th, w), r, 24, c);
            let mut bv = v + rng.range(4.0, 10.0);
            while bv < v + th - 2.0 {
                b.cylinder(f.p(u, bv, w), f.p(u, bv + 0.8, w), r + 0.4, 24, DARK);
                bv += rng.range(6.0, 16.0);
            }
            if rng.chance(0.5) {
                st.lamp_alt.cuboid(f.p(u, v + th * 0.7, w + r + 0.45), ax, [r * 0.3, 0.3, 0.05], WHITE);
            }
        }
        // sagging cable bundle between two anchor points
        if rng.chance(0.35) {
            let span = rng.range(60.0, 300.0).min(w_face - 20.0);
            let u0 = rng.range(-w_face / 2.0 + 5.0, w_face / 2.0 - span - 5.0);
            let v = y0 + rng.range(30.0, h + 20.0);
            let sag = rng.range(8.0, 45.0);
            let n = rng.int(3, 8);
            for c in 0..n {
                let r = rng.range(0.25, 0.9);
                let w = rng.range(4.0, 16.0) + c as f32 * 0.3;
                let dv = rng.range(-2.0, 2.0);
                let csag = sag * rng.range(0.8, 1.25);
                let pts: Vec<Vec3> = (0..=16)
                    .map(|k| {
                        let t = k as f32 / 16.0;
                        f.p(u0 + span * t, v + dv - csag * 4.0 * t * (1.0 - t), w)
                    })
                    .collect();
                b.tube(&pts, r, 6, if rng.chance(0.2) { HAZ_Y } else { HAZ_K });
            }
            for u in [u0, u0 + span] {
                b.cuboid(f.p(u, v, 9.0), ax, [2.0, 3.0, 9.0], DARK);
            }
        }
        // box truss gantry
        if rng.chance(0.18) {
            let len = rng.range(80.0, 360.0).min(w_face - 20.0);
            let u0 = rng.range(-w_face / 2.0 + 5.0, w_face / 2.0 - len - 5.0);
            let v = y0 + rng.range(10.0, h - 10.0);
            let (tw, thh) = (rng.range(3.0, 6.0), rng.range(4.0, 8.0));
            let w = rng.range(8.0, MAX_W - tw - 1.0);
            let r = 0.25;
            let c = tone(&mut rng);
            let chord = |a: f32, b: f32| [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(x, y)| (a * x, b * y));
            for (dw, dv) in chord(tw / 2.0, thh / 2.0) {
                b.cylinder(f.p(u0, v + dv, w + dw), f.p(u0 + len, v + dv, w + dw), r * 1.6, 5, c);
            }
            let bay = thh * 1.1;
            let mut u = u0;
            let mut flip = false;
            while u + bay <= u0 + len + 0.1 {
                for dw in [-tw / 2.0, tw / 2.0] {
                    let (a, bb) = if flip { (-thh / 2.0, thh / 2.0) } else { (thh / 2.0, -thh / 2.0) };
                    b.cylinder(f.p(u, v + a, w + dw), f.p(u + bay, v + bb, w + dw), r, 4, c);
                    b.cylinder(f.p(u, v - thh / 2.0, w + dw), f.p(u, v + thh / 2.0, w + dw), r, 4, c);
                }
                flip = !flip;
                u += bay;
            }
            if rng.chance(0.2) {
                st.lamp.cuboid(f.p(u0 + len * rng.range(0.2, 0.8), v - thh / 2.0 - 0.3, w), ax, [2.0, 0.15, 0.6], WHITE);
            }
        }
        // big pipe run: horizontal, then an elbow down the wall
        if rng.chance(0.3) {
            let r = rng.range(1.2, 4.0);
            let len = rng.range(60.0, 300.0).min(w_face - 30.0);
            let u0 = rng.range(-w_face / 2.0 + 10.0, w_face / 2.0 - len - 10.0);
            let v = y0 + rng.range(10.0, h);
            let w = rng.range(r + 2.0, MAX_W - r);
            let drop = rng.range(30.0, 160.0);
            let c = tone(&mut rng);
            shuffle_uv(b, &mut rng);
            b.tube(&[f.p(u0, v - drop, w), f.p(u0, v, w), f.p(u0 + len, v, w)], r, 12, c);
            let mut fu = u0 + 8.0;
            while fu < u0 + len {
                b.cylinder(f.p(fu, v, w), f.p(fu + 0.6, v, w), r * 1.25, 12, DARK);
                fu += rng.range(10.0, 25.0);
            }
        }
        // smoking vent (more of them near the platform level)
        if smoke > 0.0 && rng.chance(smoke) {
            let (vw, vh) = (rng.range(12.0, 30.0), rng.range(6.0, 14.0));
            let u = rng.range(-w_face / 2.0 + vw, w_face / 2.0 - vw);
            let v = y0 + rng.range(10.0, h - vh - 10.0);
            st.lamp_alt.cuboid(f.p(u, v, 4.2), ax, [vw / 2.0, vh / 2.0, 0.1], WHITE);
            for k in 0..6 {
                let vs = v - vh / 2.0 + (k as f32 + 0.5) * vh / 6.0;
                b.cuboid(f.p(u, vs, 5.0), ax, [vw / 2.0, 0.15, 0.8], DARK);
            }
            b.cuboid(f.p(u, v, 2.0), ax, [vw / 2.0 + 1.5, vh / 2.0 + 1.5, 2.0], DARK);
            billboard(&mut st.halo, f.p(u, v, 8.0), vw * 1.2, [1.0, 0.7, 0.5, 0.6]);
            let base = f.p(u, v + vh / 2.0, 7.0);
            let (ph, pw) = (rng.range(140.0, 340.0), vw * rng.range(2.5, 4.5));
            plume(&mut st.smoke, &mut rng, base, ph, vw * 0.7, pw);
            if rng.chance(0.5) {
                // a second, thinner puff for depth
                let (ph2, pw2) = (ph * 0.6, pw * 0.6);
                plume(&mut st.smoke, &mut rng, base + f.n * 3.0, ph2, vw * 0.4, pw2);
            }
        }
    }
    st
}

/// Vertical lattice towers along the wall, spanning a random number of modules near
/// the platform, with dashed light strips running up their edges (white and amber).
fn build_towers(apothem: f32) -> (MeshBuilder, MeshBuilder, MeshBuilder) {
    let w_face = face_width(apothem);
    let (mut body, mut lamp, mut amber) = (MeshBuilder::default(), MeshBuilder::glowing(), MeshBuilder::glowing());
    let mut rng = Rng::new(909);
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        for _ in 0..rng.int(1, 3) {
            let (tw, td) = (rng.range(6.0, 12.0), rng.range(5.0, 8.0));
            let u = rng.range(-w_face / 2.0 + 20.0, w_face / 2.0 - 20.0);
            let w0 = rng.range(6.0, MAX_W - td - 1.0);
            let span = rng.range(3.0, 16.0) * MODULE_H;
            let y0 = rng.range(-14.0 * MODULE_H, 14.0 * MODULE_H - span);
            let bay = rng.range(8.0, 14.0);
            let c = tone(&mut rng);
            let r = rng.range(0.35, 0.6);
            let corners = [(-tw / 2.0, w0), (tw / 2.0, w0), (-tw / 2.0, w0 + td), (tw / 2.0, w0 + td)];
            for (du, w) in corners {
                body.cylinder(f.p(u + du, y0, w), f.p(u + du, y0 + span, w), r * 1.6, 6, c);
            }
            let mut y = y0;
            let mut flip = false;
            while y + bay <= y0 + span {
                // zigzag on the face towards the centre and on one side, rungs on all four
                let (a, b) = if flip { (-tw / 2.0, tw / 2.0) } else { (tw / 2.0, -tw / 2.0) };
                body.cylinder(f.p(u + a, y, w0 + td), f.p(u + b, y + bay, w0 + td), r, 4, c);
                let side = if flip { w0 } else { w0 + td };
                body.cylinder(f.p(u - tw / 2.0, y, side), f.p(u - tw / 2.0, y + bay, w0 + w0 + td - side), r, 4, c);
                body.cylinder(f.p(u - tw / 2.0, y, w0 + td), f.p(u + tw / 2.0, y, w0 + td), r, 4, c);
                body.cylinder(f.p(u - tw / 2.0, y, w0), f.p(u - tw / 2.0, y, w0 + td), r, 4, c);
                flip = !flip;
                y += bay;
            }
            // dashed light strips on the two front edges
            let dash = rng.range(3.0, 6.0);
            let warm = rng.chance(0.35);
            for du in [-tw / 2.0, tw / 2.0] {
                let mut y = y0 + dash;
                while y < y0 + span - dash {
                    if rng.chance(0.3) {
                        let target = if warm { &mut amber } else { &mut lamp };
                        target.cuboid(f.p(u + du, y, w0 + td + r * 1.7), ax, [0.18, dash * 0.32, 0.06], WHITE);
                    }
                    y += dash;
                }
            }
        }
    }
    (body, lamp, amber)
}

/// A massive pilaster on the wall: one of the big vertical masses that give the
/// wall its rhythm and its deep shadows.
#[derive(Clone, Copy)]
struct Pilaster {
    facet: usize,
    u: f32,
    half_w: f32,
    depth: f32,
}

const PILASTER_MODULES: i32 = 50;

fn pilasters(apothem: f32) -> Vec<Pilaster> {
    let w_face = face_width(apothem);
    let mut rng = Rng::new(31337);
    let mut out = Vec::new();
    for facet in 0..SIDES {
        for _ in 0..rng.int(1, 3) {
            let half_w = rng.range(14.0, 32.0);
            out.push(Pilaster { facet, u: rng.range(-w_face / 2.0 + 60.0, w_face / 2.0 - 60.0), half_w, depth: rng.range(12.0, MAX_W - 1.0) });
        }
    }
    out
}

/// Pilasters clad in 40 m plates (each with its own depth and tone), banded at every
/// ring beam, plus bundles of big pipes running beside them.
fn build_risers(apothem: f32) -> MeshBuilder {
    let w_face = face_width(apothem);
    let mut mb = MeshBuilder::default();
    let mut rng = Rng::new(7);
    for p in pilasters(apothem) {
        let f = Facet::new(p.facet, apothem);
        let ax = f.ax();
        let mut y = -PILASTER_MODULES as f32 * MODULE_H + Z_OFF;
        while y < PILASTER_MODULES as f32 * MODULE_H {
            for (a, b) in partition(&mut rng, y, y + MODULE_H, 18.0, 50.0) {
                shuffle_uv(&mut mb, &mut rng);
                let d = p.depth - rng.range(0.0, 2.5);
                mb.cuboid(f.p(p.u, (a + b) / 2.0, d / 2.0), ax, [p.half_w - rng.range(0.0, 1.5), (b - a) / 2.0 - 0.3, d / 2.0], tone(&mut rng));
            }
            mb.cuboid(f.p(p.u, y + MODULE_H - 3.0, p.depth / 2.0 + 0.5), ax, [p.half_w + 1.5, 3.0, p.depth / 2.0 + 0.5], DARK);
            y += MODULE_H;
        }
        // a pipe bundle on one side of the pilaster
        let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
        let n = rng.int(3, 7);
        let mut u = p.u + side * (p.half_w + 2.0);
        let span = rng.range(10.0, 2.0 * PILASTER_MODULES as f32) * MODULE_H;
        let y0 = rng.range(-(PILASTER_MODULES as f32) * MODULE_H, PILASTER_MODULES as f32 * MODULE_H - span);
        for _ in 0..n {
            let r = rng.range(1.5, 5.0);
            u += side * r;
            let w = (r + rng.range(1.0, 4.0)).min(MAX_W - r);
            let c = tone(&mut rng);
            mb.cylinder(f.p(u, y0, w), f.p(u, y0 + span, w), r, 12, c);
            let mut yy = (y0 / MODULE_H).ceil() * MODULE_H + Z_OFF;
            while yy < y0 + span {
                mb.cylinder(f.p(u, yy - 1.2, w), f.p(u, yy + 1.2, w), r * 1.25, 12, DARK);
                yy += MODULE_H / 2.0;
            }
            u += side * (r + 0.6);
        }
    }
    // loose risers elsewhere
    let span = MODULES_EACH_WAY as f32 * MODULE_H;
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        for _ in 0..rng.int(2, 5) {
            let r = [0.8, 1.2, 1.8, 2.8][rng.int(0, 3) as usize];
            let u = rng.range(-w_face / 2.0 + 12.0, w_face / 2.0 - 12.0);
            let w = (4.8 + r + rng.range(0.0, 6.0)).min(MAX_W - r);
            let len = rng.range(2.0, 30.0) * MODULE_H;
            let y0 = rng.range(-span, span - len);
            let c = tone(&mut rng);
            mb.cylinder(f.p(u, y0, w), f.p(u, y0 + len, w), r, 10, c);
        }
    }
    mb
}

pub const GRATE_R: f32 = PLATFORM_R - 0.12;

/// The platform frame: a rim ring, radial beams and a hub under the grating (the
/// grating itself is a separate alpha-tested disc). No railings, nothing on top.
fn build_platform() -> Mesh {
    let mut s = MeshBuilder::default();
    let segs = 64;
    for k in 0..segs {
        let a0 = TAU * k as f32 / segs as f32;
        let a1 = TAU * (k + 1) as f32 / segs as f32;
        let am = (a0 + a1) / 2.0;
        let d = Vec3::new(am.cos(), 0.0, am.sin());
        let t = Vec3::new(-am.sin(), 0.0, am.cos());
        let len = PLATFORM_R * (a1 - a0) / 2.0 + 0.01;
        // rim: a box section flush with the grating, with a lip below
        s.cuboid(d * (PLATFORM_R - 0.06) + Vec3::Y * -0.11, [t, Vec3::Y, d], [len, 0.11, 0.06], GREY);
        s.cuboid(d * (PLATFORM_R - 0.25) + Vec3::Y * -0.27, [t, Vec3::Y, d], [len * 0.9, 0.05, 0.2], DARK);
        // inner ring beam
        let ri = 1.15;
        s.cuboid(d * ri + Vec3::Y * -0.09, [t, Vec3::Y, d], [ri * (a1 - a0) / 2.0 + 0.01, 0.07, 0.025], DARK);
    }
    // radial I-beams under the grating
    for k in 0..12 {
        let a = TAU * k as f32 / 12.0;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        let t = Vec3::new(-a.sin(), 0.0, a.cos());
        let c = d * (PLATFORM_R / 2.0);
        let ax = [d, Vec3::Y, t];
        s.cuboid(c + Vec3::Y * -0.13, ax, [PLATFORM_R / 2.0 - 0.06, 0.11, 0.008], DARK);
        s.cuboid(c + Vec3::Y * -0.24, ax, [PLATFORM_R / 2.0 - 0.06, 0.008, 0.05], DARK);
        s.cuboid(c + Vec3::Y * -0.015, ax, [PLATFORM_R / 2.0 - 0.06, 0.008, 0.04], DARK);
    }
    // hub and a short keel below, so the disc has mass
    s.cylinder(Vec3::new(0.0, -0.32, 0.0), Vec3::new(0.0, -0.02, 0.0), 0.3, 24, DARK);
    s.cylinder(Vec3::new(0.0, -0.75, 0.0), Vec3::new(0.0, -0.32, 0.0), 0.18, 16, DARK);
    s.build()
}

fn build_ring(apothem: f32) -> (Mesh, Mesh) {
    let s = S;
    let w_face = face_width(apothem);
    let depth = 15.0 * s;
    let mut m = MeshBuilder::default();
    let mut l = MeshBuilder::glowing();
    let mut rng = Rng::new(77);
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        shuffle_uv(&mut m, &mut rng);
        m.cuboid(f.p(0.0, 0.0, 4.5 * s + depth / 2.0), ax, [w_face / 2.0 - s, 3.0 * s, depth / 2.0], tone(&mut rng));
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
            if rng.chance(0.15) {
                let u = -w_face * 0.45 + k as f32 * w_face * 0.9 / nl as f32;
                l.cuboid(f.p(u, -3.05 * s, 4.5 * s + depth * 0.7), ax, [0.5 * s, 0.05 * s, 0.5 * s], WHITE);
            }
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
    const VARIANTS: u64 = 8;
    let h = MODULE_H;
    let mods: Vec<[Handle<Mesh>; 7]> = (0..VARIANTS)
        .map(|v| {
            let m = build_module(apothem, v);
            [m.shell, m.shell_lamp, m.mid, m.mid_lamp, m.mid_lamp_alt, m.fine, m.fine_lamp].map(|x| meshes.add(x))
        })
        .collect();
    let mut pick = Rng::new(3);
    let part = |commands: &mut Commands, mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, t: Transform, r: Option<VisibilityRange>| {
        let mut e = commands.spawn((ShaftPart, far(), Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), t));
        if let Some(r) = r {
            e.insert(r);
        }
    };
    for k in -MODULES_EACH_WAY..=MODULES_EACH_WAY {
        // the platform's module keeps variant 0 unturned: the scale figures stand on its catwalk
        let (v, turn) = if k == 0 { (0, 0) } else { ((pick.next_u64() % VARIANTS) as usize, pick.int(0, 11)) };
        let y = k as f32 * h + Z_OFF;
        let t = Transform::from_xyz(0.0, y, 0.0).with_rotation(Quat::from_rotation_y(turn as f32 * TAU / SIDES as f32));
        let m = &mods[v];
        let near = y.abs();
        if near > LOD_MID - h {
            part(commands, &m[0], &mats.wall, t, Some(range(LOD_MID, f32::MAX)));
            part(commands, &m[1], &mats.lamp, t, Some(range(LOD_MID, f32::MAX)));
        }
        if near < LOD_MID + h {
            // the mid level reuses the shell's back wall and beam
            part(commands, &m[0], &mats.wall, t, Some(range(0.0, LOD_MID)));
            part(commands, &m[2], &mats.wall, t, Some(range(0.0, LOD_MID)));
            part(commands, &m[3], &mats.lamp, t, Some(range(0.0, LOD_MID)));
            part(commands, &m[4], &mats.lamp_alt, t, Some(range(0.0, LOD_MID)));
        }
        if near < LOD_FINE + h {
            part(commands, &m[5], &mats.wall, t, Some(range(0.0, LOD_FINE)));
            part(commands, &m[6], &mats.lamp, t, Some(range(0.0, LOD_FINE)));
        }
        if k.abs() <= STRUCT_MODULES {
            let smoke = if k.abs() <= 3 { 0.35 } else if k.abs() <= SMOKE_MODULES { 0.12 } else { 0.0 };
            let st = build_structures(apothem, y, 1000 + (k + 500) as u64, smoke);
            for (mb, mat) in [(st.body, &mats.wall), (st.lamp, &mats.lamp), (st.lamp_alt, &mats.lamp_alt), (st.smoke, &mats.smoke), (st.halo, &mats.halo)] {
                if !mb.is_empty() {
                    part(commands, &meshes.add(mb.build()), mat, Transform::IDENTITY, None);
                }
            }
        }
    }
    part(commands, &meshes.add(build_risers(apothem).build()), &mats.wall, Transform::IDENTITY, None);
    let (tb, tl, ta) = build_towers(apothem);
    for (mb, mat) in [(tb, &mats.wall), (tl, &mats.lamp), (ta, &mats.lamp_alt)] {
        part(commands, &meshes.add(mb.build()), mat, Transform::IDENTITY, None);
    }

    // the platform is the only thing drawn live in the headset (default layer)
    commands.spawn((ShaftPart, Mesh3d(meshes.add(build_platform())), MeshMaterial3d(mats.platform.clone())));
    commands.spawn((
        ShaftPart,
        Mesh3d(meshes.add(Circle::new(GRATE_R))),
        MeshMaterial3d(mats.grate.clone()),
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    crate::reconfig::spawn_shutters(commands, meshes, mats, apothem, Z_OFF);

    // 1.8 m figures on the wall catwalk of the module the platform floats in (k = 0)
    let fig = figure(meshes);
    let catwalk_y = Z_OFF + 0.4 * h + 0.08;
    let wc = 1.2 * S + 1.0;
    for i in [0usize, 3, 7] {
        let p = Facet::new(i, apothem).p(1.5, catwalk_y, wc + 1.0);
        part(commands, &fig, &mats.figure, Transform::from_translation(p).looking_at(Vec3::new(0.0, p.y, 0.0), Vec3::Y), None);
    }

    // the moving ring
    let (r, rl) = build_ring(apothem);
    let (low, high) = (Z_OFF + h + 6.0 * S, Z_OFF + 6.0 * h);
    commands
        .spawn((ShaftPart, far(), MovingRing { low, high }, Transform::from_xyz(0.0, high, 0.0), Visibility::default()))
        .with_children(|c| {
            c.spawn((far(), Mesh3d(meshes.add(r)), MeshMaterial3d(mats.wall.clone())));
            c.spawn((far(), Mesh3d(meshes.add(rl)), MeshMaterial3d(mats.lamp.clone())));
        });

    spawn_lights(commands, meshes, mats, apothem);
    spawn_suspended(commands, meshes, mats, apothem);
}

/// Giant structures hanging in the void between the platform and the wall (60-300 m
/// away), drawn live in the headset (not in the panorama) because they are close
/// enough for stereo depth: thick cables with clamps falling from far above, chains,
/// and two lattice girders crossing the whole shaft above and below the platform.
/// They carry the close-up detail and the oppressive mass; the walls stay far away.
fn spawn_suspended(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32) {
    let mut body = MeshBuilder::default();
    let mut lamps = MeshBuilder::glowing();
    let mut amber = MeshBuilder::glowing();
    let mut rng = Rng::new(555);
    let reach = 3500.0;
    let mut lamps_live: Vec<Vec3> = Vec::new();
    let mut haze = MeshBuilder::default();
    let mut halo = MeshBuilder::default();
    let mut steam = MeshBuilder::default();
    // cables
    for k in 0..7 {
        let a = TAU * (k as f32 + rng.range(-0.3, 0.3)) / 7.0;
        let d = rng.range(70.0, 280.0);
        let c = Vec3::new(a.cos() * d, 0.0, a.sin() * d);
        let r = rng.range(1.2, 3.2);
        let n = if rng.chance(0.4) { rng.int(2, 4) } else { 1 };
        for j in 0..n {
            let off = Vec3::new(j as f32 * r * 2.4, 0.0, 0.0);
            body.cylinder(c + off - Vec3::Y * reach, c + off + Vec3::Y * reach, r, 12, [0.42, 0.4, 0.37, 1.0]);
            let mut y = -reach + rng.range(0.0, 30.0);
            while y < reach {
                body.cylinder(c + off + Vec3::Y * y, c + off + Vec3::Y * (y + 1.6), r * 1.35, 12, DARK);
                y += rng.range(18.0, 40.0);
            }
        }
        // secondary detail where it can be seen: thin service cables running beside the
        // main one, junction boxes with cooling fins, brackets and an occasional cage
        {
            let to_c = Vec3::new(c.x, 0.0, c.z).normalize();
            let tang = Vec3::new(-to_c.z, 0.0, to_c.x);
            let ns = rng.int(2, 4);
            for j in 0..ns {
                let ang = rng.range(0.0, TAU);
                let o = (to_c * ang.cos() + tang * ang.sin()) * (r + rng.range(0.6, 1.4));
                let rs = rng.range(0.12, 0.3);
                let _ = j;
                body.cylinder(c + o - Vec3::Y * 900.0, c + o + Vec3::Y * 900.0, rs, 6, GREY);
            }
            let mut y = -600.0 + rng.range(0.0, 40.0);
            while y < 600.0 {
                // denser close to the platform's level, where it can be seen
                let face = if rng.chance(0.5) { to_c } else { tang };
                let sgn = if rng.chance(0.5) { 1.0 } else { -1.0 };
                let fax = [face.cross(Vec3::Y).normalize(), Vec3::Y, face * sgn];
                match rng.int(0, 2) {
                    0 => {
                        // junction box with fins and a conduit back into the cable
                        let (bw, bh, bd) = (rng.range(1.2, 2.2), rng.range(2.0, 4.0), rng.range(0.8, 1.4));
                        let bc = c + Vec3::Y * y + face * sgn * (r * 1.35 + bd);
                        body.cuboid(bc, fax, [bw, bh, bd], tone(&mut rng));
                        let nf = rng.int(4, 8);
                        for f in 0..nf {
                            let fy = -bh + 0.4 + f as f32 * (2.0 * bh - 0.8) / nf as f32;
                            body.cuboid(bc + Vec3::Y * fy + fax[2] * (bd + 0.15), fax, [bw * 0.9, 0.06, 0.15], DARK);
                        }
                        body.cylinder(bc - Vec3::Y * (bh + 1.5), bc - Vec3::Y * bh, 0.2, 6, DARK);
                    }
                    1 => {
                        // bracket arm with a hanging counterweight
                        let arm = rng.range(4.0, 9.0);
                        let p0 = c + Vec3::Y * y + face * sgn * r;
                        let p1 = p0 + face * sgn * arm;
                        body.cuboid((p0 + p1) / 2.0, fax, [0.25, 0.5, arm / 2.0], DARK);
                        body.cylinder(p0 - Vec3::Y * 3.0, p1, 0.15, 5, GREY);
                        let drop = rng.range(4.0, 12.0);
                        body.cylinder(p1, p1 - Vec3::Y * drop, 0.06, 4, GREY);
                        body.cuboid(p1 - Vec3::Y * (drop + 0.8), fax, [0.7, 0.8, 0.7], tone(&mut rng));
                    }
                    _ => {
                        // maintenance cage around the cable: rings and bars
                        let cr = r * 1.35 + 2.0;
                        let ch = rng.range(3.0, 5.0);
                        let ring = |mb: &mut MeshBuilder, yy: f32| {
                            let seg = 12;
                            for i in 0..seg {
                                let a0 = TAU * i as f32 / seg as f32;
                                let a1 = TAU * (i + 1) as f32 / seg as f32;
                                let q0 = c + Vec3::new(a0.cos() * cr, yy, a0.sin() * cr);
                                let q1 = c + Vec3::new(a1.cos() * cr, yy, a1.sin() * cr);
                                mb.cylinder(q0, q1, 0.08, 4, GREY);
                            }
                        };
                        ring(&mut body, y);
                        ring(&mut body, y + ch);
                        ring(&mut body, y + ch * 0.5);
                        for i in 0..8 {
                            let a0 = TAU * i as f32 / 8.0;
                            let q = c + Vec3::new(a0.cos() * cr, y, a0.sin() * cr);
                            body.cylinder(q, q + Vec3::Y * ch, 0.08, 4, GREY);
                        }
                        // floor grating under it
                        for i in 0..6 {
                            let a0 = TAU * i as f32 / 6.0;
                            let q = c + Vec3::new(a0.cos() * cr, y - 0.1, a0.sin() * cr);
                            body.cylinder(c + Vec3::Y * (y - 0.1), q, 0.12, 4, DARK);
                        }
                    }
                }
                y += if y.abs() < 220.0 { rng.range(9.0, 22.0) } else { rng.range(25.0, 70.0) };
            }
        }
        // a narrow flood high above, grazing down the cable
        {
            let to_c = Vec3::new(c.x, 0.0, c.z).normalize();
            let lp = c + Vec3::Y * rng.range(250.0, 400.0) - to_c * 14.0;
            let target = c + Vec3::Y * rng.range(-200.0, 0.0);
            commands.spawn((
                ShaftPart,
                PitLight { mech: Vec3::new(1.0, 0.55, 0.2), clin: Vec3::new(0.85, 0.93, 1.0) },
                SpotLight {
                    color: Color::linear_rgb(1.0, 0.55, 0.2),
                    intensity: 6.0e8,
                    range: 800.0,
                    radius: 1.0,
                    inner_angle: 0.02,
                    outer_angle: 0.06,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(lp).looking_at(target, Vec3::X),
            ));
        }
        // work lamps clamped to the cable: warm pools on its surface and a glow of haze
        for _ in 0..2 {
            let ly = rng.range(-180.0, 220.0);
            let to_c = (Vec3::new(c.x, 0.0, c.z)).normalize();
            let lp = c + Vec3::Y * ly + to_c * (r * 2.0 + 1.0) + Vec3::new(-to_c.z, 0.0, to_c.x) * r * 1.5;
            lamps_live.push(lp);
        }
        // a few marker lights along the cable
        let mut y = rng.range(-400.0, -200.0);
        while y < 500.0 {
            if rng.chance(0.5) {
                amber.cuboid(c + Vec3::Y * y + Vec3::new(0.0, 0.0, r * 1.4), [Vec3::X, Vec3::Y, Vec3::Z], [0.25, 0.6, 0.25], WHITE);
            }
            y += rng.range(40.0, 90.0);
        }
    }
    // chains: alternating flat links
    for _ in 0..3 {
        let a = rng.range(0.0, TAU);
        let d = rng.range(110.0, 260.0);
        let c = Vec3::new(a.cos() * d, 0.0, a.sin() * d);
        let link = rng.range(2.5, 4.5);
        let mut y = -1500.0;
        let mut flip = false;
        while y < 1500.0 {
            let ax = if flip { [Vec3::X, Vec3::Y, Vec3::Z] } else { [Vec3::Z, Vec3::Y, Vec3::X] };
            let p = c + Vec3::Y * y;
            for sx in [-1.0f32, 1.0] {
                body.cuboid(p + ax[0] * sx * link * 0.35, ax, [link * 0.12, link * 0.55, link * 0.12], DARK);
            }
            for sy in [-1.0f32, 1.0] {
                body.cuboid(p + Vec3::Y * sy * link * 0.5, ax, [link * 0.35, link * 0.1, link * 0.12], DARK);
            }
            flip = !flip;
            y += link * 0.85;
        }
    }
    // two lattice girders spanning wall to wall
    for (y, off, ang, w, h) in [(170.0f32, 120.0f32, 0.4f32, 14.0f32, 18.0f32), (-230.0, -160.0, 1.9, 10.0, 14.0)] {
        let dir = Vec3::new(ang.cos(), 0.0, ang.sin());
        let side = Vec3::new(-ang.sin(), 0.0, ang.cos());
        let base = side * off + Vec3::Y * y;
        let half = apothem;
        let chords = [(-w / 2.0, -h / 2.0), (w / 2.0, -h / 2.0), (w / 2.0, h / 2.0), (-w / 2.0, h / 2.0)];
        let p = |t: f32, a: f32, b: f32| base + dir * t + side * a + Vec3::Y * b;
        for (a, b) in chords {
            body.cylinder(p(-half, a, b), p(half, a, b), 1.1, 8, DARK);
        }
        let bay = h * 1.1;
        let mut t = -half;
        let mut flip = false;
        while t < half {
            for a in [-w / 2.0, w / 2.0] {
                let (b0, b1) = if flip { (-h / 2.0, h / 2.0) } else { (h / 2.0, -h / 2.0) };
                body.cylinder(p(t, a, b0), p(t + bay, a, b1), 0.55, 6, GREY);
                body.cylinder(p(t, a, -h / 2.0), p(t, a, h / 2.0), 0.5, 6, GREY);
            }
            body.cylinder(p(t, -w / 2.0, -h / 2.0), p(t, w / 2.0, -h / 2.0), 0.45, 6, GREY);
            body.cylinder(p(t, -w / 2.0, h / 2.0), p(t, w / 2.0, h / 2.0), 0.45, 6, GREY);
            // dashed strip under the lower chords
            if !flip {
                lamps.cuboid(p(t + bay / 2.0, -w / 2.0, -h / 2.0 - 1.3), [dir, Vec3::Y, side], [bay * 0.3, 0.12, 0.25], WHITE);
                lamps.cuboid(p(t + bay / 2.0, w / 2.0, -h / 2.0 - 1.3), [dir, Vec3::Y, side], [bay * 0.3, 0.12, 0.25], WHITE);
            }
            flip = !flip;
            t += bay;
        }
        // gusset plates at every node, a pipe run along one side, hangers
        let mut t = -half;
        while t < half {
            for a in [-w / 2.0, w / 2.0] {
                for bb in [-h / 2.0, h / 2.0] {
                    body.cuboid(p(t, a, bb), [dir, Vec3::Y, side], [1.6, 1.6, 0.25], DARK);
                }
            }
            t += bay;
        }
        for (k, pr) in [(0.0f32, 0.9f32), (2.4, 0.6), (4.2, 0.45)] {
            body.cylinder(p(-half, -w / 2.0 - 2.0 - k * 0.3, -h / 2.0 + 2.0 + k), p(half, -w / 2.0 - 2.0 - k * 0.3, -h / 2.0 + 2.0 + k), pr, 8, GREY);
        }
        let mut t = -half + bay * 0.5;
        while t < half {
            body.cuboid(p(t, -w / 2.0 - 1.2, -h / 2.0 + 3.0), [dir, Vec3::Y, side], [0.3, 4.0, 1.2], DARK);
            t += bay * 2.0;
        }
        // a hanging service gondola under the girder
        {
            let gt = rng.range(-120.0, 120.0);
            let top = p(gt, 0.0, -h / 2.0);
            let drop = rng.range(30.0, 60.0);
            let g0 = top - Vec3::Y * drop;
            for sx in [-1.0f32, 1.0] {
                body.cylinder(top + dir * sx * 3.0, g0 + dir * sx * 3.0 + Vec3::Y * 2.5, 0.12, 4, GREY);
            }
            body.cuboid(g0, [dir, Vec3::Y, side], [4.0, 1.25, 2.0], tone(&mut rng));
            body.cuboid(g0 + Vec3::Y * 1.6, [dir, Vec3::Y, side], [4.1, 0.08, 2.1], DARK);
            for sx in [-3.8f32, 0.0, 3.8] {
                body.cylinder(g0 + dir * sx + side * 2.0 + Vec3::Y * 1.25, g0 + dir * sx + side * 2.0 + Vec3::Y * 2.4, 0.06, 4, GREY);
            }
            body.cylinder(g0 + side * 2.0 + Vec3::Y * 2.4 - dir * 3.8, g0 + side * 2.0 + Vec3::Y * 2.4 + dir * 3.8, 0.06, 4, GREY);
            lamps_live.push(g0 - Vec3::Y * 2.0);
        }
        // steam venting from the girder's pipes
        for t in [rng.range(-300.0f32, -80.0), rng.range(60.0, 260.0)] {
            let b = p(t, 0.0, h / 2.0 + 1.0);
            let (ph, pw) = (rng.range(120.0, 220.0), rng.range(40.0, 70.0));
            plume(&mut steam, &mut rng, b, ph, 6.0, pw);
            lamps_live.push(p(t, w / 2.0 + 3.0, h / 2.0 + 6.0));
        }
        // sodium floods hung under the girder, close to the platform's side
        for t in [-60.0f32, 90.0] {
            let pos = p(t, 0.0, -h / 2.0 - 3.0);
            commands.spawn((
                ShaftPart,
                PitLight { mech: Vec3::new(1.0, 0.55, 0.2), clin: Vec3::new(0.85, 0.93, 1.0) },
                SpotLight {
                    color: Color::linear_rgb(1.0, 0.55, 0.2),
                    intensity: 4.0e8,
                    range: 500.0,
                    radius: 1.0,
                    inner_angle: 0.2,
                    outer_angle: 0.7,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(pos).looking_to(-Vec3::Y, dir),
            ));
        }
    }
    for lp in lamps_live {
        commands.spawn((
            ShaftPart,
            PitLight { mech: Vec3::new(1.0, 0.55, 0.2), clin: Vec3::new(0.85, 0.93, 1.0) },
            PointLight { color: Color::linear_rgb(1.0, 0.55, 0.2), intensity: 1.5e7, range: 220.0, radius: 0.5, shadow_maps_enabled: false, ..default() },
            Transform::from_translation(lp),
        ));
        billboard(&mut halo, lp, 1.6, WHITE);
        billboard(&mut haze, lp, rng.range(25.0, 45.0), [1.0, 1.0, 1.0, 0.6]);
    }
    for (mb, mat) in [(body, &mats.wall), (lamps, &mats.lamp), (amber, &mats.lamp_alt), (haze, &mats.haze), (halo, &mats.halo), (steam, &mats.smoke)] {
        commands.spawn((ShaftPart, Mesh3d(meshes.add(mb.build())), MeshMaterial3d(mat.clone())));
    }
}

/// Oppressive, mysterious light: hidden sodium floods mounted on the pilasters throw
/// long warm streaks up and down their faces and the wall beside them, with a faint
/// beam in the haze; deep shadow everywhere else. A few red lamps. Vents near the
/// platform level breathe steam lit from behind, between the plume and the wall.
fn spawn_lights(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32) {
    let w_face = face_width(apothem);
    let mut rng = Rng::new(4242);
    let mut halos = MeshBuilder::default();
    let mut haze = MeshBuilder::default();
    let mut beams = MeshBuilder::default();
    let mut smoke = MeshBuilder::default();
    let mut vents = MeshBuilder::default();
    let mut vent_lamps = MeshBuilder::glowing();
    let sodium = Vec3::new(1.0, 0.55, 0.2);
    let cold = Vec3::new(0.85, 0.93, 1.0);
    for p in pilasters(apothem) {
        let f = Facet::new(p.facet, apothem);
        for _ in 0..rng.int(2, 4) {
            let y = rng.range(-1.0f32, 1.0).powi(3) * 6.0 * MODULE_H;
            // fixture tucked against the pilaster's side, aimed along its face
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let pos = f.p(p.u + side * (p.half_w + 3.0), y, p.depth + 2.0);
            let up = rng.chance(0.4);
            let dir = (Vec3::Y * if up { 1.0 } else { -1.0 } + f.n * -0.18 + f.t * -side * 0.12).normalize();
            let reach = rng.range(300.0, 520.0);
            let red = rng.chance(0.08);
            let mech = if red { Vec3::new(1.0, 0.12, 0.04) } else { sodium };
            commands.spawn((
                ShaftPart,
                far(),
                PitLight { mech, clin: cold },
                SpotLight {
                    color: Color::linear_rgb(mech.x, mech.y, mech.z),
                    intensity: 1.2e10,
                    range: reach * 1.3,
                    radius: 2.0,
                    inner_angle: 0.1,
                    outer_angle: 0.5,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(pos).looking_to(dir, f.n),
            ));
            let tint = if red { [1.0, 0.3, 0.2] } else { [1.0, 1.0, 1.0] };
            let len = reach * 0.6;
            let _ = (&mut beams, len);
            // the flood lights a pocket of haze around its throw: a soft pool, not a cone
            billboard(&mut haze, pos + dir * 30.0, rng.range(40.0, 70.0), [tint[0], tint[1], tint[2], 0.5]);
            billboard(&mut haze, pos + dir * 140.0 + f.n * 40.0, rng.range(140.0, 240.0), [tint[0], tint[1], tint[2], 0.22]);
        }
    }
    // wall washes near the platform's level: floods on the ring beams graze down the
    // wall face, so the panels and pipes read as relief in long pools of light
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        for _ in 0..2 {
            let u = rng.range(-w_face * 0.4, w_face * 0.4);
            let y = Z_OFF + MODULE_H * rng.int(1, 4) as f32 - 6.0 * S;
            let pos = f.p(u, y, 3.0 * S + 30.0);
            let dir = (-Vec3::Y + f.n * -0.22).normalize();
            let red = rng.chance(0.08);
            let mech = if red { Vec3::new(1.0, 0.12, 0.04) } else { sodium };
            commands.spawn((
                ShaftPart,
                far(),
                PitLight { mech, clin: cold },
                SpotLight {
                    color: Color::linear_rgb(mech.x, mech.y, mech.z),
                    intensity: 1.0e10,
                    range: 600.0,
                    radius: 3.0,
                    inner_angle: 0.15,
                    outer_angle: 0.6,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(pos).looking_to(dir, f.n),
            ));
            billboard(&mut haze, pos + dir * 60.0, rng.range(80.0, 140.0), [1.0, 1.0, 1.0, 0.25]);
            billboard(&mut halos, pos, 5.0, WHITE);
        }
    }
    // small red lamps: points of warning in the dark
    for _ in 0..30 {
        let f = Facet::new(rng.int(0, SIDES as u32 - 1) as usize, apothem);
        let p = f.p(rng.range(-w_face * 0.45, w_face * 0.45), rng.range(-1.0f32, 1.0).powi(3) * 5.0 * MODULE_H, 25.0);
        billboard(&mut halos, p, rng.range(3.0, 6.0), [1.0, 0.15, 0.08, 1.0]);
    }
    // steam vents near the platform level, spread around so some are always in view
    for j in 0..9 {
        let f = Facet::new((j * 4 + rng.int(0, 2) as usize) % SIDES, apothem);
        let ax = f.ax();
        let (vw, vh) = (rng.range(18.0, 34.0), rng.range(8.0, 14.0));
        let u = rng.range(-w_face * 0.35, w_face * 0.35);
        let y = rng.range(-90.0, 140.0);
        // housing sticks out past the skin, so the vent is there in both states
        let w = 34.0;
        vents.cuboid(f.p(u, y, w / 2.0), ax, [vw / 2.0 + 2.0, vh / 2.0 + 2.0, w / 2.0], DARK);
        vent_lamps.cuboid(f.p(u, y, w + 0.2), ax, [vw / 2.0, vh / 2.0, 0.1], WHITE);
        for k in 0..6 {
            let vs = y - vh / 2.0 + (k as f32 + 0.5) * vh / 6.0;
            vents.cuboid(f.p(u, vs, w + 0.8), ax, [vw / 2.0, 0.2, 0.8], DARK);
        }
        let base = f.p(u, y + vh / 2.0, w + 4.0);
        let (ph, pw) = (rng.range(220.0, 380.0), vw * rng.range(4.0, 6.0));
        plume(&mut smoke, &mut rng, base, ph, vw * 0.8, pw);
        let (ph2, pw2) = (ph * 0.55, pw * 0.5);
        plume(&mut smoke, &mut rng, base + f.n * 6.0, ph2, vw * 0.5, pw2);
        // lit from behind: the light sits between the wall and the plume
        let lp = f.p(u, y + vh * 2.0, 22.0);
        commands.spawn((
            ShaftPart,
            far(),
            PitLight { mech: sodium, clin: cold },
            PointLight { color: Color::linear_rgb(sodium.x, sodium.y, sodium.z), intensity: 3.0e7, range: 280.0, radius: 4.0, shadow_maps_enabled: false, ..default() },
            Transform::from_translation(lp),
        ));
        billboard(&mut haze, f.p(u, y + ph * 0.25, w + 10.0), pw * 0.9, [1.0, 1.0, 1.0, 0.55]);
    }
    for (mb, mat) in [(halos, &mats.halo), (haze, &mats.haze), (beams, &mats.beam), (smoke, &mats.smoke), (vents, &mats.wall), (vent_lamps, &mats.lamp_alt)] {
        commands.spawn((ShaftPart, far(), Mesh3d(meshes.add(mb.build())), MeshMaterial3d(mat.clone())));
    }
}

pub fn ring_speed() -> f32 {
    2.0 * S.sqrt()
}
