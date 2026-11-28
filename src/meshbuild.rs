//! Tiny mesh builder: boxes and cylinders with flat/smooth normals and vertex colours.

use bevy::asset::RenderAssetUsages;
use bevy::math::Vec3;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};

#[derive(Default)]
pub struct MeshBuilder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    uv: Vec<[f32; 2]>,
    tan: Vec<[f32; 4]>,
    idx: Vec<u32>,
    /// Offset added to the planar UVs of everything built next (breaks texture tiling).
    pub uv_off: [f32; 2],
    /// Skip the fake occlusion (lamps, glowing parts).
    pub no_ao: bool,
}

/// Planar projection axes for a face normal (u axis, v axis).
fn planar_axes(n: Vec3) -> (Vec3, Vec3) {
    let up = if n.y.abs() > 0.9 { Vec3::Z } else { Vec3::Y };
    let tu = up.cross(n).normalize_or_zero();
    (tu, n.cross(tu))
}

/// World-scale planar UVs, so texture detail keeps its real size on every surface.
fn planar_uv(p: Vec3, n: Vec3, off: [f32; 2]) -> [f32; 2] {
    let (tu, tv) = planar_axes(n);
    [p.dot(tu) / crate::texture::TILE_M + off[0], -p.dot(tv) / crate::texture::TILE_M + off[1]]
}

/// Fake ambient occlusion: faces looking down are in shadow, and every side face gets
/// darker towards its bottom edge, as if the parts below it blocked the light.
/// It is what gives the hard-edged boxes their volume in the dark.
fn ao_side(y: f32, ymin: f32, ymax: f32, n: Vec3) -> f32 {
    let face = 0.62 + 0.38 * n.y.clamp(-1.0, 1.0);
    let h = if ymax - ymin > 1e-3 { ((y - ymin) / (ymax - ymin)).clamp(0.0, 1.0) } else { 1.0 };
    let grad = if n.y.abs() > 0.7 { 1.0 } else { 0.4 + 0.6 * h.powf(0.7) };
    face * grad
}

fn shade(c: [f32; 4], k: f32) -> [f32; 4] {
    [c[0] * k, c[1] * k, c[2] * k, c[3]]
}

fn planar_tangent(n: Vec3) -> [f32; 4] {
    let (tu, _) = planar_axes(n);
    [tu.x, tu.y, tu.z, 1.0]
}

impl MeshBuilder {
    /// Builder for emissive parts: no fake occlusion.
    pub fn glowing() -> Self {
        Self { no_ao: true, ..Default::default() }
    }

    fn quad(&mut self, p: [Vec3; 4], n: Vec3, c: [f32; 4]) {
        self.quad_ao(p, n, c, [1.0; 4]);
    }

    /// Quad whose vertex colours are darkened by a per-vertex occlusion factor.
    fn quad_ao(&mut self, p: [Vec3; 4], n: Vec3, c: [f32; 4], ao: [f32; 4]) {
        let b = self.pos.len() as u32;
        for (v, k) in p.into_iter().zip(ao) {
            self.pos.push(v.to_array());
            self.nrm.push(n.to_array());
            self.col.push(shade(c, k));
            self.uv.push(planar_uv(v, n, self.uv_off));
            self.tan.push(planar_tangent(n));
        }
        self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    /// Free quad with explicit UVs and per-vertex colours (billboards, sprites).
    pub fn quad_uv(&mut self, p: [Vec3; 4], uv: [[f32; 2]; 4], n: Vec3, c: [[f32; 4]; 4]) {
        let b = self.pos.len() as u32;
        let t = (p[1] - p[0]).normalize_or_zero();
        for i in 0..4 {
            self.pos.push(p[i].to_array());
            self.nrm.push(n.to_array());
            self.col.push(c[i]);
            self.uv.push(uv[i]);
            self.tan.push([t.x, t.y, t.z, 1.0]);
        }
        self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    /// Open truncated cone from `p0` along `dir`, radius r0 -> r1, colour c0 -> c1
    /// (used for fake volumetric light beams with a vertex alpha fade).
    #[allow(clippy::too_many_arguments)]
    pub fn cone(&mut self, p0: Vec3, dir: Vec3, len: f32, r0: f32, r1: f32, sides: u32, c0: [f32; 4], c1: [f32; 4]) {
        let ax = dir.normalize();
        let t = ax.any_orthonormal_vector();
        let bt = ax.cross(t);
        let b = self.pos.len() as u32;
        let p1 = p0 + ax * len;
        for i in 0..=sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let d = t * a.cos() + bt * a.sin();
            for (p, r, c, v) in [(p0, r0, c0, 0.0), (p1, r1, c1, 1.0)] {
                self.pos.push((p + d * r).to_array());
                self.nrm.push(d.to_array());
                self.col.push(c);
                self.uv.push([i as f32 / sides as f32, v]);
                let tg = ax.cross(d);
                self.tan.push([tg.x, tg.y, tg.z, 1.0]);
            }
        }
        for i in 0..sides {
            let k = b + i * 2;
            self.idx.extend_from_slice(&[k, k + 2, k + 3, k, k + 3, k + 1]);
        }
    }

    /// Chain of cylinders through the points (cables, bent pipes).
    pub fn tube(&mut self, pts: &[Vec3], r: f32, sides: u32, col: [f32; 4]) {
        for w in pts.windows(2) {
            self.cylinder(w[0], w[1], r, sides, col);
        }
    }

    /// Oriented box: centre, three orthonormal axes, half extents.
    pub fn cuboid(&mut self, c: Vec3, ax: [Vec3; 3], h: [f32; 3], col: [f32; 4]) {
        let [a, b, d] = [ax[0] * h[0], ax[1] * h[1], ax[2] * h[2]];
        let faces = [
            (a, b, d),   // +a
            (-a, d, b),  // -a
            (b, d, a),   // +b
            (-b, a, d),  // -b
            (d, a, b),   // +d
            (-d, b, a),  // -d
        ];
        for (n, u, v) in faces {
            let ctr = c + n;
            let nn = n.normalize_or_zero();
            // ensure counter-clockwise winding seen from outside
            let (u, v) = if u.cross(v).dot(nn) > 0.0 { (u, v) } else { (v, u) };
            let p = [ctr - u - v, ctr + u - v, ctr + u + v, ctr - u + v];
            if self.no_ao {
                self.quad(p, nn, col);
            } else {
                let ymin = c.y - (a.y.abs() + b.y.abs() + d.y.abs());
                let ymax = c.y + (a.y.abs() + b.y.abs() + d.y.abs());
                let ao = p.map(|q| ao_side(q.y, ymin, ymax, nn));
                self.quad_ao(p, nn, col, ao);
            }
        }
    }

    /// Cylinder between two points.
    pub fn cylinder(&mut self, p0: Vec3, p1: Vec3, r: f32, sides: u32, col: [f32; 4]) {
        let ax = (p1 - p0).normalize();
        let t = ax.any_orthonormal_vector();
        let bt = ax.cross(t);
        let b = self.pos.len() as u32;
        let len = (p1 - p0).length();
        let tm = crate::texture::TILE_M;
        for i in 0..=sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let dir = t * a.cos() + bt * a.sin();
            for (p, along) in [(p0, 0.0), (p1, len)] {
                self.pos.push((p + dir * r).to_array());
                self.nrm.push(dir.to_array());
                let k = if self.no_ao { 1.0 } else { ao_side(p.y, p0.y.min(p1.y), p0.y.max(p1.y), dir) };
                self.col.push(shade(col, k));
                self.uv.push([a * r / tm + self.uv_off[0], (p0.y + along) / tm + self.uv_off[1]]);
                let tg = ax.cross(dir);
                self.tan.push([tg.x, tg.y, tg.z, 1.0]);
            }
        }
        for i in 0..sides {
            let k = b + i * 2;
            // k: p0_i, k+1: p1_i, k+2: p0_{i+1}, k+3: p1_{i+1}
            self.idx.extend_from_slice(&[k, k + 2, k + 3, k, k + 3, k + 1]);
        }
        // caps
        for (p, n) in [(p0, -ax), (p1, ax)] {
            let cb = self.pos.len() as u32;
            let col = if self.no_ao { col } else { shade(col, 0.62 + 0.38 * n.y) };
            self.pos.push(p.to_array());
            self.nrm.push(n.to_array());
            self.col.push(col);
            self.uv.push(planar_uv(p, n, self.uv_off));
            self.tan.push(planar_tangent(n));
            for i in 0..=sides {
                let a = std::f32::consts::TAU * i as f32 / sides as f32;
                let dir = t * a.cos() + bt * a.sin();
                let q = p + dir * r;
                self.pos.push(q.to_array());
                self.nrm.push(n.to_array());
                self.col.push(col);
                self.uv.push(planar_uv(q, n, self.uv_off));
                self.tan.push(planar_tangent(n));
            }
            for i in 0..sides {
                let (x, y) = (cb + 1 + i, cb + 2 + i);
                if n.dot(ax) > 0.0 {
                    self.idx.extend_from_slice(&[cb, x, y]);
                } else {
                    self.idx.extend_from_slice(&[cb, y, x]);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, self.tan)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Small deterministic RNG (xorshift64*), so every run builds the same shaft.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn int(&mut self, a: u32, b: u32) -> u32 {
        a + (self.next_u64() % (b - a + 1) as u64) as u32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}
