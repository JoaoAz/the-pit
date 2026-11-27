//! Procedural wall texture: a tileable 40 m x 40 m patch of panels, seams and
//! small lit windows at real-world size. Because this detail never scales with the
//! shaft, a bigger shaft shows thousands more windows, which is what reads as "huge".
//! (Placeholder until the Fab textures arrive.)

use crate::meshbuild::Rng;
use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// World size of one texture tile, in metres. Module height (120 m) is a multiple of it.
pub const TILE_M: f32 = 40.0;
const RES: usize = 1024;
const PX_PER_M: f32 = RES as f32 / TILE_M;

struct Canvas {
    albedo: Vec<[f32; 3]>,
    emit: Vec<[f32; 3]>,
}

impl Canvas {
    fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, f: impl Fn(&mut [f32; 3])) {
        for y in y0..y1.min(RES) {
            for x in x0..x1.min(RES) {
                f(&mut self.albedo[y * RES + x]);
            }
        }
    }
    fn light(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, c: [f32; 3]) {
        for y in y0..y1.min(RES) {
            for x in x0..x1.min(RES) {
                self.emit[y * RES + x] = c;
                self.albedo[y * RES + x] = [c[0] * 0.8, c[1] * 0.8, c[2] * 0.8];
            }
        }
    }
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
    let g = rng.range(0.45, 1.0);
    let base = [g * rng.range(0.97, 1.03), g, g * rng.range(0.95, 1.02)];
    c.rect(x0, y0, x1, y1, |p| *p = base);
    // seams: ~5 cm dark border, slightly lighter bevel on top
    let s = 2usize;
    c.rect(x0, y0, x1, y0 + s, |p| *p = [0.12, 0.12, 0.12]);
    c.rect(x0, y0, x0 + s, y1, |p| *p = [0.12, 0.12, 0.12]);
    c.rect(x0 + s, y0 + s, x1, y0 + s + 1, |p| *p = [p[0] * 1.15, p[1] * 1.15, p[2] * 1.15]);
    let (w, h) = (x1 - x0, y1 - y0);
    match rng.int(0, 9) {
        // window band: rows of small lit openings, 0.5-0.9 m, many off
        0..=2 if w > 30 && h > 20 => {
            let cell = (rng.range(0.9, 1.6) * PX_PER_M) as usize;
            let win = (cell as f32 * rng.range(0.4, 0.65)) as usize;
            let warm = rng.chance(0.7);
            let mut y = y0 + cell / 2;
            while y + win < y1 - 4 {
                let mut x = x0 + cell / 2;
                while x + win < x1 - 4 {
                    if rng.chance(0.45) {
                        let k = rng.range(0.5, 1.0);
                        let col = if warm { [1.0 * k, 0.62 * k, 0.3 * k] } else { [0.7 * k, 0.9 * k, 1.0 * k] };
                        c.light(x, y, x + win, y + win * 2 / 3, col);
                    } else {
                        c.rect(x, y, x + win, y + win * 2 / 3, |p| *p = [0.05, 0.05, 0.05]);
                    }
                    x += cell;
                }
                y += cell;
            }
        }
        // louvres
        3 | 4 if h > 16 => {
            let mut y = y0 + 6;
            while y + 3 < y1 - 3 {
                c.rect(x0 + 4, y, x1 - 4, y + 2, |p| *p = [p[0] * 0.35, p[1] * 0.35, p[2] * 0.35]);
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
                c.rect(sx, sy, sx + sw as usize, sy + sh as usize, |p| *p = [p[0] * k, p[1] * k, p[2] * k]);
                c.rect(sx, sy, sx + sw as usize, sy + 1, |p| *p = [0.15, 0.15, 0.15]);
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

fn to_srgb8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Builds an RGBA8 sRGB image with a full box-filtered mip chain and anisotropic,
/// repeating sampling (essential in VR: without mips fine detail shimmers).
fn image_with_mips(px: &[[f32; 3]]) -> Image {
    let mut levels: Vec<Vec<[f32; 3]>> = vec![px.to_vec()];
    let mut size = RES;
    while size > 1 {
        let prev = levels.last().unwrap();
        let n = size / 2;
        let mut next = vec![[0.0f32; 3]; n * n];
        for y in 0..n {
            for x in 0..n {
                let mut acc = [0.0; 3];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let p = prev[(y * 2 + dy) * size + x * 2 + dx];
                    for k in 0..3 {
                        acc[k] += p[k] * 0.25;
                    }
                }
                next[y * n + x] = acc;
            }
        }
        levels.push(next);
        size = n;
    }
    let level0: Vec<u8> = px.iter().flat_map(|p| [to_srgb8(p[0]), to_srgb8(p[1]), to_srgb8(p[2]), 255]).collect();
    let mut img = Image::new(
        Extent3d { width: RES as u32, height: RES as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        level0,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let all: Vec<u8> = levels
        .iter()
        .flat_map(|l| l.iter().flat_map(|p| [to_srgb8(p[0]), to_srgb8(p[1]), to_srgb8(p[2]), 255]))
        .collect();
    img.data = Some(all);
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

/// Returns (albedo, emissive) images.
pub fn wall_textures() -> (Image, Image) {
    let mut c = Canvas { albedo: vec![[0.5; 3]; RES * RES], emit: vec![[0.0; 3]; RES * RES] };
    let mut rng = Rng::new(2024);
    split(&mut c, &mut rng, 0, 0, RES, RES, 0);
    (image_with_mips(&c.albedo), image_with_mips(&c.emit))
}
