use std::time::Duration;

use bevy::prelude::*;

use crate::{File, Player};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (animations, load_animations))
        .add_systems(PostUpdate, bubble_animation);
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Animation {
    Nothing,
    Idle,
    WheelChair0,
    Walk,
    Run,
    Jump,
    Fall,
    Crouch,
    Sit,
    SitArmsForward,
    Die,
    WheelChair1,
    NodHead,
    ShakeHead,
    PointRightHand,
    PointLeftHand,
    PointBoth,
    IDK0,
    IDK1,
    IDK2,
    AttackRight,
    AttackLeft,
    KickRight,
    KickLeft,
    IDK3,
    IDK4,
    IDK5,
    IDK6,
    IDK7,
    IDK8,
    IDK9,
    IDK10,
    IDK11,
}

// fn set_animation(input: Res<ButtonInput<KeyCode>>, mut player: Query<&mut Animation>) {
//     for key in input.get_just_pressed() {
//         let mut digit = match key {
//             KeyCode::Digit0 => 0,
//             KeyCode::Digit1 => 1,
//             KeyCode::Digit2 => 2,
//             KeyCode::Digit3 => 3,
//             KeyCode::Digit4 => 4,
//             KeyCode::Digit5 => 5,
//             KeyCode::Digit6 => 6,
//             KeyCode::Digit7 => 7,
//             KeyCode::Digit8 => 8,
//             KeyCode::Digit9 => 9,
//             _ => continue,
//         };
//         if input.pressed(KeyCode::ShiftLeft) {
//             digit += 10;
//         }
//         if input.pressed(KeyCode::AltLeft) {
//             digit += 10;
//         }
//         if input.pressed(KeyCode::ControlLeft) {
//             digit += 10;
//         }
//         for mut animation in &mut player {
//             animation.0 = digit;
//         }
//     }
// }

fn animations(
    mut animations: Query<
        (&mut AnimationPlayer, &mut AnimationTransitions, &Animation),
        Changed<Animation>,
    >,
) {
    for (mut player, mut transition, animation) in &mut animations {
        transition
            .play(
                &mut player,
                AnimationNodeIndex::new(*animation as usize),
                Duration::from_millis(100),
            )
            .repeat();
    }
}

pub fn load_animations(
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    asset_server: Res<AssetServer>,
    files: Query<&File>,
    parents: Query<&Parent>,
    mut objest: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (entity, mut player) in &mut objest {
        let Ok(file) = files.get(parents.root_ancestor(entity)) else {
            error!("Parent is not root of scene");
            continue;
        };

        let mut clips = Vec::new();
        for i in 0..32 {
            clips.push(asset_server.load(GltfAssetLabel::Animation(i).from_asset(file.0)));
        }
        let (graph, node_indices) = AnimationGraph::from_clips(clips);

        let graph = graphs.add(graph);

        let mut transitions = AnimationTransitions::new();

        transitions
            .play(&mut player, node_indices[0], std::time::Duration::ZERO)
            .repeat();

        commands.entity(entity).insert((
            AnimationGraphHandle(graph.clone()),
            transitions,
            Animation::Idle,
        ));
    }
}

pub fn bubble_animation(
    animation: Query<(Entity, &Animation), (Without<AnimationPlayer>, Changed<Animation>)>,
    mut players: Query<&mut Animation, With<AnimationPlayer>>,
    children: Query<&Children>,
) {
    for (root, animation) in &animation {
        for child in children.iter_descendants(root) {
            if let Ok(mut player) = players.get_mut(child) {
                print!("{}, ", child);
                *player = *animation;
            }
        }
    }
}
