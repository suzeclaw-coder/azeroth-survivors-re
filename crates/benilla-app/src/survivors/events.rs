//! Dynamic World Invasions & Timed Events: scheduled mid-run pacing and environmental objectives.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::breakables;
use super::data::{self, EnemyDef};
use super::enemies::{self, GemKind};
use super::hero::Hero;
use super::units::{self, flat, Fx};
use super::{Phase, Rng, Run};

/// Scheduled world event tracking across a run.
#[derive(Resource, Default)]
pub struct WorldEventState {
    pub caravan_spawned: bool,
    pub portal_spawned: bool,
    pub blood_moon_triggered: bool,
    pub blood_moon_t: f32,
    pub bounty_1_spawned: bool,
    pub bounty_2_spawned: bool,
}

/// A moving goblin supply wagon tagged for interception.
#[derive(Component)]
pub struct Caravan {
    pub hp: f32,
    pub speed: f32,
    pub waypoints: Vec<Vec3>,
    pub route_index: usize,
    pub hit_timer: f32,
}

/// A high-value roaming bounty elite with a countdown timer.
#[derive(Component)]
pub struct WantedBounty {
    pub name: &'static str,
    pub time_left: f32,
    pub reward_gold: u64,
    pub hp: f32,
    #[allow(dead_code)]
    pub max_hp: f32,
}

/// A stationary Scourge portal pulsing necrotic rifts and summoning elites.
#[derive(Component)]
pub struct NecroticPortal {
    pub hp: f32,
    pub summon_timer: f32,
    pub rift_timer: f32,
    pub hit_timer: f32,
}

/// Elite Scourge summon spawned by the Necrotic Portal.
static SCOURGE_ELITE: EnemyDef = EnemyDef {
    name: "Scourge Necro-Fiend",
    display: 10626, // Cannibal Ghoul model
    hp: 450.0,
    speed: 5.0,
    damage: 26.0,
    scale: 1.35,
    xp: 35,
    big: false,
    level: 30,
    abilities: &[],
};

/// Ground helper ensuring entities spawn placed on the terrain floor.
fn ground(world: &WorldPoint, mut p: Vec3, fallback: f32) -> Vec3 {
    p.y = world.terrain_height_under(p).unwrap_or(fallback);
    p
}

/// Ticks scheduled mid-run world events, caravan movement/combat, portal summons/combat,
/// and Blood Moon countdown.
#[allow(clippy::type_complexity)]
pub(super) fn tick_world_events(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut state: ResMut<WorldEventState>,
    mut commands: Commands,
    mut rng: ResMut<Rng>,
    world: WorldPoint,
    mut next_phase: ResMut<NextState<Phase>>,
    mut rig: ResMut<super::hero::CamRig>,
    mut fx: Fx,
    mut sounds: MessageWriter<crate::sound::GlueSound>,
    (mut account, mut ach_state): (
        ResMut<super::save::Account>,
        ResMut<super::achievements::AchievementState>,
    ),
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut caravans: Query<
        (Entity, &mut Transform, &mut Caravan),
        (Without<Hero>, Without<NecroticPortal>, Without<WantedBounty>),
    >,
    mut portals: Query<
        (Entity, &Transform, &mut NecroticPortal),
        (Without<Hero>, Without<Caravan>, Without<WantedBounty>),
    >,
    mut bounties: Query<
        (Entity, &mut Transform, &mut WantedBounty),
        (Without<Hero>, Without<Caravan>, Without<NecroticPortal>),
    >,
    ground_areas: Query<
        &Transform,
        (
            With<super::abilities::GroundArea>,
            Without<Hero>,
            Without<Caravan>,
            Without<NecroticPortal>,
            Without<WantedBounty>,
        ),
    >,
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

    // -------------------------------------------------------------------------
    // 1. EVENT SCHEDULE TRIGGERS
    // -------------------------------------------------------------------------

    // 02:00 (120s) — Bounty 1: Foreman Thistleneedle
    if run.elapsed >= 120.0 && !state.bounty_1_spawned {
        state.bounty_1_spawned = true;
        sounds.write(crate::sound::GlueSound("igPVPUpdate"));
        run.banner = Some((
            "WANTED: Slay Foreman Thistleneedle in 60s for +150 Gold & Chest!".to_string(),
            5.0,
        ));
        let offset = Vec3::new(22.0, 0.0, -22.0);
        let bounty_pos = ground(&world, hero_pos + offset, hero_pos.y);
        let serial = run.guid();
        let bounty_e = units::spawn_unit(
            &mut commands,
            &units::UnitSpec {
                serial,
                display: 1157, // Defias Foreman
                scale: 1.4,
                level: 25,
                faction: units::FACTION_MONSTER,
                bytes0: 1 << 8,
                weapons: &[],
                sheath: 0,
            },
            bounty_pos,
            0.0,
        );
        commands.entity(bounty_e).insert(WantedBounty {
            name: "Foreman Thistleneedle",
            time_left: 60.0,
            reward_gold: 150,
            hp: 2400.0,
            max_hp: 2400.0,
        });
        fx.cast(bounty_e, 20594, &[], Some(bounty_pos));
    }

    // 03:30 (210s) — Goblin Supply Caravan
    if run.elapsed >= 210.0 && !state.caravan_spawned {
        state.caravan_spawned = true;
        sounds.write(crate::sound::GlueSound("igBackPackOpen"));
        run.banner = Some((
            "GOBLIN SUPPLY CARAVAN SIGHTED! INTERCEPT IT!".to_string(),
            4.0,
        ));

        // Generate a 6-waypoint patrol route looping around the hero's current area
        let center = hero_pos;
        let waypoints: Vec<Vec3> = (0..6)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 6.0;
                let raw = center + Vec3::new(angle.cos() * 26.0, 0.0, angle.sin() * 26.0);
                ground(&world, raw, center.y)
            })
            .collect();

        let spawn_pos = waypoints[0];
        let serial = run.guid();
        // GameObjectDisplayInfo 153 (Wagon)
        let wagon_e = units::spawn_object(&mut commands, serial, 153, 1.5, spawn_pos);
        commands.entity(wagon_e).insert(Caravan {
            hp: 1200.0,
            speed: 7.0,
            waypoints,
            route_index: 1,
            hit_timer: 0.0,
        });
    }

    // 07:30 (450s) — Scourge Necrotic Portal
    if run.elapsed >= 450.0 && !state.portal_spawned {
        state.portal_spawned = true;
        sounds.write(crate::sound::GlueSound("igCreatureAggroSelect"));
        run.banner = Some(("A NECROTIC PORTAL OPENS! DESTROY IT!".to_string(), 4.0));

        let offset = Vec3::new(16.0, 0.0, 16.0);
        let portal_pos = ground(&world, hero_pos + offset, hero_pos.y);
        let serial = run.guid();
        // GameObjectDisplayInfo 105 (Altar / Dark Rift)
        let portal_e = units::spawn_object(&mut commands, serial, 105, 1.8, portal_pos);
        commands.entity(portal_e).insert(NecroticPortal {
            hp: 3500.0,
            summon_timer: 3.0,
            rift_timer: 0.5,
            hit_timer: 0.0,
        });
        // Initial green dark rift visual (spell 30513)
        fx.cast(portal_e, 30513, &[], Some(portal_pos));
    }

    // 08:30 (510s) — Bounty 2: Old Murk-Eye
    if run.elapsed >= 510.0 && !state.bounty_2_spawned {
        state.bounty_2_spawned = true;
        sounds.write(crate::sound::GlueSound("igPVPUpdate"));
        run.banner = Some((
            "WANTED: Slay Old Murk-Eye in 60s for +250 Gold & Chest!".to_string(),
            5.0,
        ));
        let offset = Vec3::new(-24.0, 0.0, 24.0);
        let bounty_pos = ground(&world, hero_pos + offset, hero_pos.y);
        let serial = run.guid();
        let bounty_e = units::spawn_unit(
            &mut commands,
            &units::UnitSpec {
                serial,
                display: 1024, // Murloc
                scale: 1.5,
                level: 45,
                faction: units::FACTION_MONSTER,
                bytes0: 1 << 8,
                weapons: &[],
                sheath: 0,
            },
            bounty_pos,
            0.0,
        );
        commands.entity(bounty_e).insert(WantedBounty {
            name: "Old Murk-Eye",
            time_left: 60.0,
            reward_gold: 250,
            hp: 4800.0,
            max_hp: 4800.0,
        });
        fx.cast(bounty_e, 20594, &[], Some(bounty_pos));
    }

    // 12:00 (720s) — Blood Moon Eclipse
    if run.elapsed >= 720.0 && !state.blood_moon_triggered {
        state.blood_moon_triggered = true;
        state.blood_moon_t = 45.0;
        sounds.write(crate::sound::GlueSound("igPVPUpdate"));
        run.banner = Some((
            "THE BLOOD MOON RISES! TRIPLE XP & ENRAGED FOES!".to_string(),
            5.0,
        ));
    }

    // -------------------------------------------------------------------------
    // 2. TICK BLOOD MOON
    // -------------------------------------------------------------------------
    if state.blood_moon_t > 0.0 {
        state.blood_moon_t = (state.blood_moon_t - dt).max(0.0);
    }

    // -------------------------------------------------------------------------
    // 3. TICK CARAVANS
    // -------------------------------------------------------------------------
    let mut defeated_caravans = Vec::new();
    for (caravan_e, mut tf, mut caravan) in &mut caravans {
        // Move along waypoints
        if !caravan.waypoints.is_empty() {
            let target = caravan.waypoints[caravan.route_index];
            let delta = flat(target - tf.translation);
            let dist = delta.length();
            if dist < 2.0 {
                caravan.route_index = (caravan.route_index + 1) % caravan.waypoints.len();
            } else {
                let dir = delta / dist;
                tf.translation += dir * caravan.speed * dt;
                tf.translation = ground(&world, tf.translation, tf.translation.y);
                tf.rotation = Quat::from_rotation_y(units::yaw_toward(dir));
            }
        }

        let caravan_pos = tf.translation;
        let hero_dist = flat(hero_pos - caravan_pos).length();

        // Damage calculation from hero proximity, dash contact, and ground areas
        let mut dps = 0.0;
        if hero_dist <= 7.0 + stats.area * 2.0 {
            dps += (150.0 + run.level as f32 * 30.0) * stats.might;
        }
        if run.iframe_t > 0.0 && hero_dist <= 3.5 {
            dps += 400.0 * stats.might;
        }
        for area_tf in &ground_areas {
            if flat(area_tf.translation - caravan_pos).length() <= 6.0 {
                dps += 220.0 * stats.might;
            }
        }

        if dps > 0.0 {
            caravan.hp -= dps * dt;
            caravan.hit_timer -= dt;
            if caravan.hit_timer <= 0.0 {
                caravan.hit_timer = 0.35;
                fx.swing(hero_e, caravan_e, dps * 0.35);
                fx.number(caravan_e, dps * 0.35, false, units::COLOR_PHYS);
            }
        }

        if caravan.hp <= 0.0 {
            defeated_caravans.push((caravan_e, caravan_pos));
        }
    }

    for (caravan_e, cpos) in defeated_caravans {
        fx.cast(hero_e, 1481, &[], Some(cpos));
        rig.shake(1.2, 0.4);
        sounds.write(crate::sound::GlueSound("LOOTWINDOWCOINSOUND"));
        sounds.write(crate::sound::GlueSound("igQuestListOpen"));
        commands.entity(caravan_e).despawn();

        // Drops guaranteed Turkey Leg power-up!
        breakables::spawn_turkey_leg(&mut commands, &mut run, cpos);

        // Drops 300 gold coins (10 coin gems of 30 gold each)
        for i in 0..10 {
            let angle = i as f32 * std::f32::consts::TAU / 10.0;
            let coin_pos = cpos + Vec3::new(angle.cos() * 2.2, 0.0, angle.sin() * 2.2);
            enemies::spawn_gem(
                &mut commands,
                run.guid(),
                coin_pos,
                GemKind::Gold(30),
            );
        }
        run.banner = Some((
            "GOBLIN CARAVAN INTERCEPTED! +300 GOLD & TURKEY LEG!".to_string(),
            4.0,
        ));
        super::achievements::trigger_event_achievement(
            "caravan_raider",
            &mut account,
            &mut ach_state,
            &mut sounds,
        );
    }

    // -------------------------------------------------------------------------
    // 4. TICK NECROTIC PORTALS
    // -------------------------------------------------------------------------
    let mut destroyed_portals = Vec::new();
    for (portal_e, tf, mut portal) in &mut portals {
        let portal_pos = tf.translation;

        // Periodic dark rift visual pulse
        portal.rift_timer -= dt;
        if portal.rift_timer <= 0.0 {
            portal.rift_timer = 2.5;
            fx.impact(portal_e, 30513);
        }

        // Spawn elite Scourge summons every 3.0s until destroyed
        portal.summon_timer -= dt;
        if portal.summon_timer <= 0.0 {
            portal.summon_timer = 3.0;
            let angle = rng.range(0.0, std::f32::consts::TAU);
            let offset = Vec3::new(angle.cos() * 3.5, 0.0, angle.sin() * 3.5);
            let summon_pos = ground(&world, portal_pos + offset, portal_pos.y);
            enemies::spawn_enemy(
                &mut commands,
                &mut run,
                &SCOURGE_ELITE,
                summon_pos,
                hero_pos,
                false,
                (1.8, 1.4, 2.0),
                Some(enemies::ChampionModifier::Vampiric),
            );
            fx.impact(portal_e, 30513);
        }

        // Damage calculation from hero proximity, dash contact, and ground areas
        let hero_dist = flat(hero_pos - portal_pos).length();
        let mut dps = 0.0;
        if hero_dist <= 7.0 + stats.area * 2.0 {
            dps += (160.0 + run.level as f32 * 35.0) * stats.might;
        }
        if run.iframe_t > 0.0 && hero_dist <= 3.5 {
            dps += 450.0 * stats.might;
        }
        for area_tf in &ground_areas {
            if flat(area_tf.translation - portal_pos).length() <= 6.0 {
                dps += 250.0 * stats.might;
            }
        }

        if dps > 0.0 {
            portal.hp -= dps * dt;
            portal.hit_timer -= dt;
            if portal.hit_timer <= 0.0 {
                portal.hit_timer = 0.35;
                fx.impact(portal_e, 1481);
                fx.number(portal_e, dps * 0.35, false, units::COLOR_SPELL);
            }
        }

        if portal.hp <= 0.0 {
            destroyed_portals.push((portal_e, portal_pos));
        }
    }

    for (portal_e, ppos) in destroyed_portals {
        // Holy explosion visual (spell 642)
        fx.cast(hero_e, 642, &[], Some(ppos));
        fx.impact(hero_e, 642);
        rig.shake(1.5, 0.5);
        sounds.write(crate::sound::GlueSound("igPVPUpdate"));
        sounds.write(crate::sound::GlueSound("igQuestListOpen"));
        commands.entity(portal_e).despawn();

        // Grants an instant level-up
        run.level += 1;
        run.pending_levels += 1;
        next_phase.set(Phase::LevelUp);

        // Greed Shrine buff for 30s
        run.shrine_buff = Some((data::ShrineKind::Greed, 30.0));

        run.banner = Some((
            "NECROTIC PORTAL DESTROYED! INSTANT LEVEL UP & GREED BUFF!".to_string(),
            4.0,
        ));
        super::achievements::trigger_event_achievement(
            "portal_cleanser",
            &mut account,
            &mut ach_state,
            &mut sounds,
        );
    }

    // -------------------------------------------------------------------------
    // 5. TICK WANTED BOUNTIES
    // -------------------------------------------------------------------------
    let mut claimed_bounties = Vec::new();
    let mut expired_bounties = Vec::new();
    for (b_e, mut tf, mut bounty) in &mut bounties {
        bounty.time_left -= dt;
        if bounty.time_left <= 0.0 {
            expired_bounties.push((b_e, bounty.name));
            continue;
        }

        // Slight wander movement
        let wander = Vec3::new((run.elapsed * 0.7).cos() * 4.0 * dt, 0.0, (run.elapsed * 0.7).sin() * 4.0 * dt);
        tf.translation += wander;
        tf.translation = ground(&world, tf.translation, tf.translation.y);

        let b_pos = tf.translation;
        let hero_dist = flat(hero_pos - b_pos).length();

        let mut dps = 0.0;
        if hero_dist <= 7.0 + stats.area * 2.0 {
            dps += (200.0 + run.level as f32 * 40.0) * stats.might;
        }
        if run.iframe_t > 0.0 && hero_dist <= 3.5 {
            dps += 500.0 * stats.might;
        }
        for area_tf in &ground_areas {
            if flat(area_tf.translation - b_pos).length() <= 6.0 {
                dps += 250.0 * stats.might;
            }
        }

        if dps > 0.0 {
            bounty.hp -= dps * dt;
            if bounty.hp <= 0.0 {
                claimed_bounties.push((b_e, b_pos, bounty.name, bounty.reward_gold));
            }
        }
    }

    for (b_e, pos, name, gold) in claimed_bounties {
        commands.entity(b_e).despawn();
        super::chests::spawn_chest(&mut commands, pos, true, 2);
        run.gold += gold;
        sounds.write(crate::sound::GlueSound("gsCharacterSelectionEnterWorld"));
        fx.impact(hero_e, 20484);
        run.banner = Some((
            format!("BOUNTY CLAIMED: {} defeated! +{} Gold & Epic Chest!", name, gold),
            4.5,
        ));
    }

    for (b_e, name) in expired_bounties {
        commands.entity(b_e).despawn();
        run.banner = Some((
            format!("The bounty {} escaped into the wild!", name),
            3.0,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_event_state_defaults() {
        let state = WorldEventState::default();
        assert!(!state.caravan_spawned);
        assert!(!state.portal_spawned);
        assert!(!state.blood_moon_triggered);
        assert_eq!(state.blood_moon_t, 0.0);
    }

    #[test]
    fn test_caravan_component() {
        let caravan = Caravan {
            hp: 1200.0,
            speed: 7.0,
            waypoints: vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 10.0)],
            route_index: 0,
            hit_timer: 0.0,
        };
        assert_eq!(caravan.hp, 1200.0);
        assert_eq!(caravan.waypoints.len(), 2);
    }
}
