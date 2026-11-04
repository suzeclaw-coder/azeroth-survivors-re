//! Combat Companion Minions & Engineering Combat Pets.
//!
//! Faithful summons that fight, tank, and assist the hero:
//! - Ironforge Wolf: Leashed beast charging and biting nearby foes.
//! - Fiery Imp: Shoulder-hovering demon firing rapid Firebolts.
//! - Water Elemental: Glacial summon casting periodic Frost Novas.
//! - Clockwork Bombling: Suicide mechanical pet detonating in dense crowds.
//! - Gyrocopter Harvester: Autonomous scout vacuuming remote XP gems.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use crate::sound::GlueSound;
use super::abilities::Hit;
use super::enemies::Enemy;
use super::hero::Hero;
use super::units::{flat, yaw_toward, Fx, UnitSpec, FACTION_FRIENDLY};
use super::Run;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MinionKind {
    HunterWolf,
    WarlockImp,
    WaterElemental,
    ClockworkBombling,
    GyrocopterHarvester,
}

#[derive(Clone, Copy, Debug)]
pub struct MinionDef {
    pub kind: MinionKind,
    pub name: &'static str,
    pub desc: &'static str,
    pub display_id: u32,
    pub scale: f32,
    pub spell: u32,
}

pub static MINIONS: &[MinionDef] = &[
    MinionDef {
        kind: MinionKind::HunterWolf,
        name: "Ironforge Wolf",
        desc: "Loyal hunter beast that chases down and bites nearby foes for 45 physical damage.",
        display_id: 1109,
        scale: 0.9,
        spell: 19574,
    },
    MinionDef {
        kind: MinionKind::WarlockImp,
        name: "Fiery Imp",
        desc: "Hovers at your shoulder firing continuous Firebolts for 60 fire damage.",
        display_id: 4449,
        scale: 0.65,
        spell: 133,
    },
    MinionDef {
        kind: MinionKind::WaterElemental,
        name: "Water Elemental",
        desc: "Glacial guardian casting radial Frost Novas freezing enemies within 6.5 yards.",
        display_id: 1210,
        scale: 1.1,
        spell: 122,
    },
    MinionDef {
        kind: MinionKind::ClockworkBombling,
        name: "Clockwork Bombling",
        desc: "Charges dense enemy packs and detonates for 320 fire damage in a 5.5yd radius.",
        display_id: 7565,
        scale: 0.8,
        spell: 4072,
    },
    MinionDef {
        kind: MinionKind::GyrocopterHarvester,
        name: "Gyrocopter Harvester",
        desc: "Autonomous aerial scout orbiting the arena and vacuuming remote XP gems to the hero.",
        display_id: 105,
        scale: 0.85,
        spell: 1459,
    },
];

pub fn find_minion(kind: MinionKind) -> &'static MinionDef {
    MINIONS.iter().find(|m| m.kind == kind).expect("minion def exists")
}

#[derive(Component)]
pub struct Minion {
    pub kind: MinionKind,
    pub timer: f32,
    pub attack_cd: f32,
    pub orbit_angle: f32,
}

/// Spawns a minion entity in the world.
pub fn spawn_minion(
    commands: &mut Commands,
    serial: u64,
    kind: MinionKind,
    pos: Vec3,
) -> Entity {
    let def = find_minion(kind);
    let spec = UnitSpec {
        serial,
        display: def.display_id,
        scale: def.scale,
        level: 1,
        faction: FACTION_FRIENDLY,
        bytes0: 0,
        weapons: &[],
        sheath: 0,
    };

    let e = super::units::spawn_unit(commands, &spec, pos, 0.0);
    commands.entity(e).insert(Minion {
        kind,
        timer: 0.0,
        attack_cd: 0.5,
        orbit_angle: 0.0,
    });
    e
}

/// Ticks minion movement, AI targeting, and spell casts.
pub(super) fn tick_minions(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    world: WorldPoint,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut minions: Query<(Entity, &mut Transform, &mut Minion), Without<Hero>>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Hero>, Without<Minion>)>,
    mut hits: MessageWriter<Hit>,
    mut fx: Fx,
    mut sounds: MessageWriter<GlueSound>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || run.dead_t.is_some() {
        return;
    }

    let Ok((_hero_e, hero_tf)) = hero.single() else { return };
    let hpos = hero_tf.translation;

    // Spawn any newly picked/drafted companions
    if !run.pending_minions.is_empty() {
        let pending = std::mem::take(&mut run.pending_minions);
        for kind in pending {
            let serial = run.guid();
            spawn_minion(&mut commands, serial, kind, hpos + Vec3::new(1.0, 0.0, 1.0));
        }
    }

    for (me, mut mtf, mut minion) in &mut minions {
        minion.timer += dt;
        minion.attack_cd -= dt;

        match minion.kind {
            MinionKind::HunterWolf => {
                // Seek nearest enemy within 12 yards, or stay near hero
                let mut closest = None;
                let mut min_d = 12.0 * 12.0;
                for (ee, etf, enemy) in &enemies {
                    if enemy.dead { continue; }
                    let d = flat(etf.translation - mtf.translation).length_squared();
                    if d < min_d {
                        min_d = d;
                        closest = Some((ee, etf.translation));
                    }
                }

                if let Some((target_e, target_pos)) = closest {
                    // Chase and bite
                    let to_target = flat(target_pos - mtf.translation);
                    let dist = to_target.length();
                    if dist > 1.4 {
                        let step = to_target.normalize() * (7.5 * dt);
                        mtf.translation += step;
                    }
                    mtf.rotation = Quat::from_rotation_y(yaw_toward(to_target));

                    if dist <= 1.8 && minion.attack_cd <= 0.0 {
                        minion.attack_cd = 1.2;
                        let might = run.stats().might;
                        hits.write(Hit::direct(target_e, 45.0 * might, false));
                        fx.swing(me, target_e, 45.0 * might);
                        sounds.write(GlueSound("igCreatureAggroSelect"));
                    }
                } else {
                    // Return towards hero leash point (leash radius ~2.5 yds)
                    let leash_target = hpos + Vec3::new(-1.8, 0.0, 1.2);
                    let to_leash = flat(leash_target - mtf.translation);
                    if to_leash.length() > 0.5 {
                        mtf.translation += to_leash.normalize() * (7.0 * dt);
                        mtf.rotation = Quat::from_rotation_y(yaw_toward(to_leash));
                    }
                }
            }

            MinionKind::WarlockImp => {
                // Orbit at hero's shoulder (radius ~1.5)
                minion.orbit_angle += 1.5 * dt;
                let desired = hpos + Vec3::new(minion.orbit_angle.cos() * 1.6, 0.6, minion.orbit_angle.sin() * 1.6);
                mtf.translation = desired;

                // Firebolt nearest enemy within 16 yards
                if minion.attack_cd <= 0.0 {
                    let mut closest = None;
                    let mut min_d = 16.0 * 16.0;
                    for (ee, etf, enemy) in &enemies {
                        if enemy.dead { continue; }
                        let d = flat(etf.translation - mtf.translation).length_squared();
                        if d < min_d {
                            min_d = d;
                            closest = Some((ee, etf.translation));
                        }
                    }

                    if let Some((target_e, target_pos)) = closest {
                        minion.attack_cd = 1.5;
                        mtf.rotation = Quat::from_rotation_y(yaw_toward(target_pos - mtf.translation));
                        let might = run.stats().might;
                        hits.write(Hit::direct(target_e, 60.0 * might, false));
                        fx.cast(me, 133, &[target_e], None);
                    }
                }
            }

            MinionKind::WaterElemental => {
                // Follow hero at radius 2.2
                let leash_target = hpos + Vec3::new(1.8, 0.0, -1.5);
                let to_leash = flat(leash_target - mtf.translation);
                if to_leash.length() > 0.4 {
                    mtf.translation += to_leash.normalize() * (6.5 * dt);
                    mtf.rotation = Quat::from_rotation_y(yaw_toward(to_leash));
                }

                // Cast Frost Nova every 10 seconds
                if minion.attack_cd <= 0.0 {
                    minion.attack_cd = 10.0;
                    fx.impact(me, 122);
                    let might = run.stats().might;
                    for (ee, etf, enemy) in &enemies {
                        if enemy.dead { continue; }
                        if flat(etf.translation - mtf.translation).length() <= 6.5 {
                            hits.write(Hit::direct(ee, 40.0 * might, false));
                        }
                    }
                    sounds.write(GlueSound("igQuestListOpen"));
                }
            }

            MinionKind::ClockworkBombling => {
                // Seek nearest cluster of enemies
                let mut target_pos = None;
                let mut min_d = 18.0 * 18.0;
                for (_, etf, enemy) in &enemies {
                    if enemy.dead { continue; }
                    let d = flat(etf.translation - mtf.translation).length_squared();
                    if d < min_d {
                        min_d = d;
                        target_pos = Some(etf.translation);
                    }
                }

                if let Some(pos) = target_pos {
                    let to_t = flat(pos - mtf.translation);
                    let dist = to_t.length();
                    if dist > 1.2 {
                        mtf.translation += to_t.normalize() * (8.5 * dt);
                        mtf.rotation = Quat::from_rotation_y(yaw_toward(to_t));
                    } else {
                        // Detonate!
                        fx.impact(me, 4072);
                        sounds.write(GlueSound("igPVPUpdate"));
                        let might = run.stats().might;
                        for (ee, etf, enemy) in &enemies {
                            if enemy.dead { continue; }
                            if flat(etf.translation - mtf.translation).length() <= 5.5 {
                                hits.write(Hit::direct(ee, 320.0 * might, false));
                            }
                        }
                        // Reset bombling near hero after detonation
                        mtf.translation = hpos + Vec3::new(-2.0, 0.0, -2.0);
                        minion.attack_cd = 12.0;
                    }
                } else {
                    // Orbit hero while waiting
                    let to_h = flat(hpos - mtf.translation);
                    if to_h.length() > 3.0 {
                        mtf.translation += to_h.normalize() * (6.0 * dt);
                    }
                }
            }

            MinionKind::GyrocopterHarvester => {
                // Orbit hero in wide perimeter (radius ~10 yds)
                minion.orbit_angle += 1.2 * dt;
                let r = 9.0;
                let target = hpos + Vec3::new(minion.orbit_angle.cos() * r, 1.8, minion.orbit_angle.sin() * r);
                mtf.translation = target;
                mtf.rotation = Quat::from_rotation_y(minion.orbit_angle + std::f32::consts::FRAC_PI_2);

                // Magnetize gems periodically
                if minion.attack_cd <= 0.0 {
                    minion.attack_cd = 2.0;
                    run.vacuum_gems = 1.0;
                }
            }
        }

        // Clamp to ground height (except flying harvester & hovering imp)
        if minion.kind != MinionKind::GyrocopterHarvester && minion.kind != MinionKind::WarlockImp {
            if let Some(gy) = world.terrain_height_under(mtf.translation) {
                mtf.translation.y = gy;
            }
        }
    }
}

/// Spawns starting class companions if applicable.
pub(super) fn setup_starting_companions(
    mut commands: Commands,
    mut run: ResMut<Run>,
    hero: Query<&Transform, With<Hero>>,
) {
    let Ok(htf) = hero.single() else { return };
    let hpos = htf.translation;

    match run.class.id {
        3 => {
            // Hunter starts with Wolf companion
            let serial = run.guid();
            spawn_minion(&mut commands, serial, MinionKind::HunterWolf, hpos + Vec3::new(-1.8, 0.0, 1.2));
        }
        9 => {
            // Warlock starts with Imp companion
            let serial = run.guid();
            spawn_minion(&mut commands, serial, MinionKind::WarlockImp, hpos + Vec3::new(1.4, 0.6, 1.4));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_five_minions_defined() {
        assert_eq!(MINIONS.len(), 5);
        for m in MINIONS {
            assert!(!m.name.is_empty());
            assert!(!m.desc.is_empty());
            assert!(m.display_id > 0);
        }
    }
}
