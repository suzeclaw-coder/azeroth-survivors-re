//! ARPG Gear, Itemization, and Equipment for Warcraft Survivors.
//!
//! Provides item slots, procedural loot drops, stat bonuses, and quality tiers
//! ported from Hordes of Azeroth (`VanillaBonkGear.lua`).

#![allow(dead_code)]

use std::collections::HashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Rng, Stats};

/// Stat types that appear on gear items.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatType {
    Stamina,   // +HP (10 HP per stat point)
    Strength,  // +Physical damage
    Agility,   // +Physical damage & crit
    Intellect, // +Spell power
    Haste,     // -Cooldown CDR %
    Crit,      // +Crit chance %
    Armor,     // -Damage taken %
}

impl StatType {
    pub const ALL: [StatType; 7] = [
        StatType::Stamina,
        StatType::Strength,
        StatType::Agility,
        StatType::Intellect,
        StatType::Haste,
        StatType::Crit,
        StatType::Armor,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Stamina => "Stamina",
            Self::Strength => "Strength",
            Self::Agility => "Agility",
            Self::Intellect => "Intellect",
            Self::Haste => "Haste",
            Self::Crit => "Critical Strike",
            Self::Armor => "Armor",
        }
    }
}

/// Equipment slots following World of Warcraft conventions.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EquipSlot {
    Head,
    Shoulders,
    Chest,
    Legs,
    MainHand,
    OffHand,
    Trinket1,
    Trinket2,
    Tabard,
}

impl EquipSlot {
    pub const ALL: [EquipSlot; 9] = [
        EquipSlot::Head,
        EquipSlot::Shoulders,
        EquipSlot::Chest,
        EquipSlot::Legs,
        EquipSlot::MainHand,
        EquipSlot::OffHand,
        EquipSlot::Trinket1,
        EquipSlot::Trinket2,
        EquipSlot::Tabard,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::Shoulders => "Shoulders",
            Self::Chest => "Chest",
            Self::Legs => "Legs",
            Self::MainHand => "Main Hand",
            Self::OffHand => "Off-Hand",
            Self::Trinket1 => "Trinket 1",
            Self::Trinket2 => "Trinket 2",
            Self::Tabard => "Tabard",
        }
    }
}

/// Item rarity tiers with authentic WoW color coding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum ItemQuality {
    Poor,
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl ItemQuality {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Poor => "Poor",
            Self::Common => "Common",
            Self::Uncommon => "Uncommon",
            Self::Rare => "Rare",
            Self::Epic => "Epic",
            Self::Legendary => "Legendary",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Poor => Color::srgb(0.62, 0.62, 0.62),      // Gray
            Self::Common => Color::srgb(1.0, 1.0, 1.0),        // White
            Self::Uncommon => Color::srgb(0.12, 1.0, 0.0),     // Green
            Self::Rare => Color::srgb(0.0, 0.44, 0.87),        // Blue
            Self::Epic => Color::srgb(0.64, 0.21, 0.93),       // Purple
            Self::Legendary => Color::srgb(1.0, 0.5, 0.0),     // Orange
        }
    }

    pub fn stat_multiplier(&self) -> f32 {
        match self {
            Self::Poor => 0.5,
            Self::Common => 1.0,
            Self::Uncommon => 1.3,
            Self::Rare => 1.7,
            Self::Epic => 2.2,
            Self::Legendary => 3.0,
        }
    }
}

/// A persistent or dropped gear piece.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GearItem {
    pub id: u32,
    pub name: String,
    pub slot: EquipSlot,
    pub quality: ItemQuality,
    pub item_level: u32,
    pub stats: HashMap<StatType, f32>,
    pub display_id: u32,
    pub icon_spell: u32,
}

impl GearItem {
    /// Applies this item's stats to the hero derived stats.
    pub fn apply_to_stats(&self, stats: &mut Stats, class_id: u8) {
        for (&stat, &val) in &self.stats {
            match stat {
                StatType::Stamina => {
                    stats.max_hp += val * 10.0;
                }
                StatType::Strength => {
                    // Warrior (1), Paladin (2), Rogue (4), Druid (11)
                    if matches!(class_id, 1 | 2 | 4 | 11) {
                        stats.might += val * 0.01;
                    }
                }
                StatType::Agility => {
                    // Hunter (3), Rogue (4)
                    if matches!(class_id, 3 | 4) {
                        stats.might += val * 0.01;
                        stats.crit += val * 0.005;
                    }
                }
                StatType::Intellect => {
                    // Priest (5), Shaman (7), Mage (8), Warlock (9), Druid (11)
                    if matches!(class_id, 5 | 7 | 8 | 9 | 11) {
                        stats.might += val * 0.01;
                    }
                }
                StatType::Haste => {
                    stats.haste += (val * 0.005).min(0.20);
                }
                StatType::Crit => {
                    stats.crit += (val * 0.005).min(0.25);
                }
                StatType::Armor => {
                    stats.armor += (val * 0.005).min(0.30);
                }
            }
        }
    }

    /// Gold value when sold or salvaged.
    pub fn salvage_value(&self) -> u64 {
        let base = (self.item_level as u64) * 5;
        match self.quality {
            ItemQuality::Poor => 5,
            ItemQuality::Common => base,
            ItemQuality::Uncommon => base * 2,
            ItemQuality::Rare => base * 5,
            ItemQuality::Epic => base * 12,
            ItemQuality::Legendary => base * 30,
        }
    }
}

/// Generates a randomized gear item tailored to current zone level and quality roll.
pub fn generate_gear(rng: &mut Rng, ilvl: u32, quality_bias: f32) -> GearItem {
    let slots = EquipSlot::ALL;
    let slot = slots[rng.index(slots.len())];

    let roll = rng.f32() + quality_bias;
    let quality = if roll > 0.96 {
        ItemQuality::Legendary
    } else if roll > 0.85 {
        ItemQuality::Epic
    } else if roll > 0.65 {
        ItemQuality::Rare
    } else if roll > 0.35 {
        ItemQuality::Uncommon
    } else {
        ItemQuality::Common
    };

    let mult = quality.stat_multiplier();
    let base_stat = ((ilvl as f32) * 0.8 * mult).max(1.0).round();

    let mut stats = HashMap::new();
    stats.insert(StatType::Stamina, base_stat);

    match slot {
        EquipSlot::MainHand | EquipSlot::OffHand => {
            stats.insert(StatType::Strength, base_stat * 0.8);
            stats.insert(StatType::Crit, (base_stat * 0.4).max(1.0));
        }
        EquipSlot::Head | EquipSlot::Chest | EquipSlot::Legs => {
            stats.insert(StatType::Armor, base_stat * 1.2);
            stats.insert(StatType::Intellect, base_stat * 0.7);
        }
        EquipSlot::Shoulders => {
            stats.insert(StatType::Agility, base_stat * 0.7);
            stats.insert(StatType::Haste, (base_stat * 0.3).max(1.0));
        }
        EquipSlot::Trinket1 | EquipSlot::Trinket2 => {
            stats.insert(StatType::Haste, (base_stat * 0.6).max(1.0));
            stats.insert(StatType::Crit, (base_stat * 0.6).max(1.0));
        }
        EquipSlot::Tabard => {
            stats.insert(StatType::Stamina, base_stat * 0.5);
        }
    }

    let (name, icon_spell, display_id) = match slot {
        EquipSlot::Head => ("Gladiator's Crown", 28271, 31120),
        EquipSlot::Shoulders => ("Spaulders of the Vanguard", 28272, 31121),
        EquipSlot::Chest => ("Stormwind Breastplate", 28273, 31122),
        EquipSlot::Legs => ("Ironforge Greaves", 28274, 31123),
        EquipSlot::MainHand => ("Blade of the Warchief", 28275, 31124),
        EquipSlot::OffHand => ("Bulwark of the Ancients", 28276, 31125),
        EquipSlot::Trinket1 => ("Talisman of Ephemeral Power", 28277, 31126),
        EquipSlot::Trinket2 => ("Darkmoon Card: Blue Dragon", 28278, 31127),
        EquipSlot::Tabard => ("Tabard of the Scarlet Crusade", 28279, 31128),
    };

    let quality_prefix = match quality {
        ItemQuality::Poor => "Cracked ",
        ItemQuality::Common => "",
        ItemQuality::Uncommon => "Reinforced ",
        ItemQuality::Rare => "Heroic ",
        ItemQuality::Epic => "Ancient ",
        ItemQuality::Legendary => "Mythical ",
    };

    let item_name = format!("{}{}", quality_prefix, name);
    let id = 0xFFFF_0000 | ((rng.next_u64() as u32) & 0xFFFF);

    GearItem {
        id,
        name: item_name,
        slot,
        quality,
        item_level: ilvl,
        stats,
        display_id,
        icon_spell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gear_stats_application() {
        let mut stats = Stats {
            might: 1.0,
            max_hp: 100.0,
            speed: 7.0,
            regen: 0.0,
            haste: 0.0,
            area: 1.0,
            magnet: 4.5,
            growth: 1.0,
            armor: 0.0,
            crit: 0.05,
            xp_bonus: 1.0,
            gold_bonus: 1.0,
            evasion: 0.0,
            stun_res: 0.0,
        };

        let mut gear_stats = HashMap::new();
        gear_stats.insert(StatType::Stamina, 10.0);
        gear_stats.insert(StatType::Strength, 20.0);
        gear_stats.insert(StatType::Crit, 5.0);

        let item = GearItem {
            id: 1,
            name: "Test Plate".into(),
            slot: EquipSlot::Chest,
            quality: ItemQuality::Rare,
            item_level: 30,
            stats: gear_stats,
            display_id: 1234,
            icon_spell: 28273,
        };

        // For Warrior (class 1)
        item.apply_to_stats(&mut stats, 1);
        assert_eq!(stats.max_hp, 200.0); // +10 * 10 = +100
        assert!((stats.might - 1.20).abs() < 1e-4); // +20 * 0.01 = +0.20
        assert!((stats.crit - 0.075).abs() < 1e-4); // +5 * 0.005 = +0.025
    }

    #[test]
    fn test_salvage_value() {
        let item = GearItem {
            id: 1,
            name: "Epic Weapon".into(),
            slot: EquipSlot::MainHand,
            quality: ItemQuality::Epic,
            item_level: 20,
            stats: HashMap::new(),
            display_id: 100,
            icon_spell: 28275,
        };
        // 20 * 5 * 12 = 1200
        assert_eq!(item.salvage_value(), 1200);
    }
}
