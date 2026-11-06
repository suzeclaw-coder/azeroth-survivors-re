//! Earnable Feat Titles and Passive Perks for Warcraft Survivors.
//!
//! Provides player titles unlocked via in-game achievements and feats,
//! granting small passive stat bonuses. Ported from Hordes of Azeroth (`VanillaBonkTitles.lua`).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use super::gear::StatType;
use super::save::AccountSave;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TitleId {
    TheImmortal,      // Clear a run on Hardcore without dying (+5% Max HP)
    TheBloodthirsty,  // Reach a 150+ killing spree (+3% Crit)
    TheSwift,         // Clear a zone in under 12 minutes (+5% Move Speed)
    ChampionOfNaaru,  // Defeat 15 total bosses (+5% Intellect/Spell Power)
    TheDiplomat,      // Play with all 8 races (+10% Gold Earned)
}

#[derive(Debug, Clone)]
pub struct TitleDef {
    pub id: TitleId,
    pub title: &'static str,
    pub description: &'static str,
    pub perk_description: &'static str,
    pub stat: StatType,
    pub bonus_value: f32,
}

pub const TITLES: &[TitleDef] = &[
    TitleDef {
        id: TitleId::TheImmortal,
        title: "the Immortal",
        description: "Clear a complete run on Hardcore difficulty",
        perk_description: "+10% Maximum Health",
        stat: StatType::Stamina,
        bonus_value: 10.0,
    },
    TitleDef {
        id: TitleId::TheBloodthirsty,
        title: "the Bloodthirsty",
        description: "Reach a 150+ killing spree combo",
        perk_description: "+3% Critical Strike Chance",
        stat: StatType::Crit,
        bonus_value: 3.0,
    },
    TitleDef {
        id: TitleId::TheSwift,
        title: "the Swift",
        description: "Survive or extract in under 12 minutes",
        perk_description: "+5% Movement Speed",
        stat: StatType::Agility,
        bonus_value: 5.0,
    },
    TitleDef {
        id: TitleId::ChampionOfNaaru,
        title: "Champion of the Naaru",
        description: "Defeat 15 bosses across all runs",
        perk_description: "+8% Spell & Physical Power",
        stat: StatType::Intellect,
        bonus_value: 8.0,
    },
    TitleDef {
        id: TitleId::TheDiplomat,
        title: "the Diplomat",
        description: "Complete runs across all 8 races",
        perk_description: "+10% Bonus Gold",
        stat: StatType::Stamina,
        bonus_value: 5.0,
    },
];

pub fn get_title_def(id: TitleId) -> Option<&'static TitleDef> {
    TITLES.iter().find(|t| t.id == id)
}

/// Checks and unlocks titles based on run and account achievements.
pub fn check_title_unlocks(account: &mut AccountSave, run_won: bool, is_hardcore: bool, spree: u32, elapsed: f32) {
    if run_won && is_hardcore {
        account.unlock_title(TitleId::TheImmortal);
    }
    if spree >= 150 {
        account.unlock_title(TitleId::TheBloodthirsty);
    }
    if run_won && elapsed <= 720.0 {
        account.unlock_title(TitleId::TheSwift);
    }
    if account.high_scores.values().map(|r| r.highest_level).sum::<u32>() >= 100 {
        account.unlock_title(TitleId::ChampionOfNaaru);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_lookup() {
        let title = get_title_def(TitleId::TheImmortal).expect("title should exist");
        assert_eq!(title.title, "the Immortal");
    }
}
