//! Sayge's Glyphs & Darkmoon Deck Modifiers for Warcraft Survivors.
//!
//! Provides spell-altering glyph modifiers and the Darkmoon Faire reroll upgrades
//! ported from Hordes of Azeroth (`VanillaBonkGlyphs.lua`).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use super::Run;

/// A major or minor glyph modifying class abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GlyphKind {
    MovingBlizzard,   // Hurricane tracks the hero (-15% damage)
    PiercingShadow,   // Shadow Bolt pierces in a line (-20% damage)
    ConsecrationHeal, // Consecration heals hero 1.0% HP/s while inside
    RuptureBurst,     // Rupture DoT death burst deals +50% damage
    RewrittenFate,    // Awards +1 reroll per rank
}

impl GlyphKind {
    pub const ALL: [GlyphKind; 5] = [
        GlyphKind::MovingBlizzard,
        GlyphKind::PiercingShadow,
        GlyphKind::ConsecrationHeal,
        GlyphKind::RuptureBurst,
        GlyphKind::RewrittenFate,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::MovingBlizzard => "Glyph of the Moving Blizzard",
            Self::PiercingShadow => "Glyph of Piercing Shadow",
            Self::ConsecrationHeal => "Glyph of Consecration",
            Self::RuptureBurst => "Glyph of Rupture",
            Self::RewrittenFate => "Rewritten Fate",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::MovingBlizzard => "Hurricane ground area tracks your position (-15% damage).",
            Self::PiercingShadow => "Shadow Bolt pierces through enemies in a line (-20% damage).",
            Self::ConsecrationHeal => "Consecration heals the hero for 1.0% HP/s while standing in it.",
            Self::RuptureBurst => "Rupture death bursts deal +50% more damage to nearby enemies.",
            Self::RewrittenFate => "+1 draft card reroll per rank.",
        }
    }

    pub fn icon_spell(&self) -> u32 {
        match self {
            Self::MovingBlizzard => 42208,
            Self::PiercingShadow => 42210,
            Self::ConsecrationHeal => 26573,
            Self::RuptureBurst => 1943,
            Self::RewrittenFate => 2382,
        }
    }

    /// Applies any active glyph modifications to the run state.
    pub fn apply_to_run(&self, run: &mut Run, rank: u8) {
        if *self == Self::RewrittenFate {
            run.rerolls += rank as u32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewritten_fate_rerolls() {
        let mut run = Run::default();
        let initial_rerolls = run.rerolls;
        GlyphKind::RewrittenFate.apply_to_run(&mut run, 3);
        assert_eq!(run.rerolls, initial_rerolls + 3);
    }
}
