use bevy::{
    prelude::*,
    utils::hashbrown::{HashMap, HashSet},
};
use indexmap::IndexSet;

use crate::{CellIdToEntity, PastCell, Path, TargetCell};

const NEIGHBORS: [(IVec3, f32); 8] = [
    (IVec3::new(0, 0, 1), 2.),     // up
    (IVec3::new(-1, 0, 0), 2.),    // left
    (IVec3::new(0, 0, -1), 2.),    // down
    (IVec3::new(1, 0, 0), 2.),     // right
    (IVec3::new(-1, 0, 1), 1.41),  // up left
    (IVec3::new(-1, 0, -1), 1.41), // down left
    (IVec3::new(1, 0, -1), 1.41),  // right down
    (IVec3::new(1, 0, 1), 1.41),   // up right
];

pub fn plugin(app: &mut App) {
    app.add_systems(Update, render_path);
}

#[derive(Component, Clone, Copy)]
pub struct MoveCost(pub f32);

impl Default for MoveCost {
    fn default() -> Self {
        MoveCost(10.)
    }
}

//todo make return Result

#[allow(dead_code)]
pub fn a_star<T: bevy::ecs::query::QueryFilter>(
    start: IVec3,
    end: IVec3,
    cells: &Query<&MoveCost, T>,
    id_to_cell: &super::CellIdToEntity,
) -> Option<Vec<IVec3>> {
    if let Some(end) = id_to_cell.get_by_id(&end) {
        if let Ok(cost) = cells.get(end) {
            if cost.0.is_infinite() {
                return None;
            }
        } else {
            return None;
        }
    }

    let mut open = IndexSet::new();
    open.insert(start);
    let mut from = HashMap::new();
    let mut g_score = HashMap::new();
    g_score.insert(start, 0.);

    let mut f_score = HashMap::new();
    f_score.insert(start, 0.);
    let end_f32 = end.as_vec3();
    // let mut checked = HashSet::default();
    while !open.is_empty() {
        open.sort_by(|a, b| {
            f_score
                .get(b)
                .unwrap_or(&f32::INFINITY)
                .partial_cmp(f_score.get(a).unwrap_or(&f32::INFINITY))
                .unwrap_or(std::cmp::Ordering::Less)
        });
        let current = *open.last().expect("Open is not empty");
        if current == end {
            return Some(reconstruct_path(current, from));
        }
        open.shift_remove(&current);
        let Some(current_entity) = id_to_cell.get_by_id(&current) else {
            error!("cell({}) not in map", current);
            continue;
        };
        let c_cost = cells
            .get(current_entity)
            .copied()
            .unwrap_or(MoveCost(f32::INFINITY));
        for (n, cost) in NEIGHBORS {
            let n = current + n;
            let Some(n_entity) = id_to_cell.get_by_id(&n) else {
                continue;
            };
            let n_cost = cells
                .get(n_entity)
                .copied()
                .unwrap_or(MoveCost(f32::INFINITY));
            let tentative_g = g_score.get(&current).copied().unwrap_or(f32::INFINITY)
                + (c_cost.0 + n_cost.0) / cost;
            if tentative_g < g_score.get(&n).copied().unwrap_or(f32::INFINITY) {
                from.insert(n, current);
                g_score.insert(n, tentative_g);
                f_score.insert(n, tentative_g + n.as_vec3().distance(end_f32) * 3.);
                open.insert(n);
            }
        }
    }
    None
}

#[allow(dead_code)]
pub fn a_star_debug<T: bevy::ecs::query::QueryFilter>(
    start: IVec3,
    end: IVec3,
    cells: &Query<&MoveCost, T>,
    id_to_cell: &super::CellIdToEntity,
) -> Option<(Vec<IVec3>, HashSet<IVec3>)> {
    let mut open = IndexSet::new();
    open.insert(start);
    let mut from = HashMap::new();
    let mut g_score = HashMap::new();
    g_score.insert(start, 0.);

    let mut f_score = HashMap::new();
    f_score.insert(start, 0.);
    let end_f32 = end.as_vec3();
    let mut checked = HashSet::default();
    while !open.is_empty() {
        open.sort_by(|a, b| {
            f_score
                .get(b)
                .unwrap_or(&f32::INFINITY)
                .partial_cmp(f_score.get(a).unwrap_or(&f32::INFINITY))
                .unwrap_or(std::cmp::Ordering::Less)
        });
        let current = *open.last().expect("Open is not empty");
        checked.insert(current);
        if current == end {
            return Some((reconstruct_path(current, from), checked));
        }
        open.shift_remove(&current);
        let Some(current_entity) = id_to_cell.get_by_id(&current) else {
            error!("cell({}) not in map", current);
            continue;
        };
        let c_cost = cells
            .get(current_entity)
            .copied()
            .unwrap_or(MoveCost(f32::INFINITY));
        for (n, cost) in NEIGHBORS {
            let n = current + n;
            let Some(n_entity) = id_to_cell.get_by_id(&n) else {
                continue;
            };
            let n_cost = cells
                .get(n_entity)
                .copied()
                .unwrap_or(MoveCost(f32::INFINITY));
            let tentative_g = g_score.get(&current).copied().unwrap_or(f32::INFINITY)
                + (c_cost.0 + n_cost.0) / cost;
            if tentative_g < g_score.get(&n).copied().unwrap_or(f32::INFINITY) {
                from.insert(n, current);
                g_score.insert(n, tentative_g);
                f_score.insert(n, tentative_g + n.as_vec3().distance(end_f32) * 3.);
                open.insert(n);
            }
        }
    }
    None
}

fn reconstruct_path(mut current: IVec3, path: HashMap<IVec3, IVec3>) -> Vec<IVec3> {
    let mut out = vec![current];
    while path.contains_key(&current) {
        current = *path.get(&current).expect("path has node");
        out.push(current);
    }
    out.reverse();
    out
}

fn render_path(
    paths: Query<(&GlobalTransform, &Path, &TargetCell, &PastCell)>,
    cells: Query<&Transform>,
    mut gizmo: Gizmos,
    map: Res<CellIdToEntity>,
) {
    for (pos, path, next, last) in &paths {
        let Some(last) = map.get_by_id(&last.cell) else {
            continue;
        };
        let Ok(last) = cells.get(last) else {
            continue;
        };
        gizmo.sphere(
            Isometry3d::from_translation(last.translation + Vec3::Y),
            0.2,
            Color::linear_rgb(0., 1., 0.),
        );
        let Some(next) = map.get_by_id(&next.0) else {
            continue;
        };
        let Ok(next) = cells.get(next) else {
            continue;
        };
        gizmo.sphere(
            Isometry3d::from_translation(next.translation + Vec3::Y),
            0.2,
            Color::linear_rgb(1., 1., 0.),
        );
        let mut line = vec![pos.translation() + Vec3::Y, next.translation + Vec3::Y];
        for segment in path.0.iter() {
            let Some(next) = map.get_by_id(segment) else {
                continue;
            };
            let Ok(next) = cells.get(next) else {
                continue;
            };
            line.push(next.translation + Vec3::Y);
        }
        gizmo.linestrip(line, bevy::color::palettes::css::RED);
    }
}
