use core::{f32, f64};

use animations::Animation;
use bevy::{core_pipeline::experimental::taa, prelude::*, utils::HashMap};
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
                terrain::add_terrain_mesh,
            ),
        )
        .add_plugins((animations::plugin, path_finding::plugin));
    #[cfg(debug_assertions)]
    app.add_systems(FixedUpdate, random_move)
        .insert_resource(Time::<Fixed>::from_hz(1.));
    // app.add_systems(Update, (color_target, color_path, clear_color, random_move));
    // .add_plugins(Picki);
    app.run();
}

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

fn ray_casting(
    mut clicks: EventReader<Pointer<Click>>,
    terrain: Query<(), With<Terrain>>,
    cells: Query<&MoveCost, With<Cell>>,
    mut player: Query<(&mut Path, &mut TargetCell), With<Player>>,
    map: Res<CellIdToEntity>,

    mut color_cells: Query<(&mut MeshMaterial3d<StandardMaterial>, &MoveCost)>,
    assets: Res<CellAssets>,
) {
    for click in clicks.read() {
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
        let Some(cell_e) = map.get_by_id(&cell) else {
            warn!("Cell ({}) not in map", cell);
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

        let Ok((mut path, target)) = player.get_single_mut() else {
            error!("No Player");
            return;
        };

        let Some((new_path, check)) = path_finding::a_star_debug(target.0, cell, &cells, &map)
        else {
            error!("path find failed");
            continue;
        };

        for (mut cell, cost) in &mut color_cells {
            cell.0 = if cost.0 > 100. {
                assets.solid.clone_weak()
            } else if cost.0 > 2. {
                assets.slow.clone_weak()
            } else if cost.0 < 2. {
                assets.path_material.clone_weak()
            } else {
                assets.normal_material.clone_weak()
            };
        }

        for checked in check {
            let Some(entity) = map.id_to_entity.get(&checked) else {
                continue;
            };
            let Ok((mut cell, _)) = color_cells.get_mut(*entity) else {
                continue;
            };
            cell.0 = assets.checked.clone_weak();
        }

        for checked in new_path.iter() {
            let Some(entity) = map.id_to_entity.get(checked) else {
                continue;
            };
            let Ok((mut cell, _)) = color_cells.get_mut(*entity) else {
                continue;
            };
            cell.0 = assets.path_material.clone_weak();
        }

        path.0.clear();
        path.0.extend(new_path);
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
struct TargetCell(IVec3);

#[derive(Component, Default)]
struct PastCell {
    cell: IVec3,
    start_time: f32,
}

#[derive(Component, Default)]
#[require(TargetCell, PastCell)]
struct Path(std::collections::VecDeque<IVec3>);

fn move_entity(
    time: Res<Time>,
    mut entities: Query<
        (
            &mut Transform,
            &mut TargetCell,
            &mut PastCell,
            &mut Path,
            &mut Animation,
        ),
        Without<Cell>,
    >,
    cells: Query<&Transform, With<Cell>>,
    map: Res<CellIdToEntity>,
    gloabl: Query<&GlobalTransform, With<Cell>>,
) {
    for (mut pos, mut target, mut past, mut next, mut animation) in &mut entities {
        if target.0 == past.cell {
            if let Some(next) = next.0.pop_front() {
                target.0 = next;
                if *animation != Animation::Walk {
                    *animation = Animation::Walk;
                }
                continue;
            } else if *animation != Animation::Idle {
                *animation = Animation::Idle;
            };
        }
        let Some(target_e) = map.get_by_id(&target.0) else {
            warn!("Target ({}) not in map", target.0);
            continue;
        };
        let Ok(target_pos) = cells.get(target_e).cloned() else {
            warn!("Target ({}) not is not an entity", target.0);
            continue;
        };
        let current = pos.translation;

        if current.distance_squared(target_pos.translation) < 0.001 {
            past.cell = target.0;
            past.start_time = time.elapsed_secs();
            continue;
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
            (time.elapsed_secs() - past.start_time).clamp(0., 1.),
        );
        pos.translation = target;

        let Ok(g_pos) = gloabl.get(target_e).cloned() else {
            continue;
        };
        pos.look_at(target_pos.translation, Vec3::Y);
        pos.rotate_local_y(f32::consts::PI);
    }
}

fn color_target(
    cell_map: Res<CellIdToEntity>,
    mut cells: Query<&mut MeshMaterial3d<StandardMaterial>>,
    targeting: Query<&TargetCell, Changed<TargetCell>>,
    assets: Res<CellAssets>,
) {
    for target in &targeting {
        let Some(id) = cell_map.id_to_entity.get(&target.0).copied() else {
            error!("Cell({}) not in map", target.0);
            continue;
        };
        let Ok(mut cell) = cells.get_mut(id) else {
            error!("Cell({}) Entity({}) missing material", target.0, id);
            continue;
        };
        cell.0 = assets.target_material.clone();
    }
}

fn color_path(
    cell_map: Res<CellIdToEntity>,
    mut cells: Query<&mut MeshMaterial3d<StandardMaterial>>,
    targeting: Query<&Path, Changed<Path>>,
    assets: Res<CellAssets>,
) {
    for target in &targeting {
        for target in target.0.iter() {
            let Some(id) = cell_map.id_to_entity.get(target).copied() else {
                error!("Cell({}) not in map", target);
                continue;
            };
            let Ok(mut cell) = cells.get_mut(id) else {
                error!("Cell({}) Entity({}) missing material", target, id);
                continue;
            };
            cell.0 = assets.path_material.clone();
        }
    }
}

fn clear_color(
    cell_map: Res<CellIdToEntity>,
    mut cells: Query<&mut MeshMaterial3d<StandardMaterial>>,
    targeting: Query<&PastCell, Changed<PastCell>>,
    assets: Res<CellAssets>,
) {
    for target in &targeting {
        let Some(id) = cell_map.id_to_entity.get(&target.cell).copied() else {
            error!("Cell({}) not in map", target.cell);
            continue;
        };
        let Ok(mut cell) = cells.get_mut(id) else {
            error!("Cell({}) Entity({}) missing material", target.cell, id);
            continue;
        };
        cell.0 = assets.normal_material.clone();
    }
}

fn random_move(
    mut entities: Query<(&mut Path, &TargetCell), (Changed<Path>, Without<Player>)>,
    cells: Query<&MoveCost, With<Cell>>,
    map: Res<CellIdToEntity>,
) {
    for (mut path, next) in &mut entities {
        if path.0.is_empty() {
            let Some(new_path) = path_finding::a_star(
                next.0,
                IVec3::new(
                    rand::thread_rng().gen_range(-50..50),
                    0,
                    rand::thread_rng().gen_range(-50..50),
                ),
                &cells,
                &map,
            ) else {
                error!("Failed to find path");
                continue;
            };
            path.0.extend(new_path);
        }
    }
}
