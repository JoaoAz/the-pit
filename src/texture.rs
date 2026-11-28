//! Procedural textures (placeholders until the Fab textures arrive).
//!
//! Wall: a tileable 40 m x 40 m patch of panels, seams and small lit windows at
//! real-world size, with matching roughness/metal and normal maps so the surface is
//! matte and uneven instead of one uniform sheen. Because this detail never scales
//! with the shaft, a bigger shaft shows thousands more windows, which reads as "huge".
//! Skin: the off-white clinical panels, with smudges and grooves.
//! Sprites: a soft glow for lamp halos and a smoke plume.

use crate::meshbuild::Rng;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// World size of one texture tile, in metres. Module height (120 m) is a multiple of it.
pub const TILE_M: f32 = 40.0;
const RES: usize = 1024;
const PX_PER_M: f32 = RES as f32 / TILE_M;

pub struct TexSet {
    pub albedo: Image,
    pub emissive: Option<Image>,
    /// glTF convention: G = roughness, B = metallic
    pub metal_rough: Image,
    pub normal: Image,
}

struct Canvas {
    n: usize,
    albedo: Vec<[f32; 3]>,
    emit: Vec<[f32; 3]>,
    height: Vec<f32>,
    rough: Vec<f32>,
    metal: Vec<f32>,
}

impl Canvas {
    fn new(n: usize, albedo: f32, rough: f32) -> Self {
        Canvas {
            n,
            albedo: vec![[albedo; 3]; n * n],
            emit: vec![[0.0; 3]; n * n],
            height: vec![0.0; n * n],
            rough: vec![rough; n * n],
            metal: vec![0.0; n * n],
        }
    }
    fn each(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, mut f: impl FnMut(&mut Canvas, usize)) {
        for y in y0..y1.min(self.n) {
            for x in x0..x1.min(self.n) {
                f(self, y * self.n + x);
            }
        }
    }
    fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, f: impl Fn(&mut [f32; 3])) {
        self.each(x0, y0, x1, y1, |c, i| f(&mut c.albedo[i]));
    }
    fn set_h(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, h: f32) {
        self.each(x0, y0, x1, y1, |c, i| c.height[i] = h);
    }
    fn light(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, c: [f32; 3]) {
        self.each(x0, y0, x1, y1, |cv, i| {
            cv.emit[i] = c;
            cv.albedo[i] = [c[0] * 0.8, c[1] * 0.8, c[2] * 0.8];
            cv.height[i] = -0.6;
            cv.rough[i] = 0.3;
        });
    }
}

/// Tileable value noise in [0, 1] with `cells` cells across the tile.
fn vnoise(x: f32, y: f32, cells: u32, seed: u64) -> f32 {
    vnoise2(x, y, cells, cells, seed)
}

/// Anisotropic version: `cx` cells across, `cy` cells down.
fn vnoise2(x: f32, y: f32, cx: u32, cy: u32, seed: u64) -> f32 {
    let h = |ix: u32, iy: u32| {
        let mut r = Rng::new(seed ^ ((ix % cx) as u64 * 73_856_093) ^ ((iy % cy) as u64 * 19_349_663));
        r.f()
    };
    let (fx, fy) = (x * cx as f32, y * cy as f32);
    let (ix, iy) = (fx.floor() as u32, fy.floor() as u32);
    let (tx, ty) = (fx - fx.floor(), fy - fy.floor());
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = h(ix, iy) + (h(ix + 1, iy) - h(ix, iy)) * sx;
    let b = h(ix, iy + 1) + (h(ix + 1, iy + 1) - h(ix, iy + 1)) * sx;
    a + (b - a) * sy
}

fn fbm(x: f32, y: f32, base: u32, oct: u32, seed: u64) -> f32 {
    let (mut s, mut amp, mut tot) = (0.0, 1.0, 0.0);
    for o in 0..oct {
        s += vnoise(x, y, base << o, seed + o as u64) * amp;
        tot += amp;
        amp *= 0.5;
    }
    s / tot
}

/// Recursive kd split of the tile into panels between ~1.5 m and ~12 m.
fn split(c: &mut Canvas, rng: &mut Rng, x0: usize, y0: usize, x1: usize, y1: usize, depth: u32) {
    let (w, h) = ((x1 - x0) as f32 / PX_PER_M, (y1 - y0) as f32 / PX_PER_M);
    let stop = (w < 3.0 && h < 3.0) || (depth > 3 && rng.chance(0.25)) || (w < 12.0 && h < 12.0 && rng.chance(0.3));
    if !stop {
        let vertical = if w > h * 1.5 { true } else if h > w * 1.5 { false } else { rng.chance(0.5) };
        if vertical && w >= 3.0 {
            let m = x0 + ((x1 - x0) as f32 * rng.range(0.3, 0.7)) as usize;
            split(c, rng, x0, y0, m, y1, depth + 1);
            split(c, rng, m, y0, x1, y1, depth + 1);
            return;
        } else if h >= 3.0 {
            let m = y0 + ((y1 - y0) as f32 * rng.range(0.3, 0.7)) as usize;
            split(c, rng, x0, y0, x1, m, depth + 1);
            split(c, rng, x0, m, x1, y1, depth + 1);
            return;
        }
    }
    panel(c, rng, x0, y0, x1, y1);
}

fn panel(c: &mut Canvas, rng: &mut Rng, x0: usize, y0: usize, x1: usize, y1: usize) {
    let g = rng.range(0.35, 1.0);
    let rust = rng.chance(0.18);
    let base = if rust { [g * 0.95, g * 0.72, g * 0.52] } else { [g * rng.range(0.97, 1.03), g, g * rng.range(0.95, 1.02)] };
    let rough = rng.range(0.62, 0.98);
    let metal = if rng.chance(0.3) { rng.range(0.3, 0.7) } else { rng.range(0.0, 0.15) };
    let ph = rng.range(0.1, 0.5);
    c.each(x0, y0, x1, y1, |cv, i| {
        cv.albedo[i] = base;
        cv.rough[i] = rough;
        cv.metal[i] = metal;
        cv.height[i] = ph;
    });
    // seams: ~5 cm dark groove, slightly lighter bevel on top
    let s = 2usize;
    for (a, b, cc, d) in [(x0, y0, x1, y0 + s), (x0, y0, x0 + s, y1)] {
        c.each(a, b, cc, d, |cv, i| {
            cv.albedo[i] = [0.08, 0.08, 0.08];
            cv.height[i] = -0.5;
            cv.rough[i] = 1.0;
        });
    }
    c.rect(x0 + s, y0 + s, x1, y0 + s + 1, |p| *p = [p[0] * 1.15, p[1] * 1.15, p[2] * 1.15]);
    let (w, h) = (x1 - x0, y1 - y0);
    match rng.int(0, 11) {
        // window band: rows of small lit openings, 0.5-0.9 m, most of them off
        0..=1 if w > 30 && h > 20 => {
            let cell = (rng.range(0.9, 1.6) * PX_PER_M) as usize;
            let win = (cell as f32 * rng.range(0.4, 0.65)) as usize;
            let warm = rng.chance(0.75);
            let lit = rng.range(0.08, 0.35);
            let mut y = y0 + cell / 2;
            while y + win < y1 - 4 {
                let mut x = x0 + cell / 2;
                while x + win < x1 - 4 {
                    if rng.chance(lit) {
                        let k = rng.range(0.4, 1.0);
                        let col = if warm { [1.0 * k, 0.6 * k, 0.28 * k] } else { [0.7 * k, 0.9 * k, 1.0 * k] };
                        c.light(x, y, x + win, y + win * 2 / 3, col);
                    } else {
                        c.rect(x, y, x + win, y + win * 2 / 3, |p| *p = [0.03, 0.03, 0.03]);
                        c.set_h(x, y, x + win, y + win * 2 / 3, -0.6);
                    }
                    x += cell;
                }
                y += cell;
            }
        }
        // louvres
        2..=4 if h > 16 => {
            let mut y = y0 + 6;
            while y + 3 < y1 - 3 {
                c.each(x0 + 4, y, x1 - 4, y + 2, |cv, i| {
                    let p = cv.albedo[i];
                    cv.albedo[i] = [p[0] * 0.3, p[1] * 0.3, p[2] * 0.3];
                    cv.height[i] = -0.4;
                });
                y += 6;
            }
        }
        // sub-plates and hatches
        5 | 6 => {
            for _ in 0..rng.int(1, 4) {
                let (sw, sh) = (rng.range(0.2, 0.6) * w as f32, rng.range(0.2, 0.6) * h as f32);
                let sx = x0 + (rng.f() * (w as f32 - sw)) as usize;
                let sy = y0 + (rng.f() * (h as f32 - sh)) as usize;
                let k = rng.range(0.6, 1.25);
                let r2 = rng.range(0.55, 0.95);
                let hh = ph + rng.range(-0.3, 0.4);
                c.each(sx, sy, sx + sw as usize, sy + sh as usize, |cv, i| {
                    let p = cv.albedo[i];
                    cv.albedo[i] = [p[0] * k, p[1] * k, p[2] * k];
                    cv.rough[i] = r2;
                    cv.height[i] = hh;
                });
                c.rect(sx, sy, sx + sw as usize, sy + 1, |p| *p = [0.12, 0.12, 0.12]);
            }
        }
        // a single indicator light
        7 => {
            let x = x0 + w / 2;
            let y = y0 + 4;
            let col = if rng.chance(0.5) { [1.0, 0.25, 0.05] } else { [1.0, 0.8, 0.5] };
            c.light(x, y, x + 6, y + 3, col);
        }
        _ => {}
    }
}

/// Grime: dark streaks running down and blotchy dirt, darker and rougher.
fn grime(c: &mut Canvas, seed: u64, strength: f32) {
    let n = c.n;
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let blot = fbm(u, v, 4, 5, seed);
            // streaks: noise stretched vertically
            let streak = vnoise2(u, v, 96, 3, seed + 99) * vnoise(u, v, 8, seed + 7);
            let d = ((blot - 0.45) * 2.2).clamp(0.0, 1.0) * 0.55 + streak * 0.5;
            let k = 1.0 - strength * d.clamp(0.0, 1.0);
            let i = y * n + x;
            let a = c.albedo[i];
            c.albedo[i] = [a[0] * k, a[1] * k, a[2] * k];
            c.rough[i] = (c.rough[i] + strength * d * 0.3).min(1.0);
            c.metal[i] *= k;
        }
    }
}

fn to_srgb8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}
fn to_lin8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Builds an RGBA8 image with a full box-filtered mip chain and anisotropic, repeating
/// sampling (essential in VR: without mips fine detail shimmers).
fn image_with_mips(n: usize, px: &[[f32; 4]], srgb: bool) -> Image {
    let mut levels: Vec<Vec<[f32; 4]>> = vec![px.to_vec()];
    let mut size = n;
    while size > 1 {
        let prev = levels.last().unwrap();
        let h = size / 2;
        let mut next = vec![[0.0f32; 4]; h * h];
        for y in 0..h {
            for x in 0..h {
                let mut acc = [0.0; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = prev[(y * 2 + dy) * size + x * 2 + dx];
                    for k in 0..4 {
                        acc[k] += p[k] * 0.25;
                    }
                }
                next[y * h + x] = acc;
            }
        }
        levels.push(next);
        size = h;
    }
    let enc = |p: &[f32; 4]| {
        if srgb {
            [to_srgb8(p[0]), to_srgb8(p[1]), to_srgb8(p[2]), to_lin8(p[3])]
        } else {
            [to_lin8(p[0]), to_lin8(p[1]), to_lin8(p[2]), to_lin8(p[3])]
        }
    };
    let level0: Vec<u8> = px.iter().flat_map(enc).collect();
    let mut img = Image::new(
        Extent3d { width: n as u32, height: n as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        level0,
        if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm },
        RenderAssetUsages::RENDER_WORLD,
    );
    img.data = Some(levels.iter().flat_map(|l| l.iter().flat_map(enc)).collect());
    img.texture_descriptor.mip_level_count = levels.len() as u32;
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..Default::default()
    });
    img
}

/// Tangent-space normal map from the height field (wraps, so it tiles).
fn normal_map(c: &Canvas, strength: f32) -> Vec<[f32; 4]> {
    let n = c.n;
    let h = |x: usize, y: usize| c.height[(y % n) * n + (x % n)];
    let mut out = vec![[0.5, 0.5, 1.0, 1.0]; n * n];
    for y in 0..n {
        for x in 0..n {
            let dx = (h(x + 1, y) - h(x + n - 1, y)) * strength;
            let dy = (h(x, y + 1) - h(x, y + n - 1)) * strength;
            let v = bevy::math::Vec3::new(-dx, dy, 1.0).normalize();
            out[y * n + x] = [v.x * 0.5 + 0.5, v.y * 0.5 + 0.5, v.z * 0.5 + 0.5, 1.0];
        }
    }
    out
}

fn finish(c: &Canvas, normal_strength: f32, with_emit: bool) -> TexSet {
    let rgba = |v: &Vec<[f32; 3]>| v.iter().map(|p| [p[0], p[1], p[2], 1.0]).collect::<Vec<_>>();
    let mr: Vec<[f32; 4]> = c.rough.iter().zip(&c.metal).map(|(r, m)| [1.0, *r, *m, 1.0]).collect();
    TexSet {
        albedo: image_with_mips(c.n, &rgba(&c.albedo), true),
        emissive: with_emit.then(|| image_with_mips(c.n, &rgba(&c.emit), true)),
        metal_rough: image_with_mips(c.n, &mr, false),
        normal: image_with_mips(c.n, &normal_map(c, normal_strength), false),
    }
}

/// Mechanical wall.
pub fn wall_textures() -> TexSet {
    let mut c = Canvas::new(RES, 0.5, 0.9);
    let mut rng = Rng::new(2024);
    split(&mut c, &mut rng, 0, 0, RES, RES, 0);
    grime(&mut c, 11, 0.6);
    finish(&c, 1.5, true)
}

/// Clinical skin: off-white plates, fine grooves every 10 m, smudges, a few stencils.
pub fn skin_textures() -> TexSet {
    let n = 512;
    let ppm = n as f32 / TILE_M;
    let mut c = Canvas::new(n, 0.9, 0.55);
    let mut rng = Rng::new(77);
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let f = fbm(u, v, 3, 5, 5);
            let i = y * n + x;
            let g = 0.86 + 0.1 * f;
            c.albedo[i] = [g, g * 1.005, g * 1.01];
            c.rough[i] = 0.45 + 0.35 * fbm(u, v, 6, 4, 9);
        }
    }
    let step = (10.0 * ppm) as usize;
    for k in 0..(n / step) {
        let p = k * step;
        c.each(p, 0, p + 1, n, |cv, i| {
            cv.albedo[i] = [0.45, 0.46, 0.47];
            cv.height[i] = -0.6;
        });
        c.each(0, p, n, p + 1, |cv, i| {
            cv.albedo[i] = [0.45, 0.46, 0.47];
            cv.height[i] = -0.6;
        });
    }
    for _ in 0..14 {
        let (x, y) = (rng.int(4, n as u32 - 40) as usize, rng.int(4, n as u32 - 20) as usize);
        let (w, h) = (rng.int(6, 30) as usize, rng.int(2, 6) as usize);
        let dark = rng.range(0.25, 0.6);
        c.rect(x, y, x + w, y + h, |p| *p = [dark, dark, dark * 1.02]);
    }
    finish(&c, 0.8, false)
}

/// Soft radial glow for lamp halos (alpha carries the falloff).
pub fn glow_texture() -> Image {
    let n = 128;
    let mut px = vec![[0.0f32; 4]; n * n];
    for y in 0..n {
        for x in 0..n {
            let (u, v) = ((x as f32 + 0.5) / n as f32 * 2.0 - 1.0, (y as f32 + 0.5) / n as f32 * 2.0 - 1.0);
            let r = (u * u + v * v).sqrt();
            let a = ((1.0 - r).max(0.0)).powf(2.5) + 0.6 * (1.0 - r * 4.0).max(0.0);
            px[y * n + x] = [1.0, 1.0, 1.0, a.min(1.0)];
        }
    }
    let mut img = image_with_mips(n, &px, false);
    clamp(&mut img);
    img
}

/// Tileable billowing noise for smoke; the plume shape comes from vertex alpha, so
/// this texture can scroll upwards forever.
pub fn smoke_texture() -> Image {
    let n = 256;
    let mut px = vec![[0.0f32; 4]; n * n];
    for y in 0..n {
        for x in 0..n {
            let (u, v) = (x as f32 / n as f32, y as f32 / n as f32);
            let f = fbm(u, v, 3, 5, 31);
            let a = ((f - 0.28) * 1.9).clamp(0.0, 1.0);
            let g = 0.7 + 0.3 * f;
            px[y * n + x] = [g, g, g, a];
        }
    }
    image_with_mips(n, &px, true)
}

fn clamp(img: &mut Image) {
    if let ImageSampler::Descriptor(d) = &mut img.sampler {
        d.address_mode_u = ImageAddressMode::ClampToEdge;
        d.address_mode_v = ImageAddressMode::ClampToEdge;
    }
}

/// Steel floor grating: bearing bars every 3 cm one way, cross bars every 10 cm,
/// alpha = bar (drawn with alpha to coverage, so the abyss shows through the gaps).
/// One tile covers 0.3 m x 0.3 m.
pub fn grate_texture() -> Image {
    let n = 256;
    let mut px = vec![[0.0f32; 4]; n * n];
    let bear = n / 10; // 3 cm
    let cross = n / 3; // 10 cm
    for y in 0..n {
        for x in 0..n {
            let bar = x % bear < 4;
            let xb = y % cross < 6;
            let g = if bar { 0.42 } else { 0.3 };
            let a = if bar || xb { 1.0 } else { 0.0 };
            px[y * n + x] = [g, g * 0.98, g * 0.95, a];
        }
    }
    image_with_mips(n, &px, true)
}
