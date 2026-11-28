//! Everything far from the participant is drawn as a 2D image.
//!
//! The participant never leaves the 4 m platform, and the nearest wall is hundreds of
//! metres away, so the shaft has no visible parallax: a panorama looks identical in
//! stereo. The whole shaft lives on render layer 1 and is rendered by one camera into
//! the six faces of a cube (one face per frame, round robin). The headset only draws
//! the platform, the hands and six textured quads, so its cost no longer depends on
//! how much detail the shaft has. Moving parts (shutters, ring, smoke, alarm lights)
//! still move: at 90 Hz every face is refreshed 15 times per second.

use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Hdr, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::image::ImageSampler;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Render layer of the far shaft.
pub const FAR: usize = 1;
pub fn far() -> RenderLayers {
    RenderLayers::layer(FAR)
}

/// Where the panorama is captured from: the participant's head on the platform.
pub const EYE: Vec3 = Vec3::new(0.0, 1.6, 0.0);
/// Distance of the panorama quads (anything beyond the platform; the far plane must cover sqrt(3) x this).
const BOX_R: f32 = 4000.0;

/// Face directions and up vectors.
const FACES: [(Vec3, Vec3); 6] = [
    (Vec3::X, Vec3::Y),
    (Vec3::NEG_X, Vec3::Y),
    (Vec3::Z, Vec3::Y),
    (Vec3::NEG_Z, Vec3::Y),
    (Vec3::Y, Vec3::Z),
    (Vec3::NEG_Y, Vec3::NEG_Z),
];

#[derive(Resource)]
pub struct Panorama {
    faces: Vec<Handle<Image>>,
    next: usize,
}

#[derive(Component)]
struct PanoCam;

pub struct PanoramaPlugin {
    pub res: u32,
}

impl Plugin for PanoramaPlugin {
    fn build(&self, app: &mut App) {
        let res = self.res;
        app.add_systems(Startup, move |c: Commands, m: ResMut<Assets<Mesh>>, mt: ResMut<Assets<StandardMaterial>>, i: ResMut<Assets<Image>>| {
            setup(c, m, mt, i, res)
        })
        .add_systems(PostUpdate, next_face.before(TransformSystems::Propagate));
    }
}

/// 1 extra pixel of field of view on every side, so bilinear filtering does not show seams.
fn tan_half(res: u32) -> f32 {
    res as f32 / (res as f32 - 2.0)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    res: u32,
) {
    let faces: Vec<Handle<Image>> = (0..6)
        .map(|_| {
            let mut img = Image::new_target_texture(res, res, TextureFormat::Rgba16Float, None);
            img.sampler = ImageSampler::linear();
            images.add(img)
        })
        .collect();

    // the six quads seen by the headset; UV inset matches the extra pixel of the capture
    let inset = 1.0 / res as f32;
    for (i, (d, up)) in FACES.iter().enumerate() {
        let r = d.cross(*up);
        let c = *d * BOX_R;
        let (a, b) = (r * BOX_R, *up * BOX_R);
        let pos = vec![(c - a + b).to_array(), (c + a + b).to_array(), (c + a - b).to_array(), (c - a - b).to_array()];
        let uv = vec![[inset, inset], [1.0 - inset, inset], [1.0 - inset, 1.0 - inset], [inset, 1.0 - inset]];
        let n = (-*d).to_array();
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![n; 4])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
            .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color_texture: Some(faces[i].clone()),
                unlit: true,
                fog_enabled: false,
                cull_mode: None,
                ..default()
            })),
            Transform::from_translation(EYE),
        ));
    }
    // the capture camera (fog is added and kept in sync by the main app, like every camera)
    commands.spawn((
        Camera3d::default(),
        Camera { order: -10, ..default() },
        RenderTarget::Image(faces[0].clone().into()),
        Projection::Perspective(PerspectiveProjection {
            fov: 2.0 * tan_half(res).atan(),
            aspect_ratio: 1.0,
            near: 1.0,
            far: 60000.0,
            ..default()
        }),
        Hdr,
        Tonemapping::None,
        DebandDither::Disabled,
        Msaa::Sample4,
        far(),
        Transform::from_translation(EYE),
        PanoCam,
    ));
    commands.insert_resource(Panorama { faces, next: 0 });
}

fn next_face(mut pano: ResMut<Panorama>, mut q: Query<(&mut Transform, &mut RenderTarget), With<PanoCam>>) {
    let i = pano.next;
    pano.next = (i + 1) % 6;
    let (d, up) = FACES[i];
    for (mut t, mut target) in &mut q {
        *t = Transform::from_translation(EYE).looking_to(d, up);
        *target = RenderTarget::Image(pano.faces[i].clone().into());
    }
}
