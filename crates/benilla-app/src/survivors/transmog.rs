//! Transmogrification, Wardrobe, and Cosmetic Appearance for Warcraft Survivors.
//!
//! Provides unlocked display appearance tracking, hide helm/shoulders toggles,
//! and cosmetic tabards ported from Hordes of Azeroth (`VanillaBonkTransmog.lua`).

#![allow(dead_code)]


use super::save::AccountSave;

/// Iconic cosmetic tabards available for unlocking and equipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CosmeticTabard {
    ScarletCrusade,
    ArgentDawn,
    DarkmoonFaire,
    Flame,
    Frost,
}

impl CosmeticTabard {
    pub const ALL: [CosmeticTabard; 5] = [
        CosmeticTabard::ScarletCrusade,
        CosmeticTabard::ArgentDawn,
        CosmeticTabard::DarkmoonFaire,
        CosmeticTabard::Flame,
        CosmeticTabard::Frost,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::ScarletCrusade => "Tabard of the Scarlet Crusade",
            Self::ArgentDawn => "Argent Dawn Tabard",
            Self::DarkmoonFaire => "Darkmoon Faire Tabard",
            Self::Flame => "Tabard of Flame",
            Self::Frost => "Tabard of Frost",
        }
    }

    pub fn display_id(&self) -> u32 {
        match self {
            Self::ScarletCrusade => 31128,
            Self::ArgentDawn => 31129,
            Self::DarkmoonFaire => 31130,
            Self::Flame => 31131,
            Self::Frost => 31132,
        }
    }
}

/// Unlocks an item display ID in the player's account wardrobe.
pub fn unlock_transmog(account: &mut AccountSave, display_id: u32) {
    if display_id > 0 && !account.transmog_unlocked.contains(&display_id) {
        account.transmog_unlocked.push(display_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transmog_unlock() {
        let mut account = AccountSave::default();
        unlock_transmog(&mut account, 12345);
        assert!(account.transmog_unlocked.contains(&12345));
        // Idempotent
        unlock_transmog(&mut account, 12345);
        assert_eq!(account.transmog_unlocked.len(), 1);
    }
}
