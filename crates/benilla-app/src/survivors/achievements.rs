//! Achievements & Milestone Bounty System.
//!
//! Tracks gameplay achievements across sessions, awarding gold bounties
//! and displaying in-game sliding fanfare banners upon completion.

use bevy::prelude::*;

use crate::sound::GlueSound;
use super::data;
use super::hero::Hero;
use super::save::Account;
use super::{Run, Selection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AchievementDef {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub icon_spell: u32,
    pub bounty: u64,
}

pub static ACHIEVEMENTS: &[AchievementDef] = &[
    AchievementDef {
        id: "first_blood",
        name: "First Blood",
        desc: "Slay your first zone boss.",
        icon_spell: 13323,
        bounty: 100,
    },
    AchievementDef {
        id: "survivor_5m",
        name: "Survivor of the Horde",
        desc: "Survive past 5:00 in any zone.",
        icon_spell: 1022,
        bounty: 200,
    },
    AchievementDef {
        id: "champion_15m",
        name: "Champion of Azeroth",
        desc: "Survive 15:00 and claim victory.",
        icon_spell: 31884,
        bounty: 500,
    },
    AchievementDef {
        id: "overtime_gladiator",
        name: "Overtime Gladiator",
        desc: "Survive 2+ minutes in Endless Overtime.",
        icon_spell: 46924,
        bounty: 500,
    },
    AchievementDef {
        id: "torment_conqueror",
        name: "Torment Conqueror",
        desc: "Claim victory with all 4 Challenge Affixes active.",
        icon_spell: 21056,
        bounty: 1000,
    },
    AchievementDef {
        id: "glass_legend",
        name: "Glass Legend",
        desc: "Claim victory with Glass Cannon active.",
        icon_spell: 11366,
        bounty: 400,
    },
    AchievementDef {
        id: "weapon_master",
        name: "Weapon Master",
        desc: "Evolve any weapon into its ultimate form.",
        icon_spell: 421,
        bounty: 300,
    },
    AchievementDef {
        id: "vault_hoarder",
        name: "Vault Hoarder",
        desc: "Accumulate over 2,000 gold in a single run.",
        icon_spell: 20484,
        bounty: 350,
    },
    AchievementDef {
        id: "slayer_1000",
        name: "Slayer of the Horde",
        desc: "Slay 1,000 enemies in a single run.",
        icon_spell: 15496,
        bounty: 250,
    },
    AchievementDef {
        id: "nightmare_crusher",
        name: "Nightmare Crusher",
        desc: "Claim victory on Nightmare difficulty.",
        icon_spell: 24328,
        bounty: 800,
    },
    AchievementDef {
        id: "caravan_raider",
        name: "Caravan Raider",
        desc: "Intercept and loot the Goblin Supply Caravan.",
        icon_spell: 114049,
        bounty: 200,
    },
    AchievementDef {
        id: "portal_cleanser",
        name: "Portal Cleanser",
        desc: "Destroy the Scourge Necrotic Portal.",
        icon_spell: 30513,
        bounty: 250,
    },
];

pub fn find_achievement(id: &str) -> Option<&'static AchievementDef> {
    ACHIEVEMENTS.iter().find(|a| a.id == id)
}

#[derive(Resource, Default)]
pub struct AchievementState {
    pub active_toast: Option<(&'static AchievementDef, f32)>,
}

#[derive(Component)]
pub struct AchievementToastRoot;

#[derive(Component)]
pub struct AchievementToastTitle;

#[derive(Component)]
pub struct AchievementToastDesc;

#[derive(Component)]
pub struct AchievementToastBounty;

/// Checks for achievement triggers during active gameplay.
pub(super) fn check_achievements(
    time: Res<Time>,
    run: Res<Run>,
    selection: Res<Selection>,
    mut account: ResMut<Account>,
    mut state: ResMut<AchievementState>,
    mut sfx: MessageWriter<GlueSound>,
    hero: Query<&Hero>,
) {
    let dt = time.delta_secs();

    // Tick active toast duration
    if let Some((_, timer)) = &mut state.active_toast {
        *timer -= dt;
        if *timer <= 0.0 {
            state.active_toast = None;
        }
    }

    if hero.is_empty() {
        return;
    }

    let mut trigger = |id: &'static str| {
        if !account.has_achievement(id) {
            if let Some(def) = find_achievement(id) {
                if account.unlock_achievement(id, def.bounty) {
                    state.active_toast = Some((def, 4.5));
                    sfx.write(GlueSound("igQuestListOpen"));
                }
            }
        }
    };

    if run.bosses >= 1 {
        trigger("first_blood");
    }
    if run.elapsed >= 300.0 {
        trigger("survivor_5m");
    }
    if run.won {
        trigger("champion_15m");
    }
    if run.overtime && run.elapsed >= data::VICTORY_SECS + 120.0 {
        trigger("overtime_gladiator");
    }
    if run.won && run.affixes.active_count() == 4 {
        trigger("torment_conqueror");
    }
    if run.won && run.affixes.glass_cannon {
        trigger("glass_legend");
    }
    if !run.evolved.is_empty() {
        trigger("weapon_master");
    }
    if run.gold >= 2000 {
        trigger("vault_hoarder");
    }
    if run.kills >= 1000 {
        trigger("slayer_1000");
    }
    if run.won && selection.difficulty == data::Difficulty::Nightmare {
        trigger("nightmare_crusher");
    }
}

/// Triggers a manual event-based achievement unlock.
pub fn trigger_event_achievement(
    id: &'static str,
    account: &mut Account,
    state: &mut AchievementState,
    sfx: &mut MessageWriter<GlueSound>,
) {
    if !account.has_achievement(id) {
        if let Some(def) = find_achievement(id) {
            if account.unlock_achievement(id, def.bounty) {
                state.active_toast = Some((def, 4.5));
                sfx.write(GlueSound("igQuestListOpen"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twelve_achievements_defined() {
        assert_eq!(ACHIEVEMENTS.len(), 12);
        for a in ACHIEVEMENTS {
            assert!(!a.id.is_empty());
            assert!(!a.name.is_empty());
            assert!(a.bounty >= 100);
            assert!(a.icon_spell > 0);
        }
    }

    #[test]
    fn test_account_achievement_unlock() {
        let mut acc = Account::default();
        assert!(!acc.has_achievement("first_blood"));
        assert_eq!(acc.gold, 0);

        let unlocked = acc.unlock_achievement("first_blood", 100);
        assert!(unlocked);
        assert!(acc.has_achievement("first_blood"));
        assert_eq!(acc.gold, 100);

        // Cannot unlock twice
        let unlocked_again = acc.unlock_achievement("first_blood", 100);
        assert!(!unlocked_again);
        assert_eq!(acc.gold, 100);
    }
}
