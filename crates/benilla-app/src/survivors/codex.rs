//! The Adventurer's Codex & Bestiary for Warcraft Survivors.
//!
//! Provides a browsable in-game grimoire of abilities, evolutions,
//! elite champion affixes, and zone encounter guides.
//! Ported from Hordes of Azeroth (`VanillaBonkCodex.lua`).

#![allow(dead_code)]

use super::evolutions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodexTab {
    Spells,
    Evolutions,
    Affixes,
    Bestiary,
}

#[derive(Debug, Clone)]
pub struct EvolutionEntry {
    pub evolved_name: &'static str,
    pub base_ability: &'static str,
    pub required_passive: &'static str,
    pub description: &'static str,
    pub icon_spell: u32,
}

#[derive(Debug, Clone)]
pub struct AffixEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub visual_color: &'static str,
}

#[derive(Debug, Clone)]
pub struct BestiaryEntry {
    pub zone_name: &'static str,
    pub level_range: &'static str,
    pub boss_name: &'static str,
    pub description: &'static str,
}

/// Retrieves all evolution pairing recipes.
pub fn get_evolution_recipes() -> Vec<EvolutionEntry> {
    evolutions::EVOLUTIONS
        .iter()
        .map(|evo| EvolutionEntry {
            evolved_name: evo.evolved_name,
            base_ability: evo.base_ability,
            required_passive: evo.required_passive.name(),
            description: evo.description,
            icon_spell: evo.icon_spell,
        })
        .collect()
}

/// Retrieves elite champion modifier descriptions.
pub fn get_champion_affixes() -> Vec<AffixEntry> {
    vec![
        AffixEntry {
            name: "Molten",
            description: "Leaves burning magma pools on the ground that deal periodic fire damage.",
            visual_color: "Orange / Fire",
        },
        AffixEntry {
            name: "Frozen",
            description: "Radiates an icy frost aura that slows nearby hero movement speed by 25%.",
            visual_color: "Cyan / Frost",
        },
        AffixEntry {
            name: "Vampiric",
            description: "Leeches 25% of all damage dealt to heal the champion.",
            visual_color: "Crimson / Blood",
        },
        AffixEntry {
            name: "Avenger",
            description: "Enrages when nearby allies die, permanently stacking movement speed and damage.",
            visual_color: "Yellow / Fury",
        },
        AffixEntry {
            name: "Shielded",
            description: "Possesses a reinforced damage barrier that absorbs the first 500 damage.",
            visual_color: "Gold / Divine",
        },
    ]
}

/// Retrieves zone bestiary summaries.
pub fn get_bestiary_entries() -> Vec<BestiaryEntry> {
    vec![
        BestiaryEntry {
            zone_name: "Durotar (Valley of Trials)",
            level_range: "Lvl 1 - 10",
            boss_name: "Gazz'uz the Shadowcaster",
            description: "Arid red canyons inhabited by scorpid swarms, quilboar raiders, and cultists.",
        },
        BestiaryEntry {
            zone_name: "Elwynn Forest (Northshire)",
            level_range: "Lvl 5 - 15",
            boss_name: "Hogger the Gnoll Chieftain",
            description: "Lush woodland infested by Defias bandits, forest wolves, and Riverpaw gnolls.",
        },
        BestiaryEntry {
            zone_name: "Westfall (Moonbrook)",
            level_range: "Lvl 10 - 20",
            boss_name: "Edwin VanCleef",
            description: "Desolate dust plains haunted by rogue harvest golems, vultures, and Defias pirates.",
        },
        BestiaryEntry {
            zone_name: "The Barrens (Crossroads)",
            level_range: "Lvl 15 - 25",
            boss_name: "Kreenig Snarlsnout",
            description: "Vast savanna plagued by centaur warbands, plainstriders, and thunder lizards.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codex_retrieval() {
        let evolutions = get_evolution_recipes();
        assert!(!evolutions.is_empty());

        let affixes = get_champion_affixes();
        assert_eq!(affixes.len(), 5);

        let bestiary = get_bestiary_entries();
        assert_eq!(bestiary.len(), 4);
    }
}
