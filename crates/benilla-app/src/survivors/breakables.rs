//! World Breakables & Field Power-ups: barrels and crates scattered around the map
//! that drop power-ups when broken.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::enemies::Enemy;
use super::hero::Hero;
use super::units::{self, flat, Fx, COLOR_HEAL, COLOR_SPELL};
use super::{Rng, Run, Selection};

const BREAKABLE_DISPLAYS: [u32; 4] = [1761, 35, 153, 31];
const BREAKABLE_COUNT: usize = 18;
const MIN_SPAWN_DIST: f32 = 30.0;
const MAX_SPAWN_DIST: f32 = 70.0;
const HERO_BREAK_RADIUS: f32 = 1.5;
const POWERUP_PICKUP_RADIUS: f32 = 2.2;

/// A breakable world object (barrel, crate) that can be shattered to drop a power-up.
#[derive(Component)]
pub struct Breakable {
    pub hp: f32,
    pub radius: f32,
}

/// The 5 classic survivors power-up kinds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PowerupKind {
    /// Restores 35% of Hero's max health instantly.
    TurkeyLeg,
    /// Instantly vacuums all XP gems and gold on the map directly to the hero.
    ArcaneMagnet,
    /// Screen-clearing explosion that deals 500 damage to all active enemies within 35 yards.
    GoblinBomb,
    /// Freezes all enemies for 5.0 seconds.
    ChronoHourglass,
    /// Adds +100 gold directly to run.gold.
    GoldCache,
}

impl PowerupKind {
    pub fn display_id(self) -> u32 {
        match self {
            Self::TurkeyLeg => 1205,
            Self::ArcaneMagnet => 1202,
            Self::GoblinBomb => 1204,
            Self::ChronoHourglass => 1203,
            Self::GoldCache => 1201,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::TurkeyLeg => "Turkey Leg",
            Self::ArcaneMagnet => "Arcane Magnet",
            Self::GoblinBomb => "Goblin Bomb",
            Self::ChronoHourglass => "Chrono Hourglass",
            Self::GoldCache => "Gold Cache",
        }
    }
}

/// A floating field power-up waiting to be collected.
#[derive(Component)]
pub struct Powerup {
    pub kind: PowerupKind,
    pub base_y: f32,
    pub bob_t: f32,
}

const ALL_POWERUPS: [PowerupKind; 5] = [
    PowerupKind::TurkeyLeg,
    PowerupKind::ArcaneMagnet,
    PowerupKind::GoblinBomb,
    PowerupKind::ChronoHourglass,
    PowerupKind::GoldCache,
];

/// Spawns 18 breakable barrels/crates scattered around the current map when a run starts.
pub fn spawn_breakables(
    mut commands: Commands,
    selection: Res<Selection>,
    world: WorldPoint,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    existing: Query<&Breakable>,
) {
    if !existing.is_empty() {
        return;
    }
    let center = units::wow_to_bevy(selection.map_def().center);
    for _ in 0..BREAKABLE_COUNT {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let dist = rng.range(MIN_SPAWN_DIST, MAX_SPAWN_DIST);
        let mut pos = center + Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
        if let Some(h) = world.terrain_height_under(pos) {
            pos.y = h;
        } else if let Some(h) = world.terrain_height_under(center) {
            pos.y = h;
        }
        let display = BREAKABLE_DISPLAYS[rng.index(BREAKABLE_DISPLAYS.len())];
        let serial = run.guid();
        let e = units::spawn_object(&mut commands, serial, display, 1.0, pos);
        commands.entity(e).insert(Breakable {
            hp: 1.0,
            radius: 1.0,
        });
    }
}

/// Alias for `spawn_breakables`.
#[allow(dead_code)]
pub fn setup_breakables(
    commands: Commands,
    selection: Res<Selection>,
    world: WorldPoint,
    run: ResMut<Run>,
    rng: ResMut<Rng>,
    existing: Query<&Breakable>,
) {
    spawn_breakables(commands, selection, world, run, rng, existing);
}

/// Spawns a random power-up at the specified location.
pub fn spawn_powerup(
    commands: &mut Commands,
    run: &mut Run,
    rng: &mut Rng,
    pos: Vec3,
) -> Entity {
    let kind = ALL_POWERUPS[rng.index(ALL_POWERUPS.len())];
    let base_y = pos.y + 0.6;
    let mut spawn_pos = pos;
    spawn_pos.y = base_y;
    let serial = run.guid();
    let e = units::spawn_object(commands, serial, kind.display_id(), 1.2, spawn_pos);
    commands.entity(e).insert(Powerup {
        kind,
        base_y,
        bob_t: 0.0,
    });
    e
}

/// Spawns a guaranteed Turkey Leg power-up at the specified location.
pub fn spawn_turkey_leg(
    commands: &mut Commands,
    run: &mut Run,
    pos: Vec3,
) -> Entity {
    let kind = PowerupKind::TurkeyLeg;
    let base_y = pos.y + 0.6;
    let mut spawn_pos = pos;
    spawn_pos.y = base_y;
    let serial = run.guid();
    let e = units::spawn_object(commands, serial, kind.display_id(), 1.2, spawn_pos);
    commands.entity(e).insert(Powerup {
        kind,
        base_y,
        bob_t: 0.0,
    });
    e
}

/// Detects breakable collisions with hero, abilities, and ground areas; destroys barrels and spawns power-ups.
pub(super) fn break_barrels(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    breakables: Query<(Entity, &Transform, &Breakable)>,
    ground_areas: Query<&Transform, With<super::abilities::GroundArea>>,
    mut fx: Fx,
) {
    if run.dead_t.is_some() {
        return;
    }
    let Ok((hero_e, hero_tf)) = hero.single() else {
        return;
    };
    let hero_pos = hero_tf.translation;
    let stats = run.stats();
    let dash_contact = run.iframe_t > 0.0;

    // Maximum ability reach among current hero abilities
    let max_ability_radius = run
        .abilities
        .iter()
        .map(|a| a.radius(&stats))
        .fold(0.0f32, f32::max);

    let mut broken = Vec::new();
    for (barrel_e, barrel_tf, breakable) in &breakables {
        let bpos = barrel_tf.translation;
        let hero_dist = flat(hero_pos - bpos).length();

        let mut should_break = hero_dist <= HERO_BREAK_RADIUS + breakable.radius
            || (dash_contact && hero_dist <= 3.0 + breakable.radius)
            || breakable.hp <= 0.0;

        if !should_break {
            for area_tf in &ground_areas {
                if flat(area_tf.translation - bpos).length() <= 7.0 + breakable.radius {
                    should_break = true;
                    break;
                }
            }
        }

        if !should_break && max_ability_radius > 0.0 {
            if hero_dist <= max_ability_radius + breakable.radius {
                should_break = true;
            }
        }

        if should_break {
            broken.push((barrel_e, bpos));
        }
    }

    for (barrel_e, bpos) in broken {
        fx.cast(hero_e, 1481, &[], Some(bpos));
        commands.entity(barrel_e).despawn();
        spawn_powerup(&mut commands, &mut run, &mut rng, bpos);
    }
}

/// Animates floating field power-ups and collects them on hero contact.
pub(super) fn collect_powerups(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut powerups: Query<(Entity, &mut Transform, &mut Powerup), (Without<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), (Without<Hero>, Without<Powerup>)>,
    mut hits: MessageWriter<Hit>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 {
        return;
    }
    let Ok((hero_e, hero_tf)) = hero.single() else {
        return;
    };
    let hero_pos = hero_tf.translation;
    let stats = run.stats();

    let mut collected = Vec::new();
    for (powerup_e, mut powerup_tf, mut powerup) in &mut powerups {
        powerup.bob_t += dt;
        powerup_tf.translation.y = powerup.base_y + (powerup.bob_t * 3.0).sin() * 0.15;
        powerup_tf.rotate_y(1.5 * dt);

        let dist = flat(hero_pos - powerup_tf.translation).length();
        if dist <= POWERUP_PICKUP_RADIUS {
            collected.push((powerup_e, powerup.kind));
        }
    }

    for (powerup_e, kind) in collected {
        commands.entity(powerup_e).despawn();
        match kind {
            PowerupKind::TurkeyLeg => {
                if run.affixes.midas_curse {
                    let gold_award = 250u64;
                    run.gold += gold_award;
                    fx.number(hero_e, gold_award as f32, false, 0xFFFF_D700);
                    fx.impact(hero_e, 20484);
                    run.banner =
                        Some(("Midas Curse! Turkey converted to +250 Gold!".to_string(), 3.0));
                } else {
                    let heal = stats.max_hp * 0.35;
                    run.hp = (run.hp + heal).min(stats.max_hp);
                    fx.number(hero_e, heal, false, COLOR_HEAL);
                    fx.impact(hero_e, 139);
                    run.banner = Some((format!("{}! Restored 35% Health", kind.name()), 3.0));
                }
            }
            PowerupKind::ArcaneMagnet => {
                run.vacuum_gems = 15.0;
                fx.impact(hero_e, 20484);
                run.banner = Some((format!("{}! Vacuuming all gems & gold!", kind.name()), 3.0));
            }
            PowerupKind::GoblinBomb => {
                let mut hit_count = 0;
                for (enemy_e, enemy_tf, enemy) in &enemies {
                    if enemy.dead {
                        continue;
                    }
                    if flat(enemy_tf.translation - hero_pos).length() <= 35.0 {
                        hits.write(Hit {
                            spell: 11597,
                            key: "goblin_bomb",
                            color: COLOR_SPELL,
                            ..Hit::direct(enemy_e, 500.0, false)
                        });
                        hit_count += 1;
                    }
                }
                fx.cast(hero_e, 2120, &[], Some(hero_pos));
                fx.impact(hero_e, 1481);
                run.banner = Some((format!("{}! 500 Explosion Damage ({} enemies)!", kind.name(), hit_count), 3.0));
            }
            PowerupKind::ChronoHourglass => {
                for (_, _, mut enemy) in &mut enemies {
                    if enemy.dead {
                        continue;
                    }
                    enemy.stun_t = enemy.stun_t.max(5.0);
                    enemy.root_t = enemy.root_t.max(5.0);
                }
                fx.cast(hero_e, 122, &[], Some(hero_pos));
                run.banner = Some((format!("{}! All enemies frozen for 5 seconds!", kind.name()), 3.0));
            }
            PowerupKind::GoldCache => {
                run.gold += 100;
                fx.number(hero_e, 100.0, false, 0xFFFF_D700);
                fx.impact(hero_e, 19740);
                run.banner = Some((format!("{}! +100 Gold", kind.name()), 3.0));
            }
        }
    }
}

/// Vacuums gems and gold directly to the hero when the Arcane Magnet power-up is active.
pub(super) fn vacuum_magnet_gems(
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<&Transform, With<Hero>>,
    mut gems: Query<&mut Transform, (With<super::enemies::Gem>, Without<Hero>)>,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 || run.vacuum_gems <= 0.0 {
        return;
    }
    let Ok(hero_tf) = hero.single() else {
        return;
    };
    let target = hero_tf.translation + Vec3::Y * 1.0;

    run.vacuum_gems = (run.vacuum_gems - dt).max(0.0);
    let mut any_gems = false;
    for mut gem_tf in &mut gems {
        any_gems = true;
        let d = target - gem_tf.translation;
        let dist = d.length();
        if dist > 0.0 {
            gem_tf.translation += d / dist * (45.0 * dt).min(dist);
        }
    }
    if !any_gems && run.vacuum_gems < 14.0 {
        run.vacuum_gems = 0.0;
    }
}
