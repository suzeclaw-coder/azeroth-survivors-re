//! Racial Traits & Starting Asymmetry for Warcraft Survivors.
//!
//! Provides unique passive and periodic active racial traits for all 8 playable races:
//! - Human (1): *The Human Spirit* (+15% XP gems, +10% bonus gold)
//! - Orc (2): *Blood Fury* (+10% Might, +25% stun/freeze resistance)
//! - Dwarf (3): *Stoneform* (+15% max HP and +2.0 HP/s baseline health regen)
//! - Night Elf (4): *Quickness* (+10% dodge/evasion chance to take 0 damage, +5% move speed)
//! - Undead (5): *Will of the Forsaken* (10% passive lifesteal on bleeding or burning targets)
//! - Tauren (6): *War Stomp* (Passive radial stomp every 15.0s stunning nearby enemies within 6 yards for 2.0s)
//! - Gnome (7): *Engineering Tinkering* (+3.0 yd base magnet radius, +10% crit chance)
//! - Troll (8): *Berserking* (Gain up to +35% haste as current HP drops below 50%)

use bevy::prelude::*;

use super::enemies::Enemy;
use super::fct::{CombatTextKind, CombatTextMsg};
use super::hero::Hero;
use super::units::Fx;
use super::{Run, Stats};

/// Racial trait for playable races.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RaceTrait {
    TheHumanSpirit,       // Human (1)
    BloodFury,            // Orc (2)
    Stoneform,            // Dwarf (3)
    Quickness,            // Night Elf (4)
    WillOfTheForsaken,    // Undead (5)
    WarStomp,             // Tauren (6)
    EngineeringTinkering, // Gnome (7)
    Berserking,           // Troll (8)
    #[default]
    None,
}

impl RaceTrait {
    /// The WoW race id corresponding to this trait (1..=8, or 0 for none).
    #[allow(dead_code)]
    pub fn race_id(&self) -> u8 {
        match self {
            Self::TheHumanSpirit => 1,
            Self::BloodFury => 2,
            Self::Stoneform => 3,
            Self::Quickness => 4,
            Self::WillOfTheForsaken => 5,
            Self::WarStomp => 6,
            Self::EngineeringTinkering => 7,
            Self::Berserking => 8,
            Self::None => 0,
        }
    }

    /// Title of the racial trait.
    pub fn name(&self) -> &'static str {
        match self {
            Self::TheHumanSpirit => "The Human Spirit",
            Self::BloodFury => "Blood Fury",
            Self::Stoneform => "Stoneform",
            Self::Quickness => "Quickness",
            Self::WillOfTheForsaken => "Will of the Forsaken",
            Self::WarStomp => "War Stomp",
            Self::EngineeringTinkering => "Engineering Tinkering",
            Self::Berserking => "Berserking",
            Self::None => "None",
        }
    }

    /// Short summary of the trait's effects.
    pub fn description(&self) -> &'static str {
        match self {
            Self::TheHumanSpirit => "+15% XP gems, +10% bonus gold",
            Self::BloodFury => "+10% Might, +25% stun/freeze resistance",
            Self::Stoneform => "+15% max HP and +2.0 HP/s baseline health regen",
            Self::Quickness => "+10% dodge chance, +5% move speed",
            Self::WillOfTheForsaken => "10% passive lifesteal on bleeding or burning targets",
            Self::WarStomp => "Passive radial stomp every 15.0s stunning nearby enemies within 6 yards for 2.0s",
            Self::EngineeringTinkering => "+3.0 yd base magnet radius, +10% crit chance",
            Self::Berserking => "Gain up to +35% haste as current HP drops below 50%",
            Self::None => "",
        }
    }

    /// Applies the passive stat adjustments for this race trait.
    pub fn apply_stats(&self, stats: &mut Stats, run: &Run) {
        match self {
            Self::TheHumanSpirit => {
                stats.xp_bonus *= 1.15;
                stats.gold_bonus *= 1.10;
            }
            Self::BloodFury => {
                stats.might += 0.10;
                stats.stun_res += 0.25;
            }
            Self::Stoneform => {
                stats.max_hp *= 1.15;
                stats.regen += 2.0;
            }
            Self::Quickness => {
                stats.evasion += 0.10;
                stats.speed *= 1.05;
            }
            Self::WillOfTheForsaken => {
                // Handled via periodic siphoning in `tick_racial_traits` and on-hit drain in `apply_hits`
            }
            Self::WarStomp => {
                // Periodic active shockwave handled in `tick_racial_traits`
            }
            Self::EngineeringTinkering => {
                stats.magnet += 3.0;
                stats.crit += 0.10;
            }
            Self::Berserking => {
                let hp_ratio = (run.hp / stats.max_hp).clamp(0.0, 1.0);
                if hp_ratio < 0.5 {
                    let missing = (0.5 - hp_ratio) / 0.5;
                    stats.haste += 0.35 * missing;
                }
            }
            Self::None => {}
        }
    }
}

/// Helper function to resolve a `RaceTrait` from a WoW race id.
pub fn race_trait(race_id: u8) -> RaceTrait {
    match race_id {
        1 => RaceTrait::TheHumanSpirit,
        2 => RaceTrait::BloodFury,
        3 => RaceTrait::Stoneform,
        4 => RaceTrait::Quickness,
        5 => RaceTrait::WillOfTheForsaken,
        6 => RaceTrait::WarStomp,
        7 => RaceTrait::EngineeringTinkering,
        8 => RaceTrait::Berserking,
        _ => RaceTrait::None,
    }
}

/// Helper function to resolve a `RaceTrait` from an optional `(race, sex)` look tuple.
#[allow(dead_code)]
pub fn race_for_look(look: Option<(u8, u8)>) -> RaceTrait {
    look.map(|(race, _)| race_trait(race)).unwrap_or_default()
}

/// Cross-disciplinary synergy bonus between specific race and class combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassRaceSynergy {
    pub race_id: u8,
    pub class_id: u8,
    pub name: &'static str,
    pub description: &'static str,
}

pub const SYNERGIES: [ClassRaceSynergy; 5] = [
    ClassRaceSynergy {
        race_id: 2,  // Orc
        class_id: 1, // Warrior
        name: "Blood Fury Avatar",
        description: "+200% Avatar damage bonus",
    },
    ClassRaceSynergy {
        race_id: 5,  // Undead
        class_id: 9, // Warlock
        name: "Soul Siphon Aura",
        description: "Metamorphosis immolation burns heal 2% HP/s",
    },
    ClassRaceSynergy {
        race_id: 4,  // Night Elf
        class_id: 4, // Rogue
        name: "Elusive Killing Spree",
        description: "100% evasion during Killing Spree",
    },
    ClassRaceSynergy {
        race_id: 6,  // Tauren
        class_id: 2, // Paladin
        name: "Holy Consecration Stomp",
        description: "War Stomp erupts with Holy Consecration (120 Holy dmg)",
    },
    ClassRaceSynergy {
        race_id: 1,  // Human
        class_id: 8, // Mage
        name: "Arcane Vacuum",
        description: "Time Warp doubles magnet radius (+100%)",
    },
];

pub fn synergy_for(race_id: u8, class_id: u8) -> Option<&'static ClassRaceSynergy> {
    SYNERGIES.iter().find(|s| s.race_id == race_id && s.class_id == class_id)
}

/// Checks whether an ability key or spell ID represents a bleed or burn effect.
pub fn is_bleed_or_burn(key: &str, spell: u32) -> bool {
    if spell == 1943 || spell == 348 || spell == 11366 || spell == 2136 || spell == 8042 {
        return true;
    }
    let lower = key.to_ascii_lowercase();
    lower.contains("bleed")
        || lower.contains("rend")
        || lower.contains("rip")
        || lower.contains("rupture")
        || lower.contains("deep_wounds")
        || lower.contains("garrote")
        || lower.contains("rake")
        || lower.contains("burn")
        || lower.contains("fire")
        || lower.contains("flame")
        || lower.contains("immolat")
        || lower.contains("pyro")
        || lower.contains("searing")
        || lower.contains("combust")
        || lower.contains("scorch")
        || lower.contains("shadowburn")
}

/// Returns true if an enemy is currently afflicted by any bleed or burn DoT.
pub(super) fn is_bleeding_or_burning(enemy: &Enemy) -> bool {
    enemy.dots.iter().any(|d| is_bleed_or_burn(d.key, d.spell))
}

/// System running in `Update` handling active periodic racial traits like Tauren's *War Stomp*
/// and Undead's *Will of the Forsaken* passive lifesteal.
///
/// Uses strictly disjoint queries (`hero: Query<..., With<Hero>>`, `enemies: Query<..., Without<Hero>>`)
/// to prevent Bevy scheduling conflicts.
pub(super) fn tick_racial_traits(
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy), Without<Hero>>,
    mut rig: ResMut<super::hero::CamRig>,
    mut fx: Fx,
    mut combat_text: MessageWriter<CombatTextMsg>,
    mut sounds: MessageWriter<crate::sound::GlueSound>,
    mut stomp_timer: Local<f32>,
) {
    if run.dead_t.is_some() {
        return;
    }
    let Ok((hero_e, hero_tf)) = hero.single() else {
        return;
    };
    let dt = time.delta_secs();

    // 1. Tauren (6): War Stomp - Passive radial stomp every 15.0s stunning nearby enemies within 6 yards for 2.0s
    if run.race == 6 {
        *stomp_timer += dt;
        if *stomp_timer >= 15.0 {
            *stomp_timer = 0.0;
            let hero_pos = hero_tf.translation;
            let mut hit_targets = Vec::new();
            let is_paladin = run.class.id == 2;
            for (e, tf, mut en) in &mut enemies {
                if en.dead {
                    continue;
                }
                let diff = tf.translation - hero_pos;
                let flat_dist = (diff.x * diff.x + diff.z * diff.z).sqrt();
                if flat_dist <= 6.0 {
                    let dur = if en.boss { 2.0 * 0.3 } else { 2.0 };
                    en.stun_t = en.stun_t.max(dur);
                    if is_paladin {
                        let holy_dmg = 120.0 * run.stats().might;
                        en.hp -= holy_dmg;
                        combat_text.write(CombatTextMsg {
                            pos: tf.translation + Vec3::Y * 1.5,
                            amount: holy_dmg,
                            is_crit: false,
                            kind: CombatTextKind::Holy,
                        });
                    }
                    fx.impact(e, if is_paladin { 26573 } else { 20549 });
                    hit_targets.push(e);
                }
            }
            fx.cast(hero_e, if is_paladin { 26573 } else { 20549 }, &hit_targets, Some(hero_pos));
            rig.shake(1.0, 0.35);
            sounds.write(crate::sound::GlueSound("igMainMenuOptionCheckBoxOff"));
        }
    }

    // 2. Undead (5): Will of the Forsaken - Passive lifesteal on bleeding or burning targets
    if run.race == 5 {
        let mut total_siphon = 0.0;
        for (_e, _tf, en) in &enemies {
            if en.dead {
                continue;
            }
            if is_bleeding_or_burning(&en) {
                let dot_dps: f32 = en
                    .dots
                    .iter()
                    .filter(|d| is_bleed_or_burn(d.key, d.spell))
                    .map(|d| d.dps)
                    .sum();
                let dps = if dot_dps > 0.0 { dot_dps } else { 10.0 };
                total_siphon += dps * 0.10 * dt;
            }
        }
        if total_siphon > 0.0 {
            let max_hp = run.stats().max_hp;
            let heal = total_siphon.min(max_hp - run.hp);
            if heal > 0.0 {
                run.hp = (run.hp + heal).min(max_hp);
                combat_text.write(CombatTextMsg {
                    pos: hero_tf.translation + Vec3::Y * 1.5,
                    amount: heal,
                    is_crit: false,
                    kind: CombatTextKind::Heal,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_racial_trait_mapping() {
        assert_eq!(race_trait(1), RaceTrait::TheHumanSpirit);
        assert_eq!(race_trait(2), RaceTrait::BloodFury);
        assert_eq!(race_trait(3), RaceTrait::Stoneform);
        assert_eq!(race_trait(4), RaceTrait::Quickness);
        assert_eq!(race_trait(5), RaceTrait::WillOfTheForsaken);
        assert_eq!(race_trait(6), RaceTrait::WarStomp);
        assert_eq!(race_trait(7), RaceTrait::EngineeringTinkering);
        assert_eq!(race_trait(8), RaceTrait::Berserking);
        assert_eq!(race_trait(99), RaceTrait::None);
    }

    #[test]
    fn test_apply_stats() {
        let run = Run::default();
        let mut stats = run.stats();
        let base_xp = stats.xp_bonus;
        let base_gold = stats.gold_bonus;
        RaceTrait::TheHumanSpirit.apply_stats(&mut stats, &run);
        assert!((stats.xp_bonus - base_xp * 1.15).abs() < 1e-4);
        assert!((stats.gold_bonus - base_gold * 1.10).abs() < 1e-4);

        let mut d_stats = run.stats();
        let base_hp = d_stats.max_hp;
        RaceTrait::Stoneform.apply_stats(&mut d_stats, &run);
        assert!((d_stats.max_hp - base_hp * 1.15).abs() < 1e-4);
        assert_eq!(d_stats.regen, 2.0);
    }

    #[test]
    fn test_is_bleed_or_burn() {
        assert!(is_bleed_or_burn("rend", 0));
        assert!(is_bleed_or_burn("fireball", 0));
        assert!(is_bleed_or_burn("deep_wounds", 0));
        assert!(is_bleed_or_burn("unknown", 1943)); // Immolate spell id
        assert!(!is_bleed_or_burn("frostbolt", 116));
    }

    #[test]
    fn test_class_race_synergies() {
        assert!(synergy_for(2, 1).is_some()); // Orc Warrior
        assert_eq!(synergy_for(2, 1).unwrap().name, "Blood Fury Avatar");
        assert!(synergy_for(5, 9).is_some()); // Undead Warlock
        assert!(synergy_for(4, 4).is_some()); // Night Elf Rogue
        assert!(synergy_for(6, 2).is_some()); // Tauren Paladin
        assert!(synergy_for(1, 8).is_some()); // Human Mage
        assert!(synergy_for(3, 1).is_none()); // Dwarf Warrior (no specific synergy)
    }
}
