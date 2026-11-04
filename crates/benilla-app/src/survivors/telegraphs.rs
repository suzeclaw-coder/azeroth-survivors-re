//! Ground attack telegraphs: visual hazard indicators during boss ability windups,
//! detonating on completion if the hero does not evade.

use std::collections::HashMap;

use benilla_world::world_point::WorldPoint;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use super::data::{BossAbilityDef, TelegraphShape};
use super::hero::{self, Hero, HERO_RADIUS};
use super::units::{flat, yaw_toward, Fx, COLOR_HURT};
use super::{Run, RunEntity};

#[derive(Component)]
pub(super) struct Telegraph {
    pub shape: TelegraphShape,
    pub duration: f32,
    pub elapsed: f32,
    pub damage: f32,
    pub spell_id: u32,
    pub source: Entity,
    pub origin: Vec3,
    pub dir: Vec3,
    pub material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub(super) struct TelegraphCache {
    meshes: HashMap<String, Handle<Mesh>>,
}

fn circle_mesh(radius: f32) -> Mesh {
    const SEGMENTS: usize = 36;
    let mut positions = Vec::with_capacity(SEGMENTS + 1);
    let mut normals = Vec::with_capacity(SEGMENTS + 1);
    let mut uvs = Vec::with_capacity(SEGMENTS + 1);
    let mut indices = Vec::with_capacity(SEGMENTS * 3);

    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.5]);

    for i in 0..SEGMENTS {
        let theta = (i as f32) / (SEGMENTS as f32) * std::f32::consts::TAU;
        let x = radius * theta.sin();
        let z = -radius * theta.cos();
        positions.push([x, 0.0, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.5 + 0.5 * theta.sin(), 0.5 - 0.5 * theta.cos()]);
    }

    for i in 1..SEGMENTS {
        indices.extend_from_slice(&[0, (i + 1) as u32, i as u32]);
    }
    indices.extend_from_slice(&[0, 1, SEGMENTS as u32]);

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn cone_mesh(angle_rad: f32, range: f32) -> Mesh {
    const SEGMENTS: usize = 24;
    let mut positions = Vec::with_capacity(SEGMENTS + 2);
    let mut normals = Vec::with_capacity(SEGMENTS + 2);
    let mut uvs = Vec::with_capacity(SEGMENTS + 2);
    let mut indices = Vec::with_capacity(SEGMENTS * 3);

    positions.push([0.0, 0.0, 0.0]);
    normals.push([0.0, 1.0, 0.0]);
    uvs.push([0.5, 0.0]);

    let half = angle_rad * 0.5;
    for i in 0..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32;
        let theta = -half + t * angle_rad;
        let x = range * theta.sin();
        let z = -range * theta.cos();
        positions.push([x, 0.0, z]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([t, 1.0]);
    }

    for i in 1..=SEGMENTS {
        indices.extend_from_slice(&[0, (i + 1) as u32, i as u32]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn line_mesh(length: f32, width: f32) -> Mesh {
    let hw = width * 0.5;
    let positions = vec![
        [-hw, 0.0, 0.0],
        [hw, 0.0, 0.0],
        [hw, 0.0, -length],
        [-hw, 0.0, -length],
    ];
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uvs = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
    ];
    let indices = vec![0, 2, 1, 0, 3, 2];

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_telegraph(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    cache: &mut TelegraphCache,
    world: &WorldPoint,
    ability: &'static BossAbilityDef,
    source: Entity,
    origin: Vec3,
    dir: Vec3,
) -> Entity {
    let (key, mesh_gen, rot) = match ability.shape {
        TelegraphShape::Circle { radius } => (
            format!("circle_{radius}"),
            circle_mesh(radius),
            Quat::IDENTITY,
        ),
        TelegraphShape::Cone { angle_rad, range } => (
            format!("cone_{angle_rad}_{range}"),
            cone_mesh(angle_rad, range),
            Quat::from_rotation_y(yaw_toward(dir)),
        ),
        TelegraphShape::Line { length, width } => (
            format!("line_{length}_{width}"),
            line_mesh(length, width),
            Quat::from_rotation_y(yaw_toward(dir)),
        ),
    };

    let mesh_handle = cache
        .meshes
        .entry(key)
        .or_insert_with(|| meshes.add(mesh_gen))
        .clone();

    let mat_handle = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.4, 0.1, 0.18),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        depth_bias: 50.0,
        ..default()
    });

    let ground_y = world.terrain_height_under(origin).unwrap_or(origin.y) + 0.08;
    let placed_origin = Vec3::new(origin.x, ground_y, origin.z);

    commands
        .spawn((
            RunEntity,
            Telegraph {
                shape: ability.shape,
                duration: ability.windup,
                elapsed: 0.0,
                damage: ability.damage,
                spell_id: ability.spell_id,
                source,
                origin: placed_origin,
                dir,
                material: mat_handle.clone(),
            },
            Mesh3d(mesh_handle),
            MeshMaterial3d(mat_handle),
            Transform::from_translation(placed_origin).with_rotation(rot),
        ))
        .id()
}

pub(super) fn update_telegraphs(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    world: WorldPoint,
    hero: Query<(Entity, &Transform), (With<Hero>, Without<Telegraph>)>,
    mut telegraphs: Query<(Entity, &mut Transform, &mut Telegraph), Without<Hero>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let hero_data = hero.single().ok();

    for (e, mut tf, mut tele) in &mut telegraphs {
        tele.elapsed += dt;
        let frac = (tele.elapsed / tele.duration).clamp(0.0, 1.0);

        // Animate material color and alpha toward threatening crimson
        let alpha = 0.18 + 0.67 * frac.powf(1.5);
        let g = 0.4 * (1.0 - frac) + 0.05 * frac;
        let b = 0.1 * (1.0 - frac) + 0.02 * frac;
        if let Some(mat) = materials.get_mut(&tele.material) {
            mat.base_color = Color::srgba(1.0, g, b, alpha);
        }

        // Pin to ground height under origin
        if let Some(h) = world.terrain_height_under(tele.origin) {
            tf.translation.y = h + 0.08;
        }

        if tele.elapsed >= tele.duration {
            if let Some((hero_e, htf)) = hero_data {
                let h_pos = htf.translation;
                let h_flat = flat(h_pos);
                let o_flat = flat(tele.origin);
                let hit = match tele.shape {
                    TelegraphShape::Circle { radius } => {
                        h_flat.distance(o_flat) <= radius + HERO_RADIUS
                    }
                    TelegraphShape::Line { length, width } => {
                        let delta = h_flat - o_flat;
                        let fwd = tele.dir.normalize_or_zero();
                        let right = Vec3::new(-fwd.z, 0.0, fwd.x);
                        let proj_fwd = delta.dot(fwd);
                        let proj_right = delta.dot(right).abs();
                        proj_fwd >= -HERO_RADIUS
                            && proj_fwd <= length + HERO_RADIUS
                            && proj_right <= (width * 0.5) + HERO_RADIUS
                    }
                    TelegraphShape::Cone { angle_rad, range } => {
                        let delta = h_flat - o_flat;
                        let dist = delta.length();
                        if dist <= range + HERO_RADIUS {
                            if dist <= 1e-4 {
                                true
                            } else {
                                let dir_to_hero = delta / dist;
                                let cos_angle = dir_to_hero.dot(tele.dir);
                                let half_cos = (angle_rad * 0.5).cos();
                                cos_angle >= half_cos
                            }
                        } else {
                            false
                        }
                    }
                };

                if hit {
                    let taken = hero::hurt_hero(&mut run, tele.damage);
                    fx.impact(hero_e, tele.spell_id);
                    if taken > 0.0 {
                        fx.number(hero_e, taken, false, COLOR_HURT);
                    }
                } else {
                    fx.impact(tele.source, tele.spell_id);
                }
            }
            commands.entity(e).despawn();
        }
    }
}
