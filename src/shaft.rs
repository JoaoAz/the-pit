//! Procedural blockout of the shaft, platform, bridge and scale references.
//! Units are metres, Y is up, the platform walking surface is at y = 0.

use crate::meshbuild::{MeshBuilder, Rng};
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

pub const SIDES: usize = 12;
pub const MODULE_H: f32 = 30.0;
pub const MODULES_EACH_WAY: i32 = 20;
/// Module origin offset so the platform sits mid-module (ring beams at +14 and -16).
pub const Z_OFF: f32 = -16.0;
pub const PLATFORM_R: f32 = 6.0;
pub const RAIL_H: f32 = 1.1;

/// Shaft sizes the participant can compare (apothem, i.e. centre to wall face).
pub const SIZES: [(f32, &str); 3] = [(30.0, "60 m"), (45.0, "90 m"), (60.0, "120 m")];

#[derive(Component)]
pub struct ShaftPart;

#[derive(Component)]
pub struct MovingRing;

pub struct Mats {
    pub wall: Handle<StandardMaterial>,
    pub lamp: Handle<StandardMaterial>,
    pub lamp_alt: Handle<StandardMaterial>,
    pub fixed: Handle<StandardMaterial>,
    pub figure: Handle<StandardMaterial>,
    pub cap_top: Handle<StandardMaterial>,
    pub cap_bottom: Handle<StandardMaterial>,
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

fn hazard_band(mb: &mut MeshBuilder, f: &Facet, u0: f32, u1: f32, v: f32, w: f32, h: f32) {
    let n = ((u1 - u0) / 0.8).max(1.0) as i32;
    let step = (u1 - u0) / n as f32;
    for k in 0..n {
        let u = u0 + (k as f32 + 0.5) * step;
        let c = if k % 2 == 0 { HAZ_Y } else { HAZ_K };
        mb.cuboid(f.p(u, v, w), f.ax(), [step / 2.0, h, 0.05], c);
    }
}

/// One 30 m module. `var` changes the random dressing; pipes are shared so they run continuously.
fn build_module(apothem: f32, var: u64) -> (Mesh, Mesh, Mesh) {
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let mut solid = MeshBuilder::default();
    let mut lamps = MeshBuilder::default();
    let mut lamps_alt = MeshBuilder::default();
    let mut rng = Rng::new(100 + var);
    let mut spine = Rng::new(7);
    let h = MODULE_H;

    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        // back wall
        solid.cuboid(f.p(0.0, h / 2.0, -0.5), ax, [w_face / 2.0 + 1.0, h / 2.0, 0.5], DARK);
        // panel grid
        let ncol = rng.int(3, 6);
        let cw = w_face / ncol as f32;
        for c in 0..ncol {
            let mut v = 0.0;
            while v < h - 0.5 {
                let ph = [2.0f32, 3.0, 4.0, 5.0, 7.5][rng.int(0, 4) as usize].min(h - v);
                let dep = rng.range(0.1, 0.9);
                let u = -w_face / 2.0 + cw * (c as f32 + 0.5);
                solid.cuboid(f.p(u, v + ph / 2.0, dep / 2.0), ax, [cw / 2.0 - 0.06, ph / 2.0 - 0.06, dep / 2.0], tone(&mut rng));
                v += ph;
            }
        }
        // corner column
        solid.cuboid(f.p(w_face / 2.0, h / 2.0, 2.6), ax, [2.4, h / 2.0, 2.6], DARK);
        for k in 0..7 {
            solid.cuboid(f.p(w_face / 2.0, 2.0 + k as f32 * 4.0, 5.4), ax, [1.5, 0.25, 0.25], GREY);
        }
        // ring beam with lamp fixtures (the 30 m rhythm)
        solid.cuboid(f.p(0.0, h - 1.3, 3.0), ax, [w_face / 2.0 + 1.5, 1.3, 3.0], GREY);
        hazard_band(&mut solid, &f, -w_face / 2.0, w_face / 2.0, h - 0.4, 6.05, 0.4);
        for g in 0..10 {
            let u = -w_face / 2.0 + 1.2 + (g as f32 + 0.5) * (w_face - 2.4) / 10.0;
            if rng.chance(0.88) {
                lamps.cuboid(f.p(u, h - 2.65, 5.6), ax, [0.55, 0.06, 0.12], WHITE);
            }
        }
        // spine pipes (same on every module)
        for _ in 0..spine.int(2, 5) {
            let r = [0.2, 0.3, 0.45, 0.7, 1.0, 1.4][spine.int(0, 5) as usize];
            let u = spine.range(-w_face / 2.0 + 2.0, w_face / 2.0 - 2.0);
            let w = 1.2 + r + spine.range(0.0, 1.5);
            let c = tone(&mut spine);
            solid.cylinder(f.p(u, 0.0, w), f.p(u, h, w), r, 12, c);
            for v in [5.0, 15.0, 25.0] {
                solid.cuboid(f.p(u, v, w / 2.0), ax, [r * 0.6, 0.25, w / 2.0], DARK);
            }
        }
        // machinery housings: the "large" tier of the scale hierarchy
        for _ in 0..rng.int(0, 2) {
            let (hu, hv, hw) = (rng.range(2.5, 5.5), rng.range(2.5, 7.0), rng.range(1.5, 4.0));
            let u = rng.range(-w_face / 2.0 + hu + 2.5, w_face / 2.0 - hu - 2.5);
            let v = rng.range(hv + 1.0, h - hv - 3.0);
            solid.cuboid(f.p(u, v, 0.8 + hw), ax, [hu, hv, hw], tone(&mut rng));
            hazard_band(&mut solid, &f, u - hu, u + hu, v + hv + 0.15, 0.8 + 2.0 * hw + 0.05, 0.15);
            if rng.chance(0.7) {
                lamps_alt.cuboid(f.p(u + hu * 0.55, v + hv * 0.5, 0.8 + 2.0 * hw + 0.1), ax, [hu * 0.25, 0.25, 0.05], WHITE);
            }
        }
        // greebles: the "small" tier
        for _ in 0..rng.int(40, 70) {
            let u = rng.range(-w_face / 2.0 + 0.5, w_face / 2.0 - 0.5);
            let v = rng.range(0.5, h - 2.5);
            let s = rng.range(0.15f32.ln(), 2.5f32.ln()).exp();
            let (hu, hv, hw) = (s * rng.range(0.4, 1.4), s * rng.range(0.3, 1.0), s * rng.range(0.15, 0.6));
            let w0 = rng.range(0.3, 1.0);
            solid.cuboid(f.p(u, v, w0 + hw), ax, [hu, hv, hw], tone(&mut rng));
        }
        // lit readout panels
        for _ in 0..rng.int(2, 6) {
            let u = rng.range(-w_face / 2.0 + 2.0, w_face / 2.0 - 2.0);
            let v = rng.range(1.0, h - 4.0);
            let (nx, ny) = (rng.int(1, 5), rng.int(1, 3));
            let cell = rng.range(0.2, 0.45);
            solid.cuboid(f.p(u, v, 1.05), ax, [nx as f32 * cell * 0.6 + 0.15, ny as f32 * cell * 0.6 + 0.15, 0.15], DARK);
            for a in 0..nx {
                for b in 0..ny {
                    let uu = u + (a as f32 - (nx - 1) as f32 / 2.0) * cell * 1.15;
                    let vv = v + (b as f32 - (ny - 1) as f32 / 2.0) * cell * 1.15;
                    let target = if rng.chance(0.6) { &mut lamps_alt } else { &mut lamps };
                    target.cuboid(f.p(uu, vv, 1.22), ax, [cell * 0.45, cell * 0.4, 0.03], WHITE);
                }
            }
        }
        // vents
        if rng.chance(0.5) {
            let (vw, vh) = [(5.0, 3.0), (7.0, 4.0), (4.0, 6.0)][rng.int(0, 2) as usize];
            let u = rng.range(-w_face / 2.0 + vw / 2.0 + 2.0, w_face / 2.0 - vw / 2.0 - 2.0);
            let vc = rng.range(3.0, h - vh - 4.0) + vh / 2.0;
            lamps_alt.cuboid(f.p(u, vc, 0.75), ax, [vw / 2.0, vh / 2.0, 0.05], WHITE);
            let ns = (vh / 0.45) as i32;
            for s in 0..ns {
                let vs = vc - vh / 2.0 + (s as f32 + 0.5) * vh / ns as f32;
                solid.cuboid(f.p(u, vs, 1.2), ax, [vw / 2.0, 0.04, 0.25], DARK);
            }
            hazard_band(&mut solid, &f, u - vw / 2.0 - 0.5, u + vw / 2.0 + 0.5, vc - vh / 2.0 - 0.3, 1.25, 0.3);
        }
        // catwalk with railing, door and ladder on every other module (human-scale cues)
        if var % 2 == 0 {
            let v = 12.0;
            solid.cuboid(f.p(0.0, v, 4.0), ax, [w_face / 2.0 + 0.9, 0.08, 1.8], DARK);
            hazard_band(&mut solid, &f, -w_face / 2.0 - 0.9, w_face / 2.0 + 0.9, v - 0.25, 5.8, 0.18);
            solid.cylinder(f.p(-w_face / 2.0 - 0.9, v + RAIL_H, 5.7), f.p(w_face / 2.0 + 0.9, v + RAIL_H, 5.7), 0.04, 6, HAZ_Y);
            let mut u = -w_face / 2.0;
            while u < w_face / 2.0 {
                solid.cylinder(f.p(u, v, 5.7), f.p(u, v + RAIL_H, 5.7), 0.035, 6, HAZ_Y);
                u += 1.8;
            }
            // 2.1 m door
            let du = rng.range(-w_face / 2.0 + 4.0, w_face / 2.0 - 4.0);
            solid.cuboid(f.p(du, v + 1.05, 1.0), ax, [0.6, 1.05, 0.1], DARK);
            solid.cuboid(f.p(du, v + 2.25, 1.05), ax, [0.8, 0.15, 0.15], HAZ_Y);
            lamps.cuboid(f.p(du + 0.9, v + 1.4, 1.1), ax, [0.08, 0.08, 0.03], WHITE);
            // ladder to the next level
            let lu = du + 3.0;
            for s in [-0.25f32, 0.25] {
                solid.cylinder(f.p(lu + s, v, 1.3), f.p(lu + s, h - 2.6, 1.3), 0.03, 6, GREY);
            }
            let mut rv = v + 0.3;
            while rv < h - 2.6 {
                solid.cylinder(f.p(lu - 0.25, rv, 1.3), f.p(lu + 0.25, rv, 1.3), 0.02, 4, GREY);
                rv += 0.3;
            }
        }
    }
    (solid.build(), lamps.build(), lamps_alt.build())
}

fn build_platform(apothem: f32) -> (Mesh, Mesh) {
    let mut s = MeshBuilder::default();
    let mut l = MeshBuilder::default();
    let id = [Vec3::X, Vec3::Y, Vec3::Z];
    let pr = PLATFORM_R;
    s.cylinder(Vec3::new(0.0, -0.7, 0.0), Vec3::new(0.0, 0.0, 0.0), pr, 48, GREY);
    s.cylinder(Vec3::new(0.0, -0.75, 0.0), Vec3::new(0.0, -0.66, 0.0), pr + 0.25, 48, HAZ_Y);
    s.cylinder(Vec3::new(0.0, -4.0, 0.0), Vec3::new(0.0, -0.7, 0.0), 2.2, 24, DARK);
    s.cylinder(Vec3::new(0.0, -7.0, 0.0), Vec3::new(0.0, -4.0, 0.0), 1.0, 16, DARK);
    // deck seams so the floor reads at close range
    for k in 0..8 {
        let a = k as f32 * PI / 8.0;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        s.cuboid(Vec3::new(0.0, 0.003, 0.0), [d, Vec3::Y, d.cross(Vec3::Y)], [pr - 0.1, 0.003, 0.012], DARK);
    }
    // railing, open towards the bridge (-Z)
    let gap = |th: f32| {
        let d = (th - (-PI / 2.0)).rem_euclid(TAU);
        d < 0.2 || d > TAU - 0.2
    };
    let posts = 28;
    for k in 0..posts {
        let th = TAU * k as f32 / posts as f32;
        if gap(th) {
            continue;
        }
        let p = Vec3::new(th.cos(), 0.0, th.sin()) * (pr - 0.2);
        s.cylinder(p, p + Vec3::Y * RAIL_H, 0.035, 8, HAZ_Y);
    }
    for (hh, rr) in [(RAIL_H, 0.045), (0.6, 0.03)] {
        let seg = 72;
        for k in 0..seg {
            let (a0, a1) = (TAU * k as f32 / seg as f32, TAU * (k + 1) as f32 / seg as f32);
            if gap((a0 + a1) / 2.0) {
                continue;
            }
            let p0 = Vec3::new(a0.cos(), 0.0, a0.sin()) * (pr - 0.2) + Vec3::Y * hh;
            let p1 = Vec3::new(a1.cos(), 0.0, a1.sin()) * (pr - 0.2) + Vec3::Y * hh;
            s.cylinder(p0, p1, rr, 8, HAZ_Y);
        }
    }
    // console (1.1 m desk)
    s.cuboid(Vec3::new(0.0, 0.55, 1.8), id, [1.4, 0.55, 0.5], GREY);
    for q in 0..5 {
        l.cuboid(Vec3::new(-1.0 + q as f32 * 0.5, 1.12, 1.7), id, [0.18, 0.02, 0.1], WHITE);
    }
    // 1 m crate, a familiar object
    s.cuboid(Vec3::new(-3.2, 0.5, -1.5), id, [0.5, 0.5, 0.5], tone(&mut Rng::new(3)));
    // bridge to the wall along -Z
    let (z0, z1) = (-pr + 0.3, -apothem + 1.0);
    let bw = 1.5;
    let zm = (z0 + z1) / 2.0;
    let half = (z1 - z0).abs() / 2.0;
    s.cuboid(Vec3::new(0.0, -0.15, zm), id, [bw, 0.15, half], GREY);
    for sx in [-1.0f32, 1.0] {
        s.cylinder(Vec3::new(sx * bw, RAIL_H, z0), Vec3::new(sx * bw, RAIL_H, z1), 0.045, 8, HAZ_Y);
        s.cylinder(Vec3::new(sx * bw, -2.4, z0), Vec3::new(sx * bw, -2.4, z1), 0.2, 8, DARK);
        let nseg = ((z0 - z1).abs() / 2.5) as i32;
        for k in 0..=nseg {
            let z = z0 + (z1 - z0) * k as f32 / nseg as f32;
            s.cylinder(Vec3::new(sx * bw, -2.4, z), Vec3::new(sx * bw, RAIL_H, z), 0.05, 6, HAZ_Y);
            if k < nseg {
                let zn = z0 + (z1 - z0) * (k + 1) as f32 / nseg as f32;
                s.cylinder(Vec3::new(sx * bw, -2.4, z), Vec3::new(sx * bw, -0.2, zn), 0.06, 6, DARK);
            }
        }
        for k in 0..8 {
            let z = z0 + (z1 - z0) * (k as f32 + 0.5) / 8.0;
            l.cuboid(Vec3::new(sx * (bw + 0.05), -0.45, z), id, [0.05, 0.05, 0.15], WHITE);
        }
    }
    // underside lights so the platform reads as an object from far away
    for k in 0..12 {
        let a = k as f32 * PI / 6.0;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        l.cuboid(d * 5.6 + Vec3::Y * -0.9, [d, Vec3::Y, d.cross(Vec3::Y)], [0.12, 0.08, 0.3], WHITE);
    }
    (s.build(), l.build())
}

fn build_ring(apothem: f32) -> (Mesh, Mesh) {
    let w_face = 2.0 * apothem * (PI / SIDES as f32).tan();
    let depth = (apothem * 0.33).min(15.0);
    let mut s = MeshBuilder::default();
    let mut l = MeshBuilder::default();
    let mut rng = Rng::new(77);
    for i in 0..SIDES {
        let f = Facet::new(i, apothem);
        let ax = f.ax();
        s.cuboid(f.p(0.0, 0.0, 4.5 + depth / 2.0), ax, [w_face / 2.0 - 1.0, 3.0, depth / 2.0], GREY);
        hazard_band(&mut s, &f, -w_face * 0.3, w_face * 0.3, -3.2, 4.5 + depth + 0.06, 0.4);
        for _ in 0..25 {
            let u = rng.range(-w_face * 0.35, w_face * 0.35);
            let sz = rng.range(0.3, 1.6);
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            s.cuboid(f.p(u, side * (3.0 + sz * 0.3), rng.range(5.0, 4.5 + depth)), ax, [sz, sz * 0.3, sz * 0.8], tone(&mut rng));
        }
        for k in 0..6 {
            let u = -w_face * 0.3 + k as f32 * w_face * 0.12;
            l.cuboid(f.p(u, -3.05, 4.5 + depth * 0.7), ax, [0.5, 0.05, 0.5], WHITE);
        }
        // guide rail in the wall corner
        s.cuboid(f.p(w_face / 2.0 - 1.5, 0.0, 3.4), ax, [0.6, 3.5, 0.4], DARK);
    }
    (s.build(), l.build())
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
pub fn spawn_shaft(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    mats: &Mats,
    apothem: f32,
) {
    const VARIANTS: u64 = 4;
    let mods: Vec<(Handle<Mesh>, Handle<Mesh>, Handle<Mesh>)> = (0..VARIANTS)
        .map(|v| {
            let (a, b, c) = build_module(apothem, v);
            (meshes.add(a), meshes.add(b), meshes.add(c))
        })
        .collect();
    let mut pick = Rng::new(3);
    for k in -MODULES_EACH_WAY..=MODULES_EACH_WAY {
        let v = if k == -1 { 0 } else { (pick.next_u64() % VARIANTS) as usize };
        let t = Transform::from_xyz(0.0, k as f32 * MODULE_H + Z_OFF, 0.0);
        let (a, b, c) = &mods[v];
        commands.spawn((ShaftPart, Mesh3d(a.clone()), MeshMaterial3d(mats.wall.clone()), t));
        commands.spawn((ShaftPart, Mesh3d(b.clone()), MeshMaterial3d(mats.lamp.clone()), t));
        commands.spawn((ShaftPart, Mesh3d(c.clone()), MeshMaterial3d(mats.lamp_alt.clone()), t));
    }
    // glowing ends: the far "infinite" stays a colour, never a floor or ceiling
    let span = MODULES_EACH_WAY as f32 * MODULE_H;
    let cap = meshes.add(Cylinder::new(apothem + 6.0, 1.0));
    commands.spawn((ShaftPart, Mesh3d(cap.clone()), MeshMaterial3d(mats.cap_top.clone()), Transform::from_xyz(0.0, span + MODULE_H, 0.0)));
    commands.spawn((ShaftPart, Mesh3d(cap), MeshMaterial3d(mats.cap_bottom.clone()), Transform::from_xyz(0.0, -span, 0.0)));

    let (p, pl) = build_platform(apothem);
    commands.spawn((ShaftPart, Mesh3d(meshes.add(p)), MeshMaterial3d(mats.wall.clone())));
    commands.spawn((ShaftPart, Mesh3d(meshes.add(pl)), MeshMaterial3d(mats.fixed.clone())));

    // scale references: 1.8 m figures at known distances
    let fig = figure(meshes);
    let catwalk_y = -MODULE_H + Z_OFF + 12.0 + 0.08; // module k = -1 has a catwalk
    let refs = [
        Vec3::new(2.5, 0.0, -1.0),                     // on the platform, ~3 m
        Vec3::new(0.6, 0.0, -apothem + 2.0),           // end of the bridge
        Vec3::new(-2.0, catwalk_y, apothem - 4.5),     // opposite catwalk, below
        Facet::new(1, apothem).p(0.0, Z_OFF + MODULE_H, 3.0),  // on top of the ring beam above
    ];
    for p in refs {
        commands.spawn((ShaftPart, Mesh3d(fig.clone()), MeshMaterial3d(mats.figure.clone()), Transform::from_translation(p).looking_at(Vec3::new(0.0, p.y, 0.0), Vec3::Y)));
    }

    // the moving ring used to test vection comfort
    let (r, rl) = build_ring(apothem);
    commands
        .spawn((ShaftPart, MovingRing, Transform::from_xyz(0.0, 120.0, 0.0), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(meshes.add(r)), MeshMaterial3d(mats.wall.clone())));
            c.spawn((Mesh3d(meshes.add(rl)), MeshMaterial3d(mats.lamp.clone())));
        });

    // a few real lights near the platform; everything far is emissive only
    for (i, y) in [(0usize, 14.0f32), (4, -16.0), (8, 14.0), (2, -16.0), (6, 14.0), (10, -16.0)] {
        let f = Facet::new(i, apothem);
        commands.spawn((
            ShaftPart,
            PointLight { intensity: 2.0e7, range: apothem * 2.5, radius: 1.0, shadow_maps_enabled: false, ..default() },
            Transform::from_translation(f.p(0.0, y - 2.0, 7.0)),
        ));
    }
    commands.spawn((
        ShaftPart,
        PointLight { intensity: 6.0e5, range: 25.0, radius: 0.5, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(0.0, 9.0, 0.0),
    ));
}
