//! Extraction Vault for Warcraft Survivors.
//!
//! Provides the in-run safe container for gear looted from bosses and chests.
//! Items placed in the Vault survive run termination and extract to persistent Account storage.
//! Ported from Hordes of Azeroth (`VanillaBonkVault.lua`).

#![allow(dead_code)]

use bevy::prelude::*;
use super::gear::GearItem;
use super::save::AccountSave;
use super::Rng;

pub const DEFAULT_VAULT_CAPACITY: usize = 2;
pub const MAX_VAULT_CAPACITY: usize = 4;

/// In-run extraction vault and temporary backpack.
#[derive(Resource, Debug, Clone, Default)]
pub struct ExtractionVault {
    pub capacity: usize,
    /// Items secured in the extraction vault. Will survive death and extract on victory.
    pub vaulted: Vec<GearItem>,
    /// Unsecured items found during the run. Lost on death if not vaulted!
    pub backpack: Vec<GearItem>,
    /// Whether the Vault UI modal is currently open.
    pub is_open: bool,
}

impl ExtractionVault {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(DEFAULT_VAULT_CAPACITY, MAX_VAULT_CAPACITY),
            vaulted: Vec::new(),
            backpack: Vec::new(),
            is_open: false,
        }
    }

    /// Moves an item from backpack into the safe vault if space is available.
    pub fn secure_item(&mut self, backpack_idx: usize) -> bool {
        if self.vaulted.len() >= self.capacity {
            return false;
        }
        if backpack_idx < self.backpack.len() {
            let item = self.backpack.remove(backpack_idx);
            self.vaulted.push(item);
            true
        } else {
            false
        }
    }

    /// Returns an item from the vault back to the backpack.
    pub fn unsecure_item(&mut self, vault_idx: usize) -> bool {
        if vault_idx < self.vaulted.len() {
            let item = self.vaulted.remove(vault_idx);
            self.backpack.push(item);
            true
        } else {
            false
        }
    }

    /// Adds looted gear to the run's backpack.
    pub fn add_loot(&mut self, item: GearItem) {
        // Automatically secure if there is open vault space
        if self.vaulted.len() < self.capacity {
            self.vaulted.push(item);
        } else {
            self.backpack.push(item);
        }
    }

    /// Deposits all secured vault items into the account armory upon extraction or victory.
    pub fn extract_to_account(&mut self, account: &mut AccountSave) {
        account.armory.extend(self.vaulted.drain(..));
    }
}

/// Rolls gear drops on boss kills or high-tier chest openings.
pub fn roll_boss_gear_drop(
    vault: &mut ExtractionVault,
    level: u32,
    banner: &mut Option<(String, f32)>,
    rng: &mut Rng,
    is_boss: bool,
) {
    if is_boss || rng.f32() < 0.25 {
        let quality_bias = if is_boss { 0.35 } else { 0.10 };
        let ilvl = (level + 5).max(10);
        let item = super::gear::generate_gear(rng, ilvl, quality_bias);
        *banner = Some((format!("Looted Gear: {}!", item.name), 3.5));
        vault.add_loot(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::gear::{EquipSlot, ItemQuality};
    use std::collections::HashMap;

    fn make_test_item(id: u32, name: &str) -> GearItem {
        GearItem {
            id,
            name: name.into(),
            slot: EquipSlot::Chest,
            quality: ItemQuality::Rare,
            item_level: 20,
            stats: HashMap::new(),
            display_id: 1,
            icon_spell: 100,
            set: None,
        }
    }

    #[test]
    fn test_vault_capacity_and_securing() {
        let mut vault = ExtractionVault::new(2);
        assert_eq!(vault.capacity, 2);

        let i1 = make_test_item(1, "Helm");
        let i2 = make_test_item(2, "Chest");
        let i3 = make_test_item(3, "Boots");

        // First two auto-vault
        vault.add_loot(i1);
        vault.add_loot(i2);
        assert_eq!(vault.vaulted.len(), 2);
        assert_eq!(vault.backpack.len(), 0);

        // Third item goes to backpack because vault is full
        vault.add_loot(i3);
        assert_eq!(vault.vaulted.len(), 2);
        assert_eq!(vault.backpack.len(), 1);

        // Cannot secure when full
        assert!(!vault.secure_item(0));

        // Unsecure one item
        assert!(vault.unsecure_item(0));
        assert_eq!(vault.vaulted.len(), 1);
        assert_eq!(vault.backpack.len(), 2);

        // Now can secure
        assert!(vault.secure_item(0));
        assert_eq!(vault.vaulted.len(), 2);
        assert_eq!(vault.backpack.len(), 1);
    }

    #[test]
    fn test_vault_extraction() {
        let mut vault = ExtractionVault::new(2);
        let mut account = AccountSave::default();

        vault.vaulted.push(make_test_item(1, "Legendary Sword"));
        vault.backpack.push(make_test_item(2, "Lost Boots"));

        vault.extract_to_account(&mut account);
        assert_eq!(account.armory.len(), 1);
        assert_eq!(account.armory[0].name, "Legendary Sword");
        assert_eq!(vault.vaulted.len(), 0);
        // Backpack item was NOT extracted!
        assert_eq!(vault.backpack.len(), 1);
    }
}
