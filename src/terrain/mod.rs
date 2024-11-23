use std::{
    borrow::Cow,
    hash::{Hash, Hasher},
    usize,
};

use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemId,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureFormat},
        texture::ImageSampler,
    },
};

use crate::{path_finding::MoveCost, ui::ContextActions, Cell, Path, Player, Root, Target};

mod objects;

pub fn plugin(app: &mut App) {
    app.init_asset::<Biome>()
        .init_resource::<Biomes>()
        .init_resource::<MoveTarget>()
        .init_resource::<TerrainContext>()
        .add_plugins(objects::plugin)
        .add_systems(Update, add_terrain_mesh);
}

const MAP_SIZE: isize = 1000;
const UMAP_SIZE: usize = MAP_SIZE as usize;
const HALF_MAP: isize = MAP_SIZE / 2;
const MAP_VOLUME: usize = (MAP_SIZE * MAP_SIZE) as usize;

#[derive(Component)]
pub struct Terrain {
    seed: u32,
    hight_map: Vec<f32>,
    heat_map: Vec<f32>,
    biome_map: Vec<Handle<Biome>>,
}

impl Terrain {
    pub fn new(seed: u32) -> Terrain {
        let noise: noise::Fbm<noise::OpenSimplex> = noise::Fbm::new(seed);
        use noise::NoiseFn;
        let mut hights = Vec::with_capacity(MAP_VOLUME);
        let mut heats = Vec::with_capacity(MAP_VOLUME);
        for z in -HALF_MAP..HALF_MAP {
            for x in -HALF_MAP..HALF_MAP {
                hights.push(
                    ((noise.get([x as f64 * 0.026, z as f64 * 0.026]) as f32 + 0.2) * 2.25)
                        .clamp(0., 1.),
                );
                heats.push(
                    ((noise.get([x as f64 * 0.0036, z as f64 * 0.0016]) as f32 + 0.2) * 2.5)
                        .clamp(0., 1.),
                );
            }
        }

        let biomes = BiomeRule::generate_map::<UMAP_SIZE, UMAP_SIZE>(
            &[
                BiomeRule {
                    priority: 0,
                    biome: Biome::get_handel("Grass"),
                    min_hight: 0.2,
                    max_hight: 0.8,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: -1,
                    biome: Biome::get_handel("Water"),
                    min_hight: 0.0,
                    max_hight: 0.5,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: 0,
                    biome: Biome::get_handel("Sand"),
                    min_hight: 0.1,
                    max_hight: 0.2,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: -1,
                    biome: Biome::get_handel("Mountain"),
                    min_hight: 0.5,
                    max_hight: 1.0,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: -1,
                    biome: Biome::get_handel("Mountain_Snow"),
                    min_hight: 0.8,
                    max_hight: 1.0,
                    min_temperature: 0.,
                    max_temperature: 0.5,
                },
            ],
            &heats,
            &hights,
        );
        Terrain {
            seed,
            heat_map: heats,
            hight_map: hights,
            biome_map: biomes,
        }
    }

    pub fn make_mesh(&self) -> Mesh {
        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::all(),
        );
        let mut points = Vec::new();
        let mut indices = Vec::new();
        let mut uvs = Vec::new();
        for z in 0..MAP_SIZE {
            for x in 0..MAP_SIZE {
                let index = (x + z * MAP_SIZE) as u32;
                let hight = self.hight_map[index as usize];
                points.push([(x - HALF_MAP) as f32, hight * 10., (z - HALF_MAP) as f32]);
                uvs.push([x as f32 / MAP_SIZE as f32, z as f32 / MAP_SIZE as f32]);
                if x == MAP_SIZE - 1 || z == MAP_SIZE - 1 {
                    continue;
                }
                indices.extend_from_slice(&[
                    index + 1,
                    index + MAP_SIZE as u32,
                    index + MAP_SIZE as u32 + 1,
                    index,
                    index + MAP_SIZE as u32,
                    index + 1,
                ]);
            }
        }
        mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, points);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);

        mesh
    }

    pub fn make_texture(&self, biomes: &Assets<Biome>) -> Image {
        let size = Extent3d {
            width: MAP_SIZE as u32,
            height: MAP_SIZE as u32,
            depth_or_array_layers: 1,
        };
        let mut data = Vec::new();
        for z in 0..MAP_SIZE {
            for x in 0..MAP_SIZE {
                let index = (x + z * MAP_SIZE) as usize;
                let biome = &self.biome_map[index];
                let Some(biome) = biomes.get(biome.id()) else {
                    warn!("biome not loaded");
                    continue;
                };
                let color = biome.color.to_srgba();
                data.extend_from_slice(&[
                    (color.red * 256.) as u8,
                    (color.green * 256.) as u8,
                    (color.blue * 256.) as u8,
                    255,
                ]);
            }
        }

        let mut image = Image::new(
            size,
            bevy::render::render_resource::TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        image.sampler = ImageSampler::nearest();
        image
    }
}

#[derive(Resource)]
struct TerrainContext {
    on_open: SystemId,
    walk: SystemId,
}

#[derive(Resource, Default)]
struct MoveTarget(IVec3, Option<Entity>);

fn set_move_target(mut click: EventReader<Pointer<Click>>, mut target: ResMut<MoveTarget>) {
    let Some(click) = click
        .read()
        .filter(|click| click.button == PointerButton::Secondary)
        .last()
    else {
        error!("No Secondary click found");
        return;
    };
    target.1 = Some(click.target);
    target.0 = if let Some(mut pos) = click.hit.position {
        info!("right click target is {}", pos.round());
        pos.y = 0.;
        pos.round().as_ivec3()
    } else {
        error!("No Hit Data");
        return;
    };
}

fn on_walk_context(
    mut commands: Commands,
    player: Query<Entity, With<Player>>,
    target: Res<MoveTarget>,
) {
    for path in &player {
        commands.entity(path).insert(Target(target.0));
    }
}

impl FromWorld for TerrainContext {
    fn from_world(world: &mut World) -> Self {
        let open = world.register_system(set_move_target);
        let walk = world.register_system(on_walk_context);
        TerrainContext {
            on_open: open,
            walk,
        }
    }
}

#[derive(Resource)]
pub struct Biomes(Vec<Handle<Biome>>);

#[derive(Component)]
struct BiomeCell(pub Handle<Biome>);

impl FromWorld for Biomes {
    fn from_world(world: &mut World) -> Self {
        let mut assets = world.resource_mut::<Assets<Biome>>();
        let mut out = Vec::new();
        assets.insert(
            Biome::get_handel("Grass").id(),
            Biome {
                name: "Grass".into(),
                move_cost: 10.,
                color: bevy::color::palettes::css::GREEN.into(),
            },
        );
        out.push(
            assets
                .get_strong_handle(Biome::get_handel("Grass").id())
                .unwrap(),
        );
        assets.insert(
            Biome::get_handel("Water").id(),
            Biome {
                name: "Water".into(),
                move_cost: f32::INFINITY,
                color: bevy::color::palettes::css::NAVY.into(),
            },
        );
        out.push(
            assets
                .get_strong_handle(Biome::get_handel("Water").id())
                .unwrap(),
        );
        assets.insert(
            Biome::get_handel("Sand").id(),
            Biome {
                name: "Sand".into(),
                move_cost: 15.,
                color: bevy::color::palettes::css::YELLOW.into(),
            },
        );
        out.push(
            assets
                .get_strong_handle(Biome::get_handel("Sand").id())
                .unwrap(),
        );
        assets.insert(
            Biome::get_handel("Mountain").id(),
            Biome {
                name: "Mountain".into(),
                move_cost: f32::INFINITY,
                color: bevy::color::palettes::css::GRAY.into(),
            },
        );
        out.push(
            assets
                .get_strong_handle(Biome::get_handel("Mountain").id())
                .unwrap(),
        );
        assets.insert(
            Biome::get_handel("Mountain_Snow").id(),
            Biome {
                name: "Mountain_Snow".into(),
                move_cost: f32::INFINITY,
                color: bevy::color::palettes::css::GRAY.into(),
            },
        );
        out.push(
            assets
                .get_strong_handle(Biome::get_handel("Mountain_Snow").id())
                .unwrap(),
        );
        Biomes(out)
    }
}

fn add_terrain_mesh(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    terrains: Query<(Entity, &Terrain), (Added<Terrain>, Without<Mesh3d>)>,
    biomes: Res<Assets<Biome>>,
    context: Res<TerrainContext>,
) {
    for (entity, terrain) in &terrains {
        let texture = asset_server.add(terrain.make_texture(&biomes));
        commands
            .entity(entity)
            .insert((
                Name::new("Terrain"),
                Root,
                Mesh3d(asset_server.add(terrain.make_mesh())),
                MeshMaterial3d(asset_server.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: Some(texture),
                    unlit: true,
                    ..Default::default()
                })),
                ContextActions {
                    on_open: Some(context.on_open),
                    options: vec![("Walk".into(), context.walk)],
                    on_close: None,
                },
            ))
            .with_children(|commands| {
                for z in -HALF_MAP..HALF_MAP {
                    for x in -HALF_MAP..HALF_MAP {
                        let index = (x + HALF_MAP + (z + HALF_MAP) * MAP_SIZE) as usize;
                        let hight = terrain.hight_map[index];
                        let biome_handle = &terrain.biome_map[index];
                        let Some(biome) = biomes.get(biome_handle) else {
                            warn!("biome not loaded");
                            continue;
                        };
                        let color = biome.color;
                        let cost = biome.move_cost;

                        commands.spawn((
                            BiomeCell(biome_handle.clone()),
                            Transform::from_translation(Vec3::new(x as f32, hight * 10., z as f32)),
                            Cell,
                            MoveCost(cost),
                        ));
                    }
                }
            });
    }
}

struct BiomeRule {
    priority: i8,
    biome: Handle<Biome>,
    min_hight: f32,
    max_hight: f32,
    min_temperature: f32,
    max_temperature: f32,
}

#[derive(Clone, Reflect, Asset)]
struct Biome {
    name: Cow<'static, str>,
    move_cost: f32,
    color: Color,
}

impl Biome {
    fn get_handel(name: impl AsRef<str>) -> Handle<Biome> {
        let mut hasher = std::hash::DefaultHasher::new();
        name.as_ref().hash(&mut hasher);
        Handle::Weak(AssetId::Uuid {
            uuid: uuid::Uuid::from_u128(hasher.finish() as u128),
        })
    }
}

impl BiomeRule {
    fn generate_map<const W: usize, const H: usize>(
        biomes: &[BiomeRule],
        heat_map: &[f32],
        hight_map: &[f32],
    ) -> Vec<Handle<Biome>> {
        let mut map = Vec::with_capacity(W * H);
        for h in 0..H {
            for w in 0..W {
                let index = w + h * W;
                let heat = heat_map[index];
                let hight = hight_map[index];
                let mut options = biomes.iter().collect::<Vec<_>>();
                options.retain(|&option| {
                    option.max_hight >= hight
                        && option.min_hight <= hight
                        && option.min_temperature <= heat
                        && option.max_temperature >= heat
                });
                options.sort_by(|a, b| b.priority.cmp(&a.priority));
                if let Some(choice) = options.first() {
                    map.push(choice.biome.clone());
                } else {
                    map.push(Biome::get_handel("Void"));
                }
            }
        }
        map
    }
}
