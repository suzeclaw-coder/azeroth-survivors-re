//! Mid-run Gear Enchanter (Ilsa Runeweaver) for Warcraft Survivors.
//!
//! Provides slot enchantments purchased with in-run gold that persist for the duration of the run.
//! Ported from Hordes of Azeroth (`VanillaBonkEnchanter.lua`).

#![allow(dead_code)]

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::gear::EquipSlot;
use super::{Rng, Stats};

/// Enchantment types that can be bound to equipment slots during a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnchantKind {
    FieryWeapon,  // Chance on attack to deal bonus fire burst damage
    Crusader,     // Chance on hit to burst heal and temporarily boost Might
    Lifestealing, // Leech a percentage of direct damage as health
    MinorSpeed,   // Passive movement speed and dash cooldown reduction
    SpellPower,   // Passive Might and Critical strike multiplier
}

impl EnchantKind {
    pub const ALL: [EnchantKind; 5] = [
        EnchantKind::FieryWeapon,
        EnchantKind::Crusader,
        EnchantKind::Lifestealing,
        EnchantKind::MinorSpeed,
        EnchantKind::SpellPower,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::FieryWeapon => "Fiery Weapon",
            Self::Crusader => "Crusader",
            Self::Lifestealing => "Lifestealing",
            Self::MinorSpeed => "Greater Agility",
            Self::SpellPower => "Spell Power",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::FieryWeapon => "Chance on hit to burst extra Fire damage",
            Self::Crusader => "Chance on hit to heal 12% Max HP and boost Might by +30%",
            Self::Lifestealing => "Leech 3% of direct attack damage as health",
            Self::MinorSpeed => "+8% Move Speed & -10% Dash cooldown",
            Self::SpellPower => "+8% Might and +3% Critical Strike Chance",
        }
    }
}

/// Tier grade of the enchantment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum EnchantGrade {
    #[default]
    Minor,    // Gray
    Lesser,   // Green
    Greater,  // Blue
    Superior, // Purple
    Godly,    // Orange
}

impl EnchantGrade {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Minor => "Minor",
            Self::Lesser => "Lesser",
            Self::Greater => "Greater",
            Self::Superior => "Superior",
            Self::Godly => "Godly",
        }
    }

    pub fn multiplier(&self) -> f32 {
        match self {
            Self::Minor => 1.0,
            Self::Lesser => 1.3,
            Self::Greater => 1.7,
            Self::Superior => 2.2,
            Self::Godly => 3.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotEnchant {
    pub slot: EquipSlot,
    pub kind: EnchantKind,
    pub grade: EnchantGrade,
}

/// Run-specific enchantment manager.
#[derive(Debug, Clone, Default)]
pub struct RunEnchanter {
    pub enchants: HashMap<EquipSlot, SlotEnchant>,
    pub is_open: bool,
    pub crusader_timer: f32,
}

impl RunEnchanter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enchant_slot(&mut self, slot: EquipSlot, kind: EnchantKind, grade: EnchantGrade) {
        self.enchants.insert(slot, SlotEnchant { slot, kind, grade });
    }

    /// Applies passive enchantment bonuses to run stats.
    pub fn apply_to_stats(&self, stats: &mut Stats) {
        for ench in self.enchants.values() {
            let mult = ench.grade.multiplier();
            match ench.kind {
                EnchantKind::MinorSpeed => {
                    stats.speed *= 1.0 + (0.08 * mult);
                }
                EnchantKind::SpellPower => {
                    stats.might += 0.08 * mult;
                    stats.crit += 0.03 * mult;
                }
                _ => {}
            }
        }
        if self.crusader_timer > 0.0 {
            stats.might += 0.35;
        }
    }

    /// Evaluates on-hit proc chances.
    /// Returns `(fire_damage_bonus, crusader_triggered, lifesteal_fraction)`.
    pub fn roll_on_hit_procs(&mut self, rng: &mut Rng) -> (f32, bool, f32) {
        let mut fire_dmg = 0.0;
        let mut crusader = false;
        let mut lifesteal = 0.0;

        for ench in self.enchants.values() {
            let mult = ench.grade.multiplier();
            match ench.kind {
                EnchantKind::FieryWeapon => {
                    if rng.f32() < 0.25 {
                        fire_dmg += 35.0 * mult;
                    }
                }
                EnchantKind::Crusader => {
                    if rng.f32() < 0.15 {
                        crusader = true;
                        self.crusader_timer = 6.0;
                    }
                }
                EnchantKind::Lifestealing => {
                    lifesteal += 0.03 * mult;
                }
                _ => {}
            }
        }

        (fire_dmg, crusader, lifesteal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enchant_application() {
        let mut enchanter = RunEnchanter::new();
        enchanter.enchant_slot(EquipSlot::MainHand, EnchantKind::SpellPower, EnchantGrade::Greater);

        let mut stats = Stats {
            might: 1.0,
            crit: 0.05,
            speed: 7.0,
            ..Stats::default()
        };

        enchanter.apply_to_stats(&mut stats);
        assert!(stats.might > 1.10);
        assert!(stats.crit > 0.05);
    }
}
