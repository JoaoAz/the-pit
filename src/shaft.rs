//! Procedural blockout of the shaft, the floating platform and scale references.
//! Units are metres, Y is up, the platform surface is at y = 0.
//!
//! The shaft is monumental: its structure scales with the diameter, while the
//! human-sized details on the walls (catwalks, doors, ladders, figures) stay human-sized,
//! so they read as tiny and sell the size.

use crate::meshbuild::{MeshBuilder, Rng};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

pub const SIDES: usize = 12;
pub const PLATFORM_R: f32 = 6.0;
const RAIL_H: f32 = 1.1;

/// Shaft sizes the participant can compare (apothem, i.e. centre to wall face).
pub const SIZES: [(f32, &str); 3] = [(100.0, "200 m"), (200.0, "400 m"), (400.0, "800 m")];
pub const DEFAULT_SIZE: usize = 1;

/// Structure scale relative to the original 90 m concept.
pub fn scale(apothem: f32) -> f32 {
    apothem / 45.0
}
pub fn module_h(apothem: f32) -> f32 {
    30.0 * scale(apothem)
}
/// Module origin offset so the platform floats between two ring beams.
fn z_off(apothem: f32) -> f32 {
    -0.53 * module_h(apothem)
}
/// Enough modules that both ends dissolve completely in the fog.
const MODULES_EACH_WAY: i32 = 14;

/// Fog density for a shaft size: the wall stays readable, the ends never show.
pub fn fog_density(apothem: f32, mood: f32) -> f32 {
    (0.45 + (0.32 - 0.45) * mood) / apothem
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

fn tone(rng: &mut Rng) -> [f32; 4] {
    let g = rng.range(0.55, 1.0);
    let warm = rng.range(-0.04, 0.04);
    [g + warm, g, g - warm, 1.0]
}

fn hazard_band(mb: &mut MeshBuilder, f: &Facet, u0: f32, u1: f32, v: f32, w: f32, h: f32, stripe: f32) {
    let n = ((u1 - u0) / stripe).max(1.0) as i32;
    let step = (u1 - u0) / n as f32;
    for k in 0..n {
        let u = u0 + (k as f32 + 0.5) * step;
        let c = if k % 2 == 0 { HAZ_Y } else { HAZ_K };
        mb.cuboid(f.p(u, v, w), f.ax(), [step / 2.0, h, 0.05 * h.max(1.0)], c);
    }
}

/// One module. `var` changes the random dressing; spine pipes are shared so they run continuously.
fn build_module(apothem: f32, var: u64) -> (Mesh, Mesh, Mesh) {
    let s = scale(apothem);
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let h = module_h(apothem);
    let mut solid = MeshBuilder::default();
    let mut lamps = MeshBuilder::default();
    let mut lamps_alt = MeshBuilder::default();
    let mut rng = Rng::new(100 + var);
    let mut spine = Rng::new(7);

    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        // back wall
        solid.cuboid(f.p(0.0, h / 2.0, -0.5 * s), ax, [w_face / 2.0 + s, h / 2.0, 0.5 * s], DARK);
        // panel grid
        let ncol = (rng.int(3, 6) as f32 * s.sqrt()).round().max(3.0) as u32;
        let cw = w_face / ncol as f32;
        for c in 0..ncol {
            let mut v = 0.0;
            while v < h - 0.5 * s {
                let ph = ([2.0f32, 3.0, 4.0, 5.0, 7.5][rng.int(0, 4) as usize] * s).min(h - v);
                let dep = rng.range(0.1, 0.9) * s;
                let u = -w_face / 2.0 + cw * (c as f32 + 0.5);
                let gap = 0.06 * s;
                solid.cuboid(f.p(u, v + ph / 2.0, dep / 2.0), ax, [cw / 2.0 - gap, ph / 2.0 - gap, dep / 2.0], tone(&mut rng));
                v += ph;
            }
        }
        // corner column
        solid.cuboid(f.p(w_face / 2.0, h / 2.0, 2.6 * s), ax, [2.4 * s, h / 2.0, 2.6 * s], DARK);
        for k in 0..7 {
            solid.cuboid(f.p(w_face / 2.0, (2.0 + k as f32 * 4.0) * s, 5.4 * s), ax, [1.5 * s, 0.25 * s, 0.25 * s], GREY);
        }
        // ring beam with lamp fixtures (the module rhythm)
        solid.cuboid(f.p(0.0, h - 1.3 * s, 3.0 * s), ax, [w_face / 2.0 + 1.5 * s, 1.3 * s, 3.0 * s], GREY);
        hazard_band(&mut solid, &f, -w_face / 2.0, w_face / 2.0, h - 0.4 * s, 6.05 * s, 0.4 * s, 0.8 * s);
        let nl = (10.0 * s.sqrt()) as i32;
        for g in 0..nl {
            let u = -w_face / 2.0 + 1.2 * s + (g as f32 + 0.5) * (w_face - 2.4 * s) / nl as f32;
            if rng.chance(0.88) {
                lamps.cuboid(f.p(u, h - 2.65 * s, 5.6 * s), ax, [0.55 * s, 0.06 * s, 0.12 * s], WHITE);
            }
        }
        // spine pipes (same on every module)
        for _ in 0..spine.int(3, 7) {
            let r = [0.2, 0.3, 0.45, 0.7, 1.0, 1.4][spine.int(0, 5) as usize] * s;
            let u = spine.range(-w_face / 2.0 + 3.0 * s, w_face / 2.0 - 3.0 * s);
            let w = 1.2 * s + r + spine.range(0.0, 1.5) * s;
            let c = tone(&mut spine);
            solid.cylinder(f.p(u, 0.0, w), f.p(u, h, w), r, 12, c);
            for v in [5.0, 15.0, 25.0] {
                solid.cuboid(f.p(u, v * s, w / 2.0), ax, [r * 0.6, 0.25 * s, w / 2.0], DARK);
            }
        }
        // machinery housings: the large tier
        for _ in 0..rng.int(1, 3) {
            let (hu, hv, hw) = (rng.range(2.5, 5.5) * s, rng.range(2.5, 7.0) * s, rng.range(1.5, 4.0) * s);
            let u = rng.range(-w_face / 2.0 + hu + 2.5 * s, w_face / 2.0 - hu - 2.5 * s);
            let v = rng.range(hv + s, h - hv - 3.0 * s);
            solid.cuboid(f.p(u, v, 0.8 * s + hw), ax, [hu, hv, hw], tone(&mut rng));
            hazard_band(&mut solid, &f, u - hu, u + hu, v + hv + 0.15 * s, 0.8 * s + 2.0 * hw + 0.05 * s, 0.15 * s, 0.8 * s);
            if rng.chance(0.7) {
                lamps_alt.cuboid(f.p(u + hu * 0.55, v + hv * 0.5, 0.8 * s + 2.0 * hw + 0.1 * s), ax, [hu * 0.25, 0.25 * s, 0.05 * s], WHITE);
            }
        }
        // greebles: medium and small tiers (absolute small sizes stay, so close detail exists)
        let n_greeble = ((55.0 * s) as u32).min(420);
        for _ in 0..n_greeble {
            let u = rng.range(-w_face / 2.0 + 0.5 * s, w_face / 2.0 - 0.5 * s);
            let v = rng.range(0.5 * s, h - 2.5 * s);
            let sz = rng.range(0.4f32.ln(), (2.5 * s).ln()).exp();
            let (hu, hv, hw) = (sz * rng.range(0.4, 1.4), sz * rng.range(0.3, 1.0), sz * rng.range(0.15, 0.6));
            let w0 = rng.range(0.3, 1.0) * s;
            solid.cuboid(f.p(u, v, w0 + hw), ax, [hu, hv, hw], tone(&mut rng));
        }
        // lit window bands and readouts
        for _ in 0..(rng.int(2, 6) as f32 * s.sqrt()) as u32 {
            let u = rng.range(-w_face / 2.0 + 2.0 * s, w_face / 2.0 - 2.0 * s);
            let v = rng.range(s, h - 4.0 * s);
            let (nx, ny) = (rng.int(1, 6), rng.int(1, 3));
            let cell = rng.range(0.2, 0.45) * s;
            solid.cuboid(f.p(u, v, 1.05 * s), ax, [nx as f32 * cell * 0.6 + 0.15 * s, ny as f32 * cell * 0.6 + 0.15 * s, 0.15 * s], DARK);
            for a in 0..nx {
                for b in 0..ny {
                    let uu = u + (a as f32 - (nx - 1) as f32 / 2.0) * cell * 1.15;
                    let vv = v + (b as f32 - (ny - 1) as f32 / 2.0) * cell * 1.15;
                    let target = if rng.chance(0.6) { &mut lamps_alt } else { &mut lamps };
                    target.cuboid(f.p(uu, vv, 1.22 * s), ax, [cell * 0.45, cell * 0.4, 0.03 * s], WHITE);
                }
            }
        }
        // vents
        for _ in 0..rng.int(0, 2) {
            let (vw, vh) = [(5.0, 3.0), (7.0, 4.0), (4.0, 6.0)][rng.int(0, 2) as usize];
            let (vw, vh) = (vw * s, vh * s);
            let u = rng.range(-w_face / 2.0 + vw / 2.0 + 2.0 * s, w_face / 2.0 - vw / 2.0 - 2.0 * s);
            let vc = rng.range(3.0 * s, h - vh - 4.0 * s) + vh / 2.0;
            lamps_alt.cuboid(f.p(u, vc, 0.75 * s), ax, [vw / 2.0, vh / 2.0, 0.05 * s], WHITE);
            let ns = 7;
            for k in 0..ns {
                let vs = vc - vh / 2.0 + (k as f32 + 0.5) * vh / ns as f32;
                solid.cuboid(f.p(u, vs, 1.2 * s), ax, [vw / 2.0, 0.04 * s, 0.25 * s], DARK);
            }
            hazard_band(&mut solid, &f, u - vw / 2.0 - 0.5 * s, u + vw / 2.0 + 0.5 * s, vc - vh / 2.0 - 0.3 * s, 1.25 * s, 0.3 * s, 0.8 * s);
        }
        // human-sized catwalk, doors and ladder: kept at real size on purpose
        if var % 2 == 0 {
            let v = 0.4 * h;
            let wc = 1.2 * s + 1.0; // just in front of the panels
            solid.cuboid(f.p(0.0, v, wc + 1.0), ax, [w_face / 2.0 + 2.0 * s, 0.08, 1.0], DARK);
            hazard_band(&mut solid, &f, -w_face / 2.0, w_face / 2.0, v - 0.2, wc + 2.0, 0.12, 0.8);
            solid.cylinder(f.p(-w_face / 2.0, v + RAIL_H, wc + 1.95), f.p(w_face / 2.0, v + RAIL_H, wc + 1.95), 0.04, 6, HAZ_Y);
            let mut u = -w_face / 2.0;
            while u < w_face / 2.0 {
                solid.cylinder(f.p(u, v, wc + 1.95), f.p(u, v + RAIL_H, wc + 1.95), 0.035, 6, HAZ_Y);
                u += 1.8;
            }
            for _ in 0..3 {
                let du = rng.range(-w_face / 2.0 + 4.0, w_face / 2.0 - 4.0);
                solid.cuboid(f.p(du, v + 1.05, wc - 0.05), ax, [0.6, 1.05, 0.1], DARK);
                solid.cuboid(f.p(du, v + 2.25, wc), ax, [0.8, 0.15, 0.15], HAZ_Y);
                lamps.cuboid(f.p(du + 0.9, v + 1.4, wc + 0.05), ax, [0.08, 0.08, 0.03], WHITE);
            }
            let lu = rng.range(-w_face / 3.0, w_face / 3.0);
            for side in [-0.25f32, 0.25] {
                solid.cylinder(f.p(lu + side, v, wc + 0.3), f.p(lu + side, h + v, wc + 0.3), 0.03, 6, GREY);
            }
            let mut rv = v + 0.3;
            while rv < h + v {
                solid.cylinder(f.p(lu - 0.25, rv, wc + 0.3), f.p(lu + 0.25, rv, wc + 0.3), 0.02, 4, GREY);
                rv += 0.3 * (s.sqrt()); // rungs merge into a line from afar anyway
            }
        }
    }
    (solid.build(), lamps.build(), lamps_alt.build())
}

/// The platform: a plain floating disc, nothing on it.
fn build_platform() -> Mesh {
    let mut s = MeshBuilder::default();
    s.cylinder(Vec3::new(0.0, -0.5, 0.0), Vec3::ZERO, PLATFORM_R, 64, GREY);
    s.cylinder(Vec3::new(0.0, -0.9, 0.0), Vec3::new(0.0, -0.5, 0.0), PLATFORM_R - 0.4, 64, DARK);
    s.build()
}

fn build_ring(apothem: f32) -> (Mesh, Mesh) {
    let s = scale(apothem);
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let depth = 15.0 * s;
    let mut m = MeshBuilder::default();
    let mut l = MeshBuilder::default();
    let mut rng = Rng::new(77);
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        m.cuboid(f.p(0.0, 0.0, 4.5 * s + depth / 2.0), ax, [w_face / 2.0 - s, 3.0 * s, depth / 2.0], GREY);
        hazard_band(&mut m, &f, -w_face * 0.3, w_face * 0.3, -3.2 * s, 4.5 * s + depth + 0.06 * s, 0.4 * s, 0.8 * s);
        for _ in 0..40 {
            let u = rng.range(-w_face * 0.35, w_face * 0.35);
            let sz = rng.range(0.3, 1.6) * s;
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            m.cuboid(f.p(u, side * (3.0 * s + sz * 0.3), rng.range(5.0 * s, 4.5 * s + depth)), ax, [sz, sz * 0.3, sz * 0.8], tone(&mut rng));
        }
        for k in 0..6 {
            let u = -w_face * 0.3 + k as f32 * w_face * 0.12;
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

/// Spawns everything that depends on the shaft size. Called again when the size changes.
pub fn spawn_shaft(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Mats, apothem: f32) {
    const VARIANTS: u64 = 4;
    let s = scale(apothem);
    let h = module_h(apothem);
    let zo = z_off(apothem);
    let mods: Vec<(Handle<Mesh>, Handle<Mesh>, Handle<Mesh>)> = (0..VARIANTS)
        .map(|v| {
            let (a, b, c) = build_module(apothem, v);
            (meshes.add(a), meshes.add(b), meshes.add(c))
        })
        .collect();
    let mut pick = Rng::new(3);
    for k in -MODULES_EACH_WAY..=MODULES_EACH_WAY {
        let v = if k == 0 { 0 } else { (pick.next_u64() % VARIANTS) as usize };
        let t = Transform::from_xyz(0.0, k as f32 * h + zo, 0.0);
        let (a, b, c) = &mods[v];
        commands.spawn((ShaftPart, Mesh3d(a.clone()), MeshMaterial3d(mats.wall.clone()), t));
        commands.spawn((ShaftPart, Mesh3d(b.clone()), MeshMaterial3d(mats.lamp.clone()), t));
        commands.spawn((ShaftPart, Mesh3d(c.clone()), MeshMaterial3d(mats.lamp_alt.clone()), t));
    }

    commands.spawn((ShaftPart, Mesh3d(meshes.add(build_platform())), MeshMaterial3d(mats.wall.clone())));

    // 1.8 m figures on the wall catwalk of the module the platform floats in (k = 0)
    let fig = figure(meshes);
    let catwalk_y = zo + 0.4 * h + 0.08;
    let wc = 1.2 * s + 1.0;
    for i in [0usize, 3, 7] {
        let p = Facet::new(i, apothem).p(1.5, catwalk_y, wc + 1.0);
        commands.spawn((ShaftPart, Mesh3d(fig.clone()), MeshMaterial3d(mats.figure.clone()), Transform::from_translation(p).looking_at(Vec3::new(0.0, p.y, 0.0), Vec3::Y)));
    }

    // the moving ring used to test vection comfort
    let (r, rl) = build_ring(apothem);
    let (low, high) = (zo + h + 6.0 * s, zo + 4.0 * h);
    commands
        .spawn((ShaftPart, MovingRing { low, high }, Transform::from_xyz(0.0, high, 0.0), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(meshes.add(r)), MeshMaterial3d(mats.wall.clone())));
            c.spawn((Mesh3d(meshes.add(rl)), MeshMaterial3d(mats.lamp.clone())));
        });

    // a few real lights on the nearest ring beams; everything else is emissive only
    for k in [0i32, 1] {
        for i in (0..SIDES).step_by(2) {
            let f = Facet::new(i + k as usize, apothem);
            let y = zo + k as f32 * h - 3.0 * s;
            commands.spawn((
                ShaftPart,
                PointLight { intensity: 2.0e7 * s * s, range: apothem * 2.5, radius: s, shadow_maps_enabled: false, ..default() },
                Transform::from_translation(f.p(0.0, y, 7.0 * s)),
            ));
        }
    }
}
