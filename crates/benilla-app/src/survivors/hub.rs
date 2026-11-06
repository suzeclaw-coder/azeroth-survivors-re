//! 3D Town Hub & The Inn for Warcraft Survivors.
//!
//! Provides the pre-run town hub featuring interactive NPCs:
//! - Innkeeper: Meta-progression upgrades
//! - Quartermaster Durgan: Armory, gear vendor & salvage
//! - Sayge <Darkmoon Faire>: Glyphs & fortune buffs
//! - Run Master: Stage, difficulty, and map selection
//! - Target Dummies: Stationary damage targets for DPS testing
//!
//! Ported from Hordes of Azeroth (`VanillaBonkInnkeeper.lua`, `VanillaBonkQuartermaster.lua`).

#![allow(dead_code)]

use bevy::prelude::*;
use super::units::{self, UnitSpec};
use super::RunEntity;

/// Town Hub NPC identifiers.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubNpcKind {
    Innkeeper,
    QuartermasterDurgan,
    Sayge,
    RunMaster,
    TargetDummy,
    EnchanterIlsa,
}

impl HubNpcKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Innkeeper => "Innkeeper Allison",
            Self::QuartermasterDurgan => "Quartermaster Durgan Ironbelly",
            Self::Sayge => "Sayge <Darkmoon Faire>",
            Self::RunMaster => "Run Master Thalor",
            Self::TargetDummy => "Target Dummy",
            Self::EnchanterIlsa => "Ilsa Runeweaver",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Innkeeper => "<Tavern & Talents>",
            Self::QuartermasterDurgan => "<Armory & Blacksmith>",
            Self::Sayge => "<Glyphs & Divination>",
            Self::RunMaster => "<Expeditions>",
            Self::TargetDummy => "<Damage Test>",
            Self::EnchanterIlsa => "<Mystic Enchanter>",
        }
    }

    pub fn display_id(&self) -> u32 {
        match self {
            Self::Innkeeper => 3060,
            Self::QuartermasterDurgan => 4793,
            Self::Sayge => 14856,
            Self::RunMaster => 4363,
            Self::TargetDummy => 2054,
            Self::EnchanterIlsa => 10045,
        }
    }
}

/// Spawns town hub NPCs around the tavern square.
pub fn spawn_hub_npcs(mut commands: Commands) {
    let npcs = [
        (HubNpcKind::Innkeeper, Vec3::new(-6.0, 0.0, -4.0)),
        (HubNpcKind::QuartermasterDurgan, Vec3::new(6.0, 0.0, -4.0)),
        (HubNpcKind::Sayge, Vec3::new(-8.0, 0.0, 4.0)),
        (HubNpcKind::RunMaster, Vec3::new(0.0, 0.0, -8.0)),
        (HubNpcKind::TargetDummy, Vec3::new(8.0, 0.0, 6.0)),
        (HubNpcKind::EnchanterIlsa, Vec3::new(4.0, 0.0, 4.0)),
    ];

    for (kind, pos) in npcs {
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: 0x9000_0000 | (kind.display_id() as u64),
                display: kind.display_id(),
                scale: if kind == HubNpcKind::TargetDummy { 1.2 } else { 1.0 },
                level: 60,
                faction: units::FACTION_FRIENDLY,
                bytes0: 0,
                weapons: &[],
                sheath: 0,
            },
            pos,
            0.0,
        );
        commands.entity(e).insert((kind, RunEntity));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_npc_properties() {
        assert_eq!(HubNpcKind::Innkeeper.name(), "Innkeeper Allison");
        assert_eq!(HubNpcKind::TargetDummy.display_id(), 2054);
        assert_eq!(HubNpcKind::EnchanterIlsa.name(), "Ilsa Runeweaver");
    }
}
