//! Elite Treasure Chests & Multi-Card Roulette.
//!
//! Spawns interactive 3D Treasure Chests on boss defeats that reward gold bonuses
//! and random ability/passive upgrades (similar to level up choices).

use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;

use super::hero::Hero;
use super::hud::{self, Choice};
use super::units::{self, flat, Fx};
use super::{Owned, Rng, Run, RunEntity};

/// Classic WoW Treasure Chest GameObject display ID.
const CHEST_DISPLAY_ID: u32 = 259;

/// An interactive 3D Treasure Chest dropped by bosses.
#[derive(Component, Debug, Clone)]
pub struct Chest {
    pub opened: bool,
    pub gold: u32,
    pub card_count: u8,
    pub bob_t: f32,
}

/// Spawns an interactive 3D Treasure Chest at `pos`.
///
/// Standard boss drops 1 or 3 cards (80% 1, 18% 3, 2% 5, boosted by Fortune upgrade).
/// Major 10m/15m boss drops 3 or 5 cards (65% 3, 35% 5, boosted by Fortune upgrade).
/// Gold bonus: 50-250 gold.
pub fn spawn_chest(
    commands: &mut Commands,
    pos: Vec3,
    is_major_boss: bool,
    fortune_rank: u8,
) -> Entity {
    let mut rng = Rng::default();
    let luck = (fortune_rank as f32) * 0.05;
    let card_count = if is_major_boss {
        let five_chance = (0.35 + luck * 0.5).min(0.70);
        if rng.f32() < five_chance {
            5
        } else {
            3
        }
    } else {
        let r = rng.f32();
        let one_threshold = (0.80 - luck).max(0.40);
        let three_threshold = (0.98 - luck * 0.5).max(0.70);
        if r < one_threshold {
            1
        } else if r < three_threshold {
            3
        } else {
            5
        }
    };
    let gold = rng.range(50.0, 250.0) as u32;

    static NEXT_CHEST_SERIAL: AtomicU64 = AtomicU64::new(0x7000_0000);
    let serial = NEXT_CHEST_SERIAL.fetch_add(1, Ordering::Relaxed);

    let scale = if is_major_boss { 1.8 } else { 1.4 };
    let e = units::spawn_object(commands, serial, CHEST_DISPLAY_ID, scale, pos);
    commands.entity(e).insert((
        RunEntity,
        Chest {
            opened: false,
            gold,
            card_count,
            bob_t: 0.0,
        },
    ));
    e
}

/// Applies a single upgrade to `run`, or drains pending levels / awards XP if pool is maxed.
fn apply_upgrade_step(run: &mut Run, rng: &mut Rng) {
    let choices = hud::choices_for(run, rng);
    let available: Vec<Choice> = choices
        .into_iter()
        .filter(|c| !matches!(c, Choice::Heal | Choice::Gold(_) | Choice::UltimateSurge))
        .collect();

    if !available.is_empty() {
        let choice = available[rng.index(available.len())];
        match choice {
            Choice::NewAbility(def) => {
                run.abilities.push(Owned {
                    def,
                    level: 1,
                    cd: 0.3,
                });
            }
            Choice::Upgrade(k) => {
                if let Some(o) = run.abilities.get_mut(k) {
                    o.level += 1;
                }
            }
            Choice::NewPassive(p) => {
                run.passives.push((p, 1));
            }
            Choice::UpPassive(k) => {
                if let Some(entry) = run.passives.get_mut(k) {
                    entry.1 += 1;
                }
            }
            Choice::Evolution(evo) => {
                run.evolved.push(evo.evolved_name);
            }
            Choice::Specialization(_) => {}
            Choice::Minion(m) => {
                if !run.minions.contains(&m.kind) {
                    run.minions.push(m.kind);
                    run.pending_minions.push(m.kind);
                }
            }
            Choice::Heal | Choice::Gold(_) | Choice::UltimateSurge => unreachable!(),
        }
        run.version += 1;
    } else {
        // Pool is maxed out: drain pending levels and award bonus XP / health
        if run.pending_levels > 0 {
            run.pending_levels = run.pending_levels.saturating_sub(1);
        }
        run.xp += 150.0;
        run.hp = (run.hp + 50.0).min(run.stats().max_hp);
        run.version += 1;
    }
}

/// Ticks active chests: sparkles while unopened, detects hero proximity to open,
/// rewards cards/gold/fanfare, and despawns after 0.5s.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tick_chests(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut chests: Query<(Entity, &Transform, &mut Chest), Without<Hero>>,
    mut rig: ResMut<super::hero::CamRig>,
    mut fx: Fx,
    mut sounds: MessageWriter<crate::sound::GlueSound>,
    account: Option<Res<super::save::Account>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let hero_info = if run.dead_t.is_none() {
        hero.single().ok()
    } else {
        None
    };

    for (chest_e, chest_tf, mut chest) in &mut chests {
        if chest.opened {
            chest.bob_t += dt;
            if chest.bob_t >= 0.5 {
                commands.entity(chest_e).despawn();
            }
            continue;
        }

        // Periodic sparkle beacon while waiting
        let was = chest.bob_t;
        chest.bob_t += dt;
        if was.floor() != chest.bob_t.floor() {
            fx.impact(chest_e, 20484);
        }

        let Some((hero_e, hero_tf)) = hero_info else {
            continue;
        };

        let dist = flat(chest_tf.translation - hero_tf.translation).length();
        let auto_loot = account.as_deref().map(|a| a.auto_loot_chests).unwrap_or(true);
        let interact_pressed = keys.as_ref().map(|k| k.just_pressed(KeyCode::KeyF) || k.just_pressed(KeyCode::KeyE)).unwrap_or(false);
        if dist <= 2.5 && (auto_loot || interact_pressed) {
            chest.opened = true;
            chest.bob_t = 0.0;

            // Celebratory fanfare VFX (spell 642 or 1481) via Fx
            fx.impact(hero_e, 1481);
            fx.impact(chest_e, 642);
            rig.shake(0.8, 0.3);
            sounds.write(crate::sound::GlueSound("igQuestListOpen"));
            sounds.write(crate::sound::GlueSound("LOOTWINDOWCOINSOUND"));

            // Add gold directly to run.gold & show floating gold combat text
            run.gold += chest.gold as u64;
            fx.number(hero_e, chest.gold as f32, false, 0xFFFF_D700);

            // Reward card_count upgrades
            for _ in 0..chest.card_count {
                apply_upgrade_step(&mut run, &mut rng);
            }

            // Chance to discover a legendary artifact trinket
            if (chest.card_count >= 3 || rng.f32() < 0.40) && run.trinkets.len() < super::trinkets::MAX_TRINKETS {
                if let Some(trinket) = super::trinkets::roll_random_unowned(&run, &mut rng) {
                    super::trinkets::award_trinket(&mut run, trinket, &mut sounds);
                }
            }

            let card_text = if chest.card_count == 1 {
                "1 Upgrade".to_string()
            } else {
                format!("{} Upgrades", chest.card_count)
            };
            run.banner = Some((
                format!(
                    "TREASURE CHEST OPENED! +{} Gold & {}!",
                    chest.gold, card_text
                ),
                3.5,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chest_component() {
        let chest = Chest {
            opened: false,
            gold: 150,
            card_count: 3,
            bob_t: 0.0,
        };
        assert!(!chest.opened);
        assert_eq!(chest.gold, 150);
        assert_eq!(chest.card_count, 3);
        assert_eq!(chest.bob_t, 0.0);
    }

    #[test]
    fn test_apply_upgrade_step_on_fresh_run() {
        let mut run = Run::default();
        let mut rng = Rng::default();
        let initial_abilities = run.abilities.len();
        let initial_passives = run.passives.len();
        let initial_minions = run.minions.len();

        apply_upgrade_step(&mut run, &mut rng);

        // Either an ability, passive, or minion companion was added
        assert!(
            run.abilities.len() > initial_abilities
                || run.passives.len() > initial_passives
                || run.minions.len() > initial_minions
        );
        assert!(run.version > 0);
    }
}
