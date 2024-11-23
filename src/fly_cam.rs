use core::f32;

use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, PrimaryWindow},
};

#[derive(Component)]
pub struct FlyCam;

impl Plugin for FlyCam {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlySettings>()
            .add_systems(Update, cursor_toggle)
            .add_systems(Update, (player_look, player_move))
            .add_systems(Startup, cursor_grab);
    }
}

fn player_move(
    mut camera: Query<&mut Transform, With<FlyCam>>,
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    settings: Res<FlySettings>,
) {
    for mut camera in &mut camera {
        let mut delta = Vec3::ZERO;

        if input.pressed(KeyCode::KeyW) {
            delta.z += 1.;
        }
        if input.pressed(KeyCode::KeyS) {
            delta.z -= 1.;
        }
        if input.pressed(KeyCode::KeyA) {
            delta.x -= 1.;
        }
        if input.pressed(KeyCode::KeyD) {
            delta.x += 1.;
        }
        let mut forward = camera.forward().as_vec3();
        forward.y = 0.;
        forward = forward.normalize();
        let mut right = camera.right().as_vec3();
        right.y = 0.;
        right = right.normalize();
        if input.pressed(KeyCode::Space) {
            delta.y += 1.;
        }
        if input.pressed(KeyCode::ShiftLeft) {
            delta.y -= 1.;
        }

        let next = (forward * delta.z + right * delta.x + Vec3::Y * delta.y)
            * time.delta_secs()
            * settings.speed;
        if !next.is_nan() {
            camera.translation += next;
        }
    }
}

fn player_look(
    settings: Res<FlySettings>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut query: Query<&mut Transform, With<FlyCam>>,
    mut input: EventReader<MouseMotion>,
) {
    let delta: Vec2 = input.read().map(|v| v.delta).sum();
    if let Ok(window) = primary_window.get_single() {
        if window.cursor_options.grab_mode != CursorGrabMode::None {
            for mut transform in query.iter_mut() {
                let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
                let window_scale = window.height().min(window.width());
                pitch -= (settings.mouse_sensitivity * delta.y * window_scale).to_radians();
                pitch = pitch.clamp(-f32::consts::PI / 2., f32::consts::PI / 2.);
                yaw -= (settings.mouse_sensitivity * delta.x * window_scale).to_radians();
                transform.rotation =
                    Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
            }
        }
    } else {
        warn!("Primary window not found for `player_look`!");
    }
}

fn cursor_release(mut primary_window: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = primary_window.get_single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::None;
        window.cursor_options.visible = true;
    } else {
        warn!("Primary window not found for `initial_grab_cursor`!");
    }
}

fn cursor_grab(mut primary_window: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = primary_window.get_single_mut() {
        window.cursor_options.grab_mode = CursorGrabMode::Confined;
        window.cursor_options.visible = false;
    } else {
        warn!("Primary window not found for `initial_grab_cursor`!");
    }
}

fn cursor_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    key_bindings: Res<FlySettings>,
    mut primary_window: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let Ok(mut window) = primary_window.get_single_mut() {
        if keys.just_pressed(key_bindings.toggle_grab_cursor) {
            match window.cursor_options.grab_mode {
                CursorGrabMode::None => {
                    window.cursor_options.grab_mode = CursorGrabMode::Confined;
                    window.cursor_options.visible = false;
                }
                _ => {
                    window.cursor_options.grab_mode = CursorGrabMode::None;
                    window.cursor_options.visible = true;
                }
            }
        }
    } else {
        warn!("Primary window not found for `cursor_grab`!");
    }
}

#[derive(Resource)]
pub struct FlySettings {
    pub toggle_grab_cursor: KeyCode,
    pub mouse_sensitivity: f32,
    pub speed: f32,
}

impl Default for FlySettings {
    fn default() -> Self {
        Self {
            toggle_grab_cursor: KeyCode::Escape,
            mouse_sensitivity: 0.00012,
            speed: 10.,
        }
    }
}
