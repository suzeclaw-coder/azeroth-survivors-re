//! Interactive Map Shrines & Moonwells: standing within 6 yards channels them over 4 seconds,
//! granting a 30-second powerful map-wide buff.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::data::ShrineKind;
use super::enemies::Enemy;
use super::hero::Hero;
use super::units::{self, flat, Fx, COLOR_SPELL};
use super::{Rng, Run, Selection};

const SHRINE_RADIUS: f32 = 6.0;
const CHANNEL_REQUIRED: f32 = 4.0;
const BUFF_DURATION: f32 = 30.0;

/// An interactive shrine on the battleground.
#[derive(Component)]
pub(super) struct Shrine {
    pub kind: ShrineKind,
    pub active: bool,
    pub channel_t: f32,
}

/// Spawns the shrines defined for the selected battleground.
pub(super) fn spawn_shrines(
    mut commands: Commands,
    selection: Res<Selection>,
    world: WorldPoint,
    mut run: ResMut<Run>,
    existing: Query<&Shrine>,
) {
    if !existing.is_empty() {
        return;
    }
    for def in selection.map_def().shrines {
        let (display, scale) = def.kind.display_info();
        let mut pos = units::wow_to_bevy(def.pos);
        if let Some(h) = world.terrain_height_under(pos) {
            pos.y = h;
        }
        let serial = run.guid();
        let e = units::spawn_object(&mut commands, serial, display, scale, pos);
        commands.entity(e).insert(Shrine {
            kind: def.kind,
            active: false,
            channel_t: 0.0,
        });
    }
}

/// Ticks shrine channeling and active shrine buff effects.
#[allow(clippy::type_complexity)]
pub(super) fn tick_shrines(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut shrines: Query<(Entity, &Transform, &mut Shrine)>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 {
        return;
    }
    let Ok((hero_e, hero_tf)) = hero.single() else {
        return;
    };
    let hero_pos = hero_tf.translation;

    // Detect hero proximity to unactivated shrines.
    for (shrine_e, shrine_tf, mut shrine) in &mut shrines {
        if shrine.active {
            continue;
        }
        let dist = flat(shrine_tf.translation - hero_pos).length();
        if dist <= SHRINE_RADIUS {
            let was = shrine.channel_t;
            shrine.channel_t += dt;

            if shrine.channel_t < CHANNEL_REQUIRED {
                // Update channeling progress banner if not showing an activation banner.
                let show_progress = run
                    .banner
                    .as_ref()
                    .map_or(true, |(b, _)| b.starts_with("Channeling"));
                if show_progress {
                    run.banner = Some((
                        format!(
                            "Channeling {}... {:.1}s / {:.0}s",
                            shrine.kind.name(),
                            shrine.channel_t,
                            CHANNEL_REQUIRED
                        ),
                        0.25,
                    ));
                }
                // Periodic channeling sparkle every 1s.
                if was.floor() != shrine.channel_t.floor() {
                    fx.impact(hero_e, 20594);
                }
            } else {
                // Shrine activated!
                shrine.active = true;
                run.shrine_buff = Some((shrine.kind, BUFF_DURATION));
                run.banner = Some((
                    format!(
                        "{} Activated! ({})",
                        shrine.kind.name(),
                        shrine.kind.buff_desc()
                    ),
                    4.0,
                ));
                // Play fanfare
                match shrine.kind {
                    ShrineKind::Moonwell => {
                        fx.impact(hero_e, 139);
                        fx.impact(shrine_e, 20484);
                    }
                    ShrineKind::Swiftness => {
                        fx.impact(hero_e, 2983);
                        fx.impact(shrine_e, 20484);
                    }
                    ShrineKind::Storm => {
                        fx.impact(hero_e, 421);
                        fx.impact(shrine_e, 421);
                    }
                    ShrineKind::Greed => {
                        fx.impact(hero_e, 19740);
                        fx.impact(shrine_e, 20484);
                    }
                }
            }
        } else if shrine.channel_t > 0.0 {
            // Hero stepped out before finishing: reset channel
            shrine.channel_t = 0.0;
        }
    }

    // Ticks active shrine buff
    if let Some((kind, mut t)) = run.shrine_buff {
        let prev_t = t;
        t -= dt;

        let stats = run.stats();

        match kind {
            ShrineKind::Moonwell => {
                run.hp = (run.hp + stats.max_hp * 0.15 * dt).min(stats.max_hp);
            }
            ShrineKind::Swiftness => {
                run.dash_cd = 0.0;
            }
            ShrineKind::Storm => {
                // Every 1.0s, pulse Chain Lightning at up to 4 nearby enemies.
                if prev_t.floor() != t.floor() {
                    let mut targets: Vec<(Entity, Vec3, f32)> = enemies
                        .iter()
                        .filter(|(_, _, en)| !en.dead)
                        .map(|(e, tf, en)| {
                            (
                                e,
                                tf.translation,
                                flat(tf.translation - hero_pos).length() - en.radius,
                            )
                        })
                        .filter(|(_, _, d)| *d <= 18.0)
                        .collect();
                    targets.sort_by(|a, b| a.2.total_cmp(&b.2));
                    targets.truncate(4);

                    if !targets.is_empty() {
                        let hit_entities: Vec<Entity> =
                            targets.iter().map(|(e, _, _)| *e).collect();
                        fx.cast(hero_e, 421, &hit_entities, None);
                        for &(target_e, _, _) in &targets {
                            let base_dmg = 45.0 * stats.might;
                            let (dmg, crit) = if rng.f32() < stats.crit {
                                (base_dmg * 2.0, true)
                            } else {
                                (base_dmg, false)
                            };
                            hits.write(Hit {
                                spell: 421,
                                key: "storm_shrine",
                                color: COLOR_SPELL,
                                ..Hit::direct(target_e, dmg, crit)
                            });
                        }
                    }
                }
            }
            ShrineKind::Greed => {
                // Greed buff effects (4x magnet and 2x growth) are handled via Run::stats().
            }
        }

        run.shrine_buff = if t > 0.0 { Some((kind, t)) } else { None };
    }
}
