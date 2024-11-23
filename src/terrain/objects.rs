use bevy::{ecs::system::SystemId, prelude::*};
use noise::Add;
use rand::{Rng, SeedableRng};

use crate::{ui::ContextActions, Path, Player, Target};

use super::{Biome, BiomeCell, MoveTarget};

#[derive(Component)]
#[require(Age)]
struct Tree;

#[derive(Component, Debug, Default)]
struct Age(f32);

#[derive(Resource)]
struct TreeContext {
    open: SystemId,
    chop: SystemId,
    walk: SystemId,
}

impl FromWorld for TreeContext {
    fn from_world(world: &mut World) -> Self {
        let open = world.register_system(super::set_move_target);
        let walk = world.register_system(super::on_walk_context);
        let chop = world.register_system(on_chop_context);
        TreeContext { open, chop, walk }
    }
}

#[derive(Component)]
struct Chop(Entity);

fn on_chop(player: Query<(Entity, &Chop, &Path), With<Player>>, mut commands: Commands) {
    for (entity, target, path) in &player {
        if path.0.is_empty() {
            commands.entity(target.0).despawn_recursive();
            commands.entity(entity).remove::<Chop>();
        }
    }
}

fn on_chop_context(
    mut commands: Commands,
    player: Query<Entity, With<Player>>,
    target: Res<MoveTarget>,
) {
    for path in &player {
        commands
            .entity(path)
            .insert((Target(target.0), Chop(target.1.unwrap())));
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (rng_trees, grow_tree, update_age, on_chop))
        .init_resource::<TreeContext>();
}

fn rng_trees(
    mut commands: Commands,
    cells: Query<(Entity, &BiomeCell, &Transform), Added<BiomeCell>>,
    asset_server: Res<AssetServer>,
    context: Res<TreeContext>,
) {
    for (entity, cell, pos) in &cells {
        let id = pos.translation.as_ivec3();
        let seed = id.x ^ id.y ^ id.z;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed as u64);
        if cell.0 == Biome::get_handel("Sand") && rng.gen_bool(0.01) {
            commands.entity(entity).with_children(|p| {
                p.spawn((
                    SceneRoot(asset_server.load("tree.glb#Scene0")),
                    Transform::from_scale(Vec3::splat(0.01)),
                    Tree,
                    ContextActions {
                        on_open: Some(context.open),
                        on_close: None,
                        options: vec![
                            ("Walk".to_string(), context.walk),
                            ("Chop".to_string(), context.chop),
                        ],
                    },
                    Visibility::Visible,
                    Name::new("Palm Tree"),
                ));
            });
        }
    }
}

fn update_age(time: Res<Time>, mut objects: Query<&mut Age>) {
    for mut object in &mut objects {
        object.0 += time.delta_secs();
    }
}

fn grow_tree(mut trees: Query<(&mut Transform, &Age), With<Tree>>) {
    for (mut pos, age) in &mut trees {
        if age.0 < 100. {
            pos.scale = Vec3::splat(age.0 * 0.01);
        }
    }
}
