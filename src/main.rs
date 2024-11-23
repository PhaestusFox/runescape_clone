use core::f32;

use animations::Animation;
use bevy::{ecs::system::SystemId, prelude::*, utils::HashMap};
use path_finding::MoveCost;
use rand::{seq::SliceRandom, Rng};
use terrain::Terrain;

mod animations;
mod fly_cam;
mod path_finding;
mod terrain;

fn main() {
    let mut app = App::new();
    app.add_plugins((DefaultPlugins, MeshPickingPlugin, fly_cam::FlyCam))
        .init_resource::<CellAssets>()
        .init_resource::<CellIdToEntity>()
        .add_systems(Startup, (spawn_camera, spawn_map, spawn_character))
        .add_systems(
            Update,
            (
                ray_casting,
                move_entity,
                update_cells,
                add_root,
                build_path,
                run_player_action,
            ),
        )
        .add_plugins((
            animations::plugin,
            path_finding::plugin,
            terrain::plugin,
            ui::plugin,
        ));
    #[cfg(debug_assertions)]
    app.add_systems(FixedUpdate, random_move)
        .insert_resource(Time::<Fixed>::from_hz(1.));
    // app.add_systems(Update, (color_target, color_path, clear_color, random_move));
    // .add_plugins(Picki);
    app.run();
}

mod ui;

#[derive(Resource)]
struct CellAssets {
    mesh: Handle<Mesh>,
    normal_material: Handle<StandardMaterial>,
    target_material: Handle<StandardMaterial>,
    path_material: Handle<StandardMaterial>,
    checked: Handle<StandardMaterial>,
    slow: Handle<StandardMaterial>,
    solid: Handle<StandardMaterial>,
}

impl FromWorld for CellAssets {
    fn from_world(world: &mut World) -> Self {
        let mesh = world
            .resource_mut::<Assets<Mesh>>()
            .add(Cuboid::new(0.9, 0.01, 0.9));
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let material_two = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgb(1., 0.0, 0.0),
                ..Default::default()
            });
        let material_three =
            world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::srgb(0.5, 1., 0.5),
                    ..Default::default()
                });
        CellAssets {
            mesh,
            target_material: material_two,
            normal_material: material,
            path_material: material_three,
            slow: world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::srgb(1., 1., 0.),
                    ..Default::default()
                }),
            checked: world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::srgb(0., 1., 1.),
                    ..Default::default()
                }),
            solid: world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: bevy::color::palettes::css::BROWN.into(),
                    ..Default::default()
                }),
        }
    }
}

#[derive(Component)]
struct Cell;

#[derive(Resource, Default)]
struct CellIdToEntity {
    id_to_entity: HashMap<IVec3, Entity>,
    entity_to_id: HashMap<Entity, IVec3>,
}

impl CellIdToEntity {
    pub fn get_by_id(&self, id: &IVec3) -> Option<Entity> {
        self.id_to_entity.get(id).cloned()
    }

    pub fn get_by_entity(&self, entity: Entity) -> Option<IVec3> {
        self.entity_to_id.get(&entity).cloned()
    }
}

fn update_cells(
    cells: Query<(Entity, &Transform), Added<Cell>>,
    mut cell_map: ResMut<CellIdToEntity>,
    mut despawnd: RemovedComponents<Cell>,
) {
    for (entity, cell) in &cells {
        let mut id = cell.translation;
        id.y = 0.;
        if cell_map
            .id_to_entity
            .insert(id.round().as_ivec3(), entity)
            .is_some()
        {
            warn!("Cell({}) is duplicated", id.round().as_ivec3());
        };
    }
    for entity in despawnd.read() {
        if let Some(id) = cell_map.entity_to_id.remove(&entity) {
            cell_map.id_to_entity.remove(&id);
        }
    }
}

fn spawn_camera(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_translation(Vec3::Y * 100.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((Camera3d::default(), fly_cam::FlyCam));
    #[cfg(debug_assertions)]
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            ..Default::default()
        },
    ));
}

fn spawn_map(mut commands: Commands, assets: Res<CellAssets>, asset_server: Res<AssetServer>) {
    commands.spawn((
        Transform::default(),
        Visibility::default(),
        Terrain::new(0),
        Cell,
    ));
}

#[derive(Component)]
struct Root;

fn add_root(mut commands: Commands, scenes: Query<Entity, Added<SceneRoot>>) {
    for entity in &scenes {
        commands.entity(entity).insert(Root);
    }
}

fn ray_casting(
    mut commands: Commands,
    mut clicks: EventReader<Pointer<Click>>,
    terrain: Query<(), With<Terrain>>,
    player: Query<Entity, With<Player>>,
) {
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        if !terrain.contains(click.target) {
            continue;
        }
        let cell = if let Some(pos) = click.hit.position {
            let mut target = pos.round();
            target.y = 0.;
            target.as_ivec3()
        } else {
            error!("Click has no position data");
            continue;
        };
        let player = player.single();
        commands.entity(player).insert(Target(cell));
    }
}

#[derive(Component, Clone, Copy)]
struct Action(SystemId);

fn run_player_action(world: &mut World) {
    let actions = world
        .query::<&Action>()
        .iter(world)
        .cloned()
        .collect::<Vec<_>>();
    for action in &actions {
        world.run_system(action.0).unwrap();
    }
}

fn build_path(
    mut commands: Commands,
    cells: Query<&MoveCost, With<Cell>>,
    mut path_finder: Query<(Entity, &mut Path, &Target, &NextCell, &PastCell)>,
    map: Res<CellIdToEntity>,
) {
    for (entity, mut path, target, next, past) in &mut path_finder {
        let Some(cell_e) = map.get_by_id(&target.0) else {
            warn!("Cell ({}) not in map", target.0);
            continue;
        };
        if let Ok(cost) = cells.get(cell_e) {
            if cost.0.is_infinite() {
                trace!("Clicked Impassable Cell");
                continue;
            }
        } else {
            warn!("Cell Has not MoveCost");
            continue;
        }

        let start = if let Some(next) = next.0 {
            next
        } else {
            past.cell
        };

        let Some((new_path, check)) = path_finding::a_star_debug(start, target.0, &cells, &map)
        else {
            error!("path find failed");
            commands.entity(entity).remove::<Target>();
            continue;
        };

        path.0.clear();
        path.0.extend(new_path);
        commands.entity(entity).remove::<Target>();
    }
}

#[derive(Component)]
struct File(&'static str);

fn spawn_character(mut commands: Commands, asset_server: Res<AssetServer>) {
    let char = [
        "characters/character-female-a.glb#Scene0",
        "characters/character-female-b.glb#Scene0",
        "characters/character-female-c.glb#Scene0",
        "characters/character-female-d.glb#Scene0",
        "characters/character-female-e.glb#Scene0",
        "characters/character-female-f.glb#Scene0",
        "characters/character-male-a.glb#Scene0",
        "characters/character-male-b.glb#Scene0",
        "characters/character-male-c.glb#Scene0",
        "characters/character-male-d.glb#Scene0",
        "characters/character-male-e.glb#Scene0",
        "characters/character-male-f.glb#Scene0",
    ];
    let mut rng = rand::thread_rng();

    let main = *char.choose(&mut rng).expect(">= one str");
    commands.spawn((
        File(main),
        SceneRoot(asset_server.load(main)),
        Player,
        Animation::Idle,
        Path::default(),
    ));
    for _ in 0..10 {
        let main = *char.choose(&mut rng).expect(">= one str");
        commands.spawn((
            File(main),
            Animation::Idle,
            SceneRoot(asset_server.load(*char.choose(&mut rng).expect(">= one str"))),
            Path::default(),
        ));
    }
}

#[derive(Component)]
struct Player;

#[derive(Component, Default)]
struct NextCell(Option<IVec3>);

#[derive(Component)]
struct Target(IVec3);

#[derive(Component, Default)]
struct PastCell {
    cell: IVec3,
    start_time: f32,
}

#[derive(Component, Default)]
#[require(NextCell, PastCell)]
struct Path(std::collections::VecDeque<IVec3>);

fn move_entity(
    time: Res<Time>,
    mut entities: Query<
        (
            &mut Transform,
            &mut NextCell,
            &mut PastCell,
            &mut Path,
            &mut Animation,
        ),
        Without<Cell>,
    >,
    cells: Query<&Transform, With<Cell>>,
    map: Res<CellIdToEntity>,
) {
    for (mut pos, mut target, mut past, mut next, mut animation) in &mut entities {
        if target.0.is_none() && !next.0.is_empty() {
            target.0 = next.0.pop_front();
        }
        let Some(target_cell) = target.0 else {
            continue;
        };
        let Some(target_e) = map.get_by_id(&target_cell) else {
            warn!("Target ({}) not in map", target_cell);
            continue;
        };
        let Ok(mut target_pos) = cells.get(target_e).cloned() else {
            warn!("Target ({}) not is not an entity", target_cell);
            continue;
        };
        let current = pos.translation;
        if current.distance_squared(target_pos.translation) < 0.001 {
            past.cell = target_cell;
            if let Some(next) = next.0.pop_front() {
                past.start_time = time.elapsed_secs();
                target.0 = Some(next);
                let Some(target_e) = map.get_by_id(&next) else {
                    warn!("Target ({}) not in map", next);
                    continue;
                };
                let Ok(next_pos) = cells.get(target_e).cloned() else {
                    warn!("Target ({}) not is not an entity", next);
                    continue;
                };
                target_pos = next_pos;
                if *animation != Animation::Walk {
                    *animation = Animation::Walk;
                }
            } else {
                target.0 = None;
                if *animation != Animation::Idle {
                    *animation = Animation::Idle;
                }
                continue;
            };
        }

        let Some(past_e) = map.get_by_id(&past.cell) else {
            warn!("Target ({}) not in map", past.cell);
            continue;
        };
        let Ok(past_pos) = cells.get(past_e) else {
            warn!("Target ({}) not is not an entity", past.cell);
            continue;
        };

        let target = past_pos.translation.lerp(
            target_pos.translation,
            ((time.elapsed_secs() - past.start_time) * 10.).clamp(0., 0.999),
        );
        pos.translation = target;
        pos.look_at(target_pos.translation, Vec3::Y);
        pos.rotate_local_y(f32::consts::PI);
    }
}

fn random_move(
    mut commands: Commands,
    entities: Query<(&NextCell, &PastCell, Entity), Without<Player>>,
) {
    for (next, past, entity) in &entities {
        if next.0.is_none() {
            commands.entity(entity).insert(Target(
                past.cell
                    + IVec3::new(
                        rand::thread_rng().gen_range(-10..10),
                        0,
                        rand::thread_rng().gen_range(-10..10),
                    ),
            ));
        }
    }
}
