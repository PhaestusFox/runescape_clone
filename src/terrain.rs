use std::{borrow::Cow, usize};

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureFormat},
        texture::ImageSampler,
    },
};

use crate::{path_finding::MoveCost, Cell};

const MAP_SIZE: isize = 1000;
const UMAP_SIZE: usize = MAP_SIZE as usize;
const HALF_MAP: isize = MAP_SIZE / 2;
const MAP_VOLUME: usize = (MAP_SIZE * MAP_SIZE) as usize;

#[derive(Component)]
pub struct Terrain {
    hight_map: Vec<f32>,
    heat_map: Vec<f32>,
    biome_map: Vec<Biome>,
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
                    biome: Biome {
                        name: "Grass".into(),
                        move_cost: 10.,
                        color: bevy::color::palettes::css::GREEN.into(),
                    },
                    min_hight: 0.2,
                    max_hight: 0.8,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: -1,
                    biome: Biome {
                        name: "Water".into(),
                        move_cost: f32::INFINITY,
                        color: bevy::color::palettes::css::NAVY.into(),
                    },
                    min_hight: 0.0,
                    max_hight: 0.5,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: 0,
                    biome: Biome {
                        name: "Sand".into(),
                        move_cost: 15.,
                        color: bevy::color::palettes::css::YELLOW.into(),
                    },
                    min_hight: 0.1,
                    max_hight: 0.2,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: -1,
                    biome: Biome {
                        name: "Mountain".into(),
                        move_cost: f32::INFINITY,
                        color: bevy::color::palettes::css::GRAY.into(),
                    },
                    min_hight: 0.5,
                    max_hight: 1.0,
                    min_temperature: 0.,
                    max_temperature: 1.,
                },
                BiomeRule {
                    priority: -1,
                    biome: Biome {
                        name: "Mountain_Snow".into(),
                        move_cost: f32::INFINITY,
                        color: bevy::color::palettes::css::WHITE.into(),
                    },
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

    pub fn make_texture(&self) -> Image {
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

pub fn add_terrain_mesh(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    terrains: Query<(Entity, &Terrain), (Added<Terrain>, Without<Mesh3d>)>,
    assets: Res<super::CellAssets>,
) {
    for (entity, terrain) in &terrains {
        println!("here3");
        let texture = asset_server.add(terrain.make_texture());
        commands
            .entity(entity)
            .insert((
                Mesh3d(asset_server.add(terrain.make_mesh())),
                MeshMaterial3d(asset_server.add(StandardMaterial {
                    base_color: Color::WHITE,
                    base_color_texture: Some(texture),
                    // unlit: true,
                    ..Default::default()
                })),
            ))
            .with_children(|commands| {
                for z in -HALF_MAP..HALF_MAP {
                    for x in -HALF_MAP..HALF_MAP {
                        let index = (x + HALF_MAP + (z + HALF_MAP) * MAP_SIZE) as usize;
                        let hight = terrain.hight_map[index];
                        let biome = &terrain.biome_map[index];
                        let color = biome.color;
                        let cost = biome.move_cost;

                        commands.spawn((
                            // Mesh3d(assets.mesh.clone()),
                            // MeshMaterial3d(asset_server.add(StandardMaterial {
                            //     base_color: color,
                            //     ..Default::default()
                            // })),
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
    biome: Biome,
    min_hight: f32,
    max_hight: f32,
    min_temperature: f32,
    max_temperature: f32,
}

#[derive(Clone)]
struct Biome {
    name: Cow<'static, str>,
    move_cost: f32,
    color: Color,
}

impl BiomeRule {
    fn generate_map<const W: usize, const H: usize>(
        biomes: &[BiomeRule],
        heat_map: &[f32],
        hight_map: &[f32],
    ) -> Vec<Biome> {
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
                    map.push(Biome {
                        name: "void".into(),
                        move_cost: f32::INFINITY,
                        color: Color::BLACK,
                    });
                }
            }
        }
        map
    }
}
