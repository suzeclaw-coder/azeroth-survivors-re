//! Dynamic Arena Weather Hazards & Environmental Disasters.
//!
//! Zone-specific hazards that dynamically alter combat arenas:
//! - Fire Plume Ridge: Lava Geyser eruptions (red telegraph warning -> fire eruption).
//! - The Barrens & Ahn'Qiraj: Desert Dust Devils (swirling tornadoes knocking units back).
//! - Naxxramas: Creeping Plague Spore clouds (area denial toxic mist).
//! - Mount Hyjal: Sacred Blooms (blessed flora restoring health and granting haste).

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use crate::sound::GlueSound;
use super::abilities::Hit;
use super::data::{self, BossAbilityDef, TelegraphShape};
use super::hero::{self, Hero, HERO_RADIUS};
use super::telegraphs::{self, TelegraphCache};
use super::units::{self, flat, Fx};
use super::{Rng, Run, Selection};

#[derive(Resource, Default)]
pub struct HazardState {
    pub timer: f32,
    pub secondary_timer: f32,
}

#[derive(Component)]
pub enum HazardKind {
    LavaGeyser {
        origin: Vec3,
        radius: f32,
        delay: f32,
        elapsed: f32,
        damage: f32,
    },
    DustDevil {
        pos: Vec3,
        dir: Vec3,
        life: f32,
    },
    PlagueCloud {
        pos: Vec3,
        dir: Vec3,
        life: f32,
    },
    SacredBloom {
        pos: Vec3,
        life: f32,
    },
}

static GEYSER_ABILITY: BossAbilityDef = BossAbilityDef {
    name: "Lava Geyser",
    shape: TelegraphShape::Circle { radius: 3.2 },
    windup: 1.6,
    damage: 48.0,
    spell_id: 11990,
    cooldown: 8.0,
};

pub(super) fn tick_hazards(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut state: ResMut<HazardState>,
    selection: Res<Selection>,
    world: WorldPoint,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut hazards: Query<(Entity, &mut Transform, &mut HazardKind), Without<Hero>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<TelegraphCache>,
    mut fx: Fx,
    mut sfx: MessageWriter<GlueSound>,
    _hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || run.dead_t.is_some() {
        return;
    }
    let Ok((hero_e, htf)) = hero.single() else { return };
    let hpos = htf.translation;
    let zone_idx = selection.map;

    state.timer -= dt;
    state.secondary_timer -= dt;

    // 1. SPAWN HAZARDS ACCORDING TO ZONE
    // Map 3: Fire Plume Ridge -> Lava Geysers
    if zone_idx == 3 && state.timer <= 0.0 {
        state.timer = rng.range(7.0, 10.0);
        let count = rng.range(2.0, 4.0) as usize;
        for _ in 0..count {
            let offset_a = rng.range(0.0, std::f32::consts::TAU);
            let offset_r = rng.range(4.0, 16.0);
            let spot = hpos + Vec3::new(offset_a.cos() * offset_r, 0.0, offset_a.sin() * offset_r);
            let ground_y = world.terrain_height_under(spot).unwrap_or(spot.y);
            let origin = Vec3::new(spot.x, ground_y, spot.z);

            telegraphs::spawn_telegraph(
                &mut commands,
                &mut meshes,
                &mut materials,
                &mut cache,
                &world,
                &GEYSER_ABILITY,
                hero_e,
                origin,
                Vec3::Y,
            );
            fx.impact(hero_e, 11990);

            commands.spawn((
                Transform::from_translation(origin),
                HazardKind::LavaGeyser {
                    origin,
                    radius: 3.2,
                    delay: 1.6,
                    elapsed: 0.0,
                    damage: 48.0,
                },
            ));
        }
    }

    // Map 0 (The Barrens) or Map 2 (Gates of Ahn'Qiraj) -> Desert Dust Devils
    if (zone_idx == 0 || zone_idx == 2) && state.secondary_timer <= 0.0 {
        state.secondary_timer = 22.0;
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let pos = hpos + Vec3::new(angle.cos() * 24.0, 0.0, angle.sin() * 24.0);
        let dir = (hpos - pos).normalize_or_zero();
        let serial = run.guid();
        let e = units::spawn_object(&mut commands, serial, 105, 1.4, pos);
        commands.entity(e).insert(HazardKind::DustDevil {
            pos,
            dir,
            life: 16.0,
        });
        run.banner = Some(("A desert dust devil approaches!".to_string(), 3.0));
    }

    // Map 15: Naxxramas -> Creeping Plague Spores
    if zone_idx == 15 && state.timer <= 0.0 {
        state.timer = 25.0;
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let pos = hpos + Vec3::new(angle.cos() * 22.0, 0.0, angle.sin() * 22.0);
        let dir = (hpos - pos).normalize_or_zero();
        let serial = run.guid();
        let e = units::spawn_object(&mut commands, serial, 105, 2.0, pos);
        commands.entity(e).insert(HazardKind::PlagueCloud {
            pos,
            dir,
            life: 18.0,
        });
        run.banner = Some(("Creeping plague spores drift across the field!".to_string(), 3.0));
    }

    // Map 14: Mount Hyjal -> Sacred Blooms
    if zone_idx == 14 && state.timer <= 0.0 {
        state.timer = 35.0;
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(6.0, 18.0);
        let pos = hpos + Vec3::new(angle.cos() * r, 0.0, angle.sin() * r);
        let serial = run.guid();
        let e = units::spawn_object(&mut commands, serial, 259, 1.2, pos);
        commands.entity(e).insert(HazardKind::SacredBloom { pos, life: 25.0 });
        run.banner = Some(("A Sacred Bloom has sprouted from Nordrassil!".to_string(), 3.5));
        sfx.write(GlueSound("igQuestListOpen"));
    }

    // 2. TICK EXISTING HAZARDS
    for (e, mut tf, mut hazard) in &mut hazards {
        match &mut *hazard {
            HazardKind::LavaGeyser {
                origin,
                radius,
                delay,
                elapsed,
                damage,
            } => {
                *elapsed += dt;
                if *elapsed >= *delay {
                    let dist = flat(hpos - *origin).length();
                    if dist <= *radius {
                        let taken = hero::hurt_hero(&mut run, *damage);
                        if taken > 0.0 {
                            fx.number(hero_e, taken, false, 0xFFFF_4500);
                        }
                    }
                    commands.entity(e).despawn();
                }
            }
            HazardKind::DustDevil { pos, dir, life } => {
                *life -= dt;
                if *life <= 0.0 {
                    commands.entity(e).despawn();
                    continue;
                }
                *pos += *dir * (5.0 * dt);
                if let Some(gy) = world.terrain_height_under(*pos) {
                    pos.y = gy;
                }
                tf.translation = *pos;

                let dist = flat(hpos - *pos).length();
                if dist <= 3.5 {
                    let taken = hero::hurt_hero(&mut run, 18.0 * dt);
                    if taken > 0.0 {
                        fx.number(hero_e, taken, false, units::COLOR_HURT);
                    }
                }
            }
            HazardKind::PlagueCloud { pos, dir, life } => {
                *life -= dt;
                if *life <= 0.0 {
                    commands.entity(e).despawn();
                    continue;
                }
                *pos += *dir * (3.5 * dt);
                if let Some(gy) = world.terrain_height_under(*pos) {
                    pos.y = gy;
                }
                tf.translation = *pos;

                let dist = flat(hpos - *pos).length();
                if dist <= 4.5 {
                    let taken = hero::hurt_hero(&mut run, 12.0 * dt);
                    if taken > 0.0 {
                        fx.number(hero_e, taken, false, 0xFF00_FF66);
                    }
                }
            }
            HazardKind::SacredBloom { pos, life } => {
                *life -= dt;
                if *life <= 0.0 {
                    commands.entity(e).despawn();
                    continue;
                }
                let dist = flat(hpos - *pos).length();
                if dist <= HERO_RADIUS + 1.2 {
                    // Bloom collected!
                    commands.entity(e).despawn();
                    let stats = run.stats();
                    run.hp = (run.hp + 40.0).min(stats.max_hp);
                    run.shrine_buff = Some((data::ShrineKind::Swiftness, 12.0));
                    fx.impact(hero_e, 26573);
                    sfx.write(GlueSound("LOOTWINDOWCOINSOUND"));
                    run.banner = Some(("Sacred Bloom collected! +40 HP & Swiftness!".to_string(), 3.0));
                }
            }
        }
    }
}
