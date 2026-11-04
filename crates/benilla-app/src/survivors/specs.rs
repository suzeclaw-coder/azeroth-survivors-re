//! Class Specialization System (Level 10 Spec Choice).
//!
//! Upon reaching Level 10, players choose one of two authentic WoW talent specializations:
//! - Warrior: Arms (Sweeping Strikes & bleed) vs Protection (Shield Wall & thorns)
//! - Paladin: Retribution (Judgement crits & Might) vs Holy (Consecration Aura & regen)
//! - Hunter: Beast Mastery (Companion damage & Haste) vs Marksmanship (Trueshot & pierce)
//! - Rogue: Assassination (Deadly Poison DoTs) vs Combat (Adrenaline Rush & dash CD)
//! - Priest: Shadow (Shadowform damage) vs Holy (Holy Nova Aura heal & damage)
//! - Shaman: Elemental (Overload double-cast) vs Enhancement (Windfury multi-strike)
//! - Mage: Fire (Ignite & Pyroblast procs) vs Frost (Shatter & freeze criticals)
//! - Warlock: Destruction (Chaos Wave burst) vs Affliction (Siphon Life leech)
//! - Druid: Balance (Celestial Alignment) vs Feral (Predatory Instincts & crit)

use super::Stats;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpecKind {
    WarriorArms,
    WarriorProtection,
    PaladinRetribution,
    PaladinHoly,
    HunterBeastMastery,
    HunterMarksmanship,
    RogueAssassination,
    RogueCombat,
    PriestShadow,
    PriestHoly,
    ShamanElemental,
    ShamanEnhancement,
    MageFire,
    MageFrost,
    WarlockDestruction,
    WarlockAffliction,
    DruidBalance,
    DruidFeral,
}

#[derive(Clone, Copy, Debug)]
pub struct SpecDef {
    pub kind: SpecKind,
    pub class_id: u8,
    pub name: &'static str,
    pub tag: &'static str,
    pub desc: &'static str,
    pub spell: u32,
}

pub static SPECS: &[SpecDef] = &[
    // 1: Warrior
    SpecDef {
        kind: SpecKind::WarriorArms,
        class_id: 1,
        name: "Arms",
        tag: "Specialization",
        desc: "Sweeping Strikes: Bleed damage +40%; primary melee strikes hit 2 extra nearby targets.",
        spell: 12294,
    },
    SpecDef {
        kind: SpecKind::WarriorProtection,
        class_id: 1,
        name: "Protection",
        tag: "Specialization",
        desc: "Shield Wall: Gain a permanent 250 HP shield that refreshes every 12s; +150% thorns damage.",
        spell: 871,
    },

    // 2: Paladin
    SpecDef {
        kind: SpecKind::PaladinRetribution,
        class_id: 2,
        name: "Retribution",
        tag: "Specialization",
        desc: "Judgement of Light: Might +35%; critical strikes unleash radiant holy shockwaves.",
        spell: 20271,
    },
    SpecDef {
        kind: SpecKind::PaladinHoly,
        class_id: 2,
        name: "Holy",
        tag: "Specialization",
        desc: "Consecration Aura: Permanent holy ground dealing 35 dmg/s to foes and +15 HP/s healing to hero.",
        spell: 26573,
    },

    // 3: Hunter
    SpecDef {
        kind: SpecKind::HunterBeastMastery,
        class_id: 3,
        name: "Beast Mastery",
        tag: "Specialization",
        desc: "Bestial Synergy: Combat companions deal +100% damage; hero gains +20% haste.",
        spell: 19574,
    },
    SpecDef {
        kind: SpecKind::HunterMarksmanship,
        class_id: 3,
        name: "Marksmanship",
        tag: "Specialization",
        desc: "Trueshot Aura: Projectile damage +30%; all shots pierce 2 additional enemies.",
        spell: 19506,
    },

    // 4: Rogue
    SpecDef {
        kind: SpecKind::RogueAssassination,
        class_id: 4,
        name: "Assassination",
        tag: "Specialization",
        desc: "Deadly Poison: Attacks poison targets for 35 nature dmg/s; poisoned foes burst on death.",
        spell: 2823,
    },
    SpecDef {
        kind: SpecKind::RogueCombat,
        class_id: 4,
        name: "Combat",
        tag: "Specialization",
        desc: "Adrenaline Rush: Cooldown recovery +40%; mobility dash cooldown reduced by 40%.",
        spell: 13750,
    },

    // 5: Priest
    SpecDef {
        kind: SpecKind::PriestShadow,
        class_id: 5,
        name: "Shadow",
        tag: "Specialization",
        desc: "Shadowform: Shadow spell damage +50%; spell hits slow enemy movement speed by 40%.",
        spell: 15473,
    },
    SpecDef {
        kind: SpecKind::PriestHoly,
        class_id: 5,
        name: "Holy",
        tag: "Specialization",
        desc: "Holy Nova Radiance: Every 6s unleashes a holy burst healing hero 25 HP and damaging foes 60.",
        spell: 27801,
    },

    // 7: Shaman
    SpecDef {
        kind: SpecKind::ShamanElemental,
        class_id: 7,
        name: "Elemental",
        tag: "Specialization",
        desc: "Elemental Overload: All spells have a 35% chance to duplicate and cast twice instantly.",
        spell: 421,
    },
    SpecDef {
        kind: SpecKind::ShamanEnhancement,
        class_id: 7,
        name: "Enhancement",
        tag: "Specialization",
        desc: "Windfury Weapon: Melee strikes have a 25% chance to trigger 3 extra rapid attacks.",
        spell: 8232,
    },

    // 8: Mage
    SpecDef {
        kind: SpecKind::MageFire,
        class_id: 8,
        name: "Fire",
        tag: "Specialization",
        desc: "Ignite & Hot Streak: All spells burn foes; burn ticks have 15% chance to trigger free Pyroblasts.",
        spell: 11366,
    },
    SpecDef {
        kind: SpecKind::MageFrost,
        class_id: 8,
        name: "Frost",
        tag: "Specialization",
        desc: "Shatter: Freeze duration +50%; attacks against frozen enemies deal 2.5x critical damage.",
        spell: 11185,
    },

    // 9: Warlock
    SpecDef {
        kind: SpecKind::WarlockDestruction,
        class_id: 9,
        name: "Destruction",
        tag: "Specialization",
        desc: "Chaos Wave: Hellfire and Shadow Bolt damage +50%; targets explode with hellfire upon defeat.",
        spell: 17962,
    },
    SpecDef {
        kind: SpecKind::WarlockAffliction,
        class_id: 9,
        name: "Affliction",
        tag: "Specialization",
        desc: "Siphon Life: All DoT damage heals the hero for 30% of damage dealt.",
        spell: 18879,
    },

    // 11: Druid
    SpecDef {
        kind: SpecKind::DruidBalance,
        class_id: 11,
        name: "Balance",
        tag: "Specialization",
        desc: "Celestial Alignment: Spell power +40%; area of effect for all spells increased by 35%.",
        spell: 48465,
    },
    SpecDef {
        kind: SpecKind::DruidFeral,
        class_id: 11,
        name: "Feral",
        tag: "Specialization",
        desc: "Predatory Instincts: Movement speed +25%; critical strike chance increased by +30%.",
        spell: 5217,
    },
];

#[allow(dead_code)]
pub fn find_spec(kind: SpecKind) -> &'static SpecDef {
    SPECS.iter().find(|s| s.kind == kind).expect("spec must exist")
}

pub fn specs_for_class_id(class_id: u8) -> Vec<&'static SpecDef> {
    SPECS.iter().filter(|s| s.class_id == class_id).collect()
}

pub fn apply_spec_stats(spec: SpecKind, stats: &mut Stats) {
    match spec {
        SpecKind::WarriorArms => {
            stats.might += 0.20;
        }
        SpecKind::WarriorProtection => {
            stats.max_hp *= 1.25;
            stats.armor += 0.20;
        }
        SpecKind::PaladinRetribution => {
            stats.might += 0.35;
        }
        SpecKind::PaladinHoly => {
            stats.regen += 15.0;
        }
        SpecKind::HunterBeastMastery => {
            stats.haste += 0.20;
        }
        SpecKind::HunterMarksmanship => {
            stats.might += 0.30;
        }
        SpecKind::RogueAssassination => {
            stats.crit += 0.15;
        }
        SpecKind::RogueCombat => {
            stats.haste += 0.25;
            stats.speed *= 1.10;
        }
        SpecKind::PriestShadow => {
            stats.might += 0.50;
        }
        SpecKind::PriestHoly => {
            stats.regen += 10.0;
        }
        SpecKind::ShamanElemental => {
            stats.area += 0.25;
            stats.might += 0.20;
        }
        SpecKind::ShamanEnhancement => {
            stats.haste += 0.30;
        }
        SpecKind::MageFire => {
            stats.might += 0.30;
        }
        SpecKind::MageFrost => {
            stats.area += 0.35;
        }
        SpecKind::WarlockDestruction => {
            stats.might += 0.40;
        }
        SpecKind::WarlockAffliction => {
            stats.growth += 0.20;
        }
        SpecKind::DruidBalance => {
            stats.might += 0.40;
            stats.area += 0.35;
        }
        SpecKind::DruidFeral => {
            stats.speed *= 1.25;
            stats.crit += 0.30;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eighteen_specs_defined() {
        assert_eq!(SPECS.len(), 18);
        for id in [1, 2, 3, 4, 5, 7, 8, 9, 11] {
            let list = specs_for_class_id(id);
            assert_eq!(list.len(), 2, "class id {} should have 2 specs", id);
        }
    }
}
