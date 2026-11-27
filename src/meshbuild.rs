//! Tiny mesh builder: boxes and cylinders with flat/smooth normals and vertex colours.

use bevy::asset::RenderAssetUsages;
use bevy::math::Vec3;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};

#[derive(Default)]
pub struct MeshBuilder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshBuilder {
    fn quad(&mut self, p: [Vec3; 4], n: Vec3, c: [f32; 4]) {
        let b = self.pos.len() as u32;
        for v in p {
            self.pos.push(v.to_array());
            self.nrm.push(n.to_array());
            self.col.push(c);
        }
        self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
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
            self.quad([ctr - u - v, ctr + u - v, ctr + u + v, ctr - u + v], nn, col);
        }
    }

    /// Cylinder between two points.
    pub fn cylinder(&mut self, p0: Vec3, p1: Vec3, r: f32, sides: u32, col: [f32; 4]) {
        let ax = (p1 - p0).normalize();
        let t = ax.any_orthonormal_vector();
        let bt = ax.cross(t);
        let b = self.pos.len() as u32;
        for i in 0..=sides {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let dir = t * a.cos() + bt * a.sin();
            for p in [p0, p1] {
                self.pos.push((p + dir * r).to_array());
                self.nrm.push(dir.to_array());
                self.col.push(col);
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
            self.pos.push(p.to_array());
            self.nrm.push(n.to_array());
            self.col.push(col);
            for i in 0..=sides {
                let a = std::f32::consts::TAU * i as f32 / sides as f32;
                let dir = t * a.cos() + bt * a.sin();
                self.pos.push((p + dir * r).to_array());
                self.nrm.push(n.to_array());
                self.col.push(col);
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

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
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
