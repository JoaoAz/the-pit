//! Quest Touch controller buttons and hand markers (VR mode only).
//! A = mechanical/clinical, B = next shaft size, X = start/stop the moving ring.

use std::ops::Deref;

use bevy::prelude::*;
use bevy_mod_openxr::{
    action_binding::{OxrSendActionBindings, OxrSuggestActionBinding},
    action_set_attaching::OxrAttachActionSet,
    action_set_syncing::{OxrActionSetSyncSet, OxrSyncActionSet},
    openxr_session_running,
    resources::OxrInstance,
    session::OxrSession,
};
use bevy_mod_xr::session::{XrSessionCreated, session_available};
use openxr::Posef;

use crate::{Mood, RingMotion, Size};
use crate::shaft::SIZES;

#[derive(Resource)]
struct Actions {
    set: openxr::ActionSet,
    mood: openxr::Action<bool>,
    size: openxr::Action<bool>,
    ring: openxr::Action<bool>,
    left: openxr::Action<Posef>,
    right: openxr::Action<Posef>,
}

pub struct XrInputPlugin;

impl Plugin for XrInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, create_actions.run_if(session_available))
            .add_systems(OxrSendActionBindings, suggest_bindings)
            .add_systems(XrSessionCreated, (attach_set, spawn_hands))
            .add_systems(PreUpdate, sync_actions.before(OxrActionSetSyncSet).run_if(openxr_session_running))
            .add_systems(Update, read_buttons.run_if(openxr_session_running));
    }
}

fn create_actions(instance: Res<OxrInstance>, mut commands: Commands) {
    let set = instance.create_action_set("pit", "The Pit", 0).unwrap();
    let mood = set.create_action("mood", "Mudar estado", &[]).unwrap();
    let size = set.create_action("size", "Mudar tamanho", &[]).unwrap();
    let ring = set.create_action("ring", "Anel", &[]).unwrap();
    let left = set.create_action("left_pose", "Mao esquerda", &[]).unwrap();
    let right = set.create_action("right_pose", "Mao direita", &[]).unwrap();
    commands.insert_resource(Actions { set, mood, size, ring, left, right });
}

fn suggest_bindings(actions: Res<Actions>, mut out: MessageWriter<OxrSuggestActionBinding>) {
    let profile = "/interaction_profiles/oculus/touch_controller";
    let b = |action: openxr::sys::Action, path: &'static str| OxrSuggestActionBinding {
        action,
        interaction_profile: profile.into(),
        bindings: vec![path.into()],
    };
    out.write(b(actions.mood.as_raw(), "/user/hand/right/input/a/click"));
    out.write(b(actions.size.as_raw(), "/user/hand/right/input/b/click"));
    out.write(b(actions.ring.as_raw(), "/user/hand/left/input/x/click"));
    out.write(b(actions.left.as_raw(), "/user/hand/left/input/grip/pose"));
    out.write(b(actions.right.as_raw(), "/user/hand/right/input/grip/pose"));
}

fn attach_set(actions: Res<Actions>, mut attach: MessageWriter<OxrAttachActionSet>) {
    attach.write(OxrAttachActionSet(actions.set.clone()));
}

fn sync_actions(actions: Res<Actions>, mut sync: MessageWriter<OxrSyncActionSet>) {
    sync.write(OxrSyncActionSet(actions.set.clone()));
}

/// Small blocks at the controllers: your own hands are the strongest scale reference in VR.
fn spawn_hands(
    actions: Res<Actions>,
    session: Res<OxrSession>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(0.08, 0.04, 0.14));
    let mat = materials.add(StandardMaterial { base_color: Color::srgb(0.9, 0.5, 0.1), ..default() });
    for a in [&actions.left, &actions.right] {
        if let Ok(space) = session.create_action_space(a, openxr::Path::NULL, Isometry3d::IDENTITY) {
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), space));
        }
    }
}

fn pressed(action: &openxr::Action<bool>, session: &OxrSession) -> bool {
    match action.state(session.deref(), openxr::Path::NULL) {
        Ok(s) => s.changed_since_last_sync && s.current_state,
        Err(_) => false,
    }
}

fn read_buttons(
    actions: Res<Actions>,
    session: Res<OxrSession>,
    mut mood: ResMut<Mood>,
    mut size: ResMut<Size>,
    mut ring: ResMut<RingMotion>,
) {
    if pressed(&actions.mood, &session) {
        mood.target = if mood.target < 0.5 { 1.0 } else { 0.0 };
    }
    if pressed(&actions.size, &session) {
        size.0 = (size.0 + 1) % SIZES.len();
    }
    if pressed(&actions.ring, &session) {
        ring.moving = !ring.moving;
    }
}
