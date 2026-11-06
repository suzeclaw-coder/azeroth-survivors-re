//! Persistent account state for Warcraft Survivors.
//!
//! Stores accumulated gold, talent/stat upgrades purchased between runs, and per-zone
//! high score survival records. Saved atomically to `survivors-account.toml` in the
//! local state home folder (`localStorage` on wasm32).

#![allow(dead_code)]

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Maximum rank for any account upgrade.
pub const MAX_UPGRADE_RANK: u8 = 5;

/// Bonus percentages and rates per upgrade rank.
pub const MIGHT_BONUS_PER_RANK: f32 = 0.05;
pub const VITALITY_BONUS_PER_RANK: f32 = 0.10;
pub const HASTE_BONUS_PER_RANK: f32 = 0.05;
pub const MAGNET_BONUS_PER_RANK: f32 = 1.5;
pub const DASH_CD_REDUCTION_PER_RANK: f32 = 0.3;
pub const GREED_BONUS_PER_RANK: f32 = 0.10;
pub const CHANNELER_BONUS_PER_RANK: f32 = 0.10;
pub const FORTUNE_BONUS_PER_RANK: f32 = 0.05;

/// A fallen hardcore legend recorded in the Graveyard.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(crate) struct FallenHero {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub race: u8,
    #[serde(default)]
    pub class: u8,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub zone: String,
    #[serde(default)]
    pub survived_secs: f32,
    #[serde(default)]
    pub killed_by: String,
    #[serde(default)]
    pub date_utc: String,
    #[serde(default)]
    pub highest_spree: u32,
}

/// Root account data persisted across game sessions.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(crate) struct AccountSave {
    #[serde(default)]
    pub gold: u64,
    #[serde(default)]
    pub upgrades: AccountUpgrades,
    #[serde(default)]
    pub high_scores: HashMap<String, ZoneRecord>,
    #[serde(default)]
    pub achievements: Vec<String>,
    #[serde(default)]
    pub graveyard: Vec<FallenHero>,
    #[serde(default)]
    pub armory: Vec<super::gear::GearItem>,
    #[serde(default)]
    pub equipped: HashMap<super::gear::EquipSlot, super::gear::GearItem>,
    #[serde(default)]
    pub transmog_unlocked: Vec<u32>,
    #[serde(default)]
    pub hide_helm: bool,
    #[serde(default)]
    pub hide_shoulders: bool,
    #[serde(default)]
    pub hide_cloak: bool,
    #[serde(default)]
    pub determination: HashMap<String, u8>,
    #[serde(default)]
    pub unlocked_titles: Vec<super::titles::TitleId>,
    #[serde(default)]
    pub active_title: Option<super::titles::TitleId>,
}

/// Meta-progression upgrades purchased with gold, capped at 5 ranks each.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub(crate) struct AccountUpgrades {
    #[serde(default)]
    pub might: u8,
    #[serde(default)]
    pub vitality: u8,
    #[serde(default)]
    pub haste: u8,
    #[serde(default)]
    pub magnet: u8,
    #[serde(default)]
    pub dash_cd: u8,
    #[serde(default)]
    pub greed: u8,
    #[serde(default)]
    pub channeler: u8,
    #[serde(default)]
    pub fortune: u8,
}

/// Best run record achieved in a specific zone.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(crate) struct ZoneRecord {
    #[serde(default)]
    pub best_survival_secs: f32,
    #[serde(default)]
    pub highest_level: u32,
    #[serde(default)]
    pub highest_difficulty: String,
}

/// Identifies each upgradeable stat in the meta-progression tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum UpgradeKind {
    Might,
    Vitality,
    Haste,
    Magnet,
    DashCd,
    Greed,
    Channeler,
    Fortune,
}

impl UpgradeKind {
    #[allow(non_upper_case_globals)]
    pub const DashCooldown: UpgradeKind = UpgradeKind::DashCd;

    pub const ALL: [UpgradeKind; 8] = [
        UpgradeKind::Might,
        UpgradeKind::Vitality,
        UpgradeKind::Haste,
        UpgradeKind::Magnet,
        UpgradeKind::DashCd,
        UpgradeKind::Greed,
        UpgradeKind::Channeler,
        UpgradeKind::Fortune,
    ];

    pub fn key(&self) -> &'static str {
        match self {
            UpgradeKind::Might => "might",
            UpgradeKind::Vitality => "vitality",
            UpgradeKind::Haste => "haste",
            UpgradeKind::Magnet => "magnet",
            UpgradeKind::DashCd => "dash_cd",
            UpgradeKind::Greed => "greed",
            UpgradeKind::Channeler => "channeler",
            UpgradeKind::Fortune => "fortune",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            UpgradeKind::Might => "Might",
            UpgradeKind::Vitality => "Vitality",
            UpgradeKind::Haste => "Haste",
            UpgradeKind::Magnet => "Magnet",
            UpgradeKind::DashCd => "Agility",
            UpgradeKind::Greed => "Greed",
            UpgradeKind::Channeler => "Channeler",
            UpgradeKind::Fortune => "Fortune",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            UpgradeKind::Might => "+5% damage per rank",
            UpgradeKind::Vitality => "+10% max HP per rank",
            UpgradeKind::Haste => "+5% attack speed / cooldown reduction per rank",
            UpgradeKind::Magnet => "+1.5 yd magnet radius per rank",
            UpgradeKind::DashCd => "-0.3s dash cooldown per rank",
            UpgradeKind::Greed => "+10% gold/xp per rank",
            UpgradeKind::Channeler => "+10% ultimate charge speed per rank",
            UpgradeKind::Fortune => "+5% chest drop luck & card rarity per rank",
        }
    }

    /// Gold cost to purchase a rank when the current rank is `current_rank` (0..4).
    /// Returns None if already at max rank (5).
    pub fn cost_for_rank(current_rank: u8) -> Option<u64> {
        match current_rank {
            0 => Some(100),
            1 => Some(250),
            2 => Some(500),
            3 => Some(1000),
            4 => Some(2000),
            _ => None,
        }
    }
}

impl AccountUpgrades {
    pub const MAX_RANK: u8 = MAX_UPGRADE_RANK;

    pub fn might_rank(&self) -> u8 {
        self.might.min(Self::MAX_RANK)
    }

    pub fn vitality_rank(&self) -> u8 {
        self.vitality.min(Self::MAX_RANK)
    }

    pub fn haste_rank(&self) -> u8 {
        self.haste.min(Self::MAX_RANK)
    }

    pub fn magnet_rank(&self) -> u8 {
        self.magnet.min(Self::MAX_RANK)
    }

    pub fn dash_cd_rank(&self) -> u8 {
        self.dash_cd.min(Self::MAX_RANK)
    }

    pub fn greed_rank(&self) -> u8 {
        self.greed.min(Self::MAX_RANK)
    }

    pub fn channeler_rank(&self) -> u8 {
        self.channeler.min(Self::MAX_RANK)
    }

    pub fn fortune_rank(&self) -> u8 {
        self.fortune.min(Self::MAX_RANK)
    }

    /// Returns the clamped rank (0..=5) for the given upgrade kind.
    pub fn get_rank(&self, kind: UpgradeKind) -> u8 {
        match kind {
            UpgradeKind::Might => self.might_rank(),
            UpgradeKind::Vitality => self.vitality_rank(),
            UpgradeKind::Haste => self.haste_rank(),
            UpgradeKind::Magnet => self.magnet_rank(),
            UpgradeKind::DashCd => self.dash_cd_rank(),
            UpgradeKind::Greed => self.greed_rank(),
            UpgradeKind::Channeler => self.channeler_rank(),
            UpgradeKind::Fortune => self.fortune_rank(),
        }
    }

    /// Sets the rank for the given upgrade kind, clamped to `MAX_RANK` (5).
    pub fn set_rank(&mut self, kind: UpgradeKind, rank: u8) {
        let rank = rank.min(Self::MAX_RANK);
        match kind {
            UpgradeKind::Might => self.might = rank,
            UpgradeKind::Vitality => self.vitality = rank,
            UpgradeKind::Haste => self.haste = rank,
            UpgradeKind::Magnet => self.magnet = rank,
            UpgradeKind::DashCd => self.dash_cd = rank,
            UpgradeKind::Greed => self.greed = rank,
            UpgradeKind::Channeler => self.channeler = rank,
            UpgradeKind::Fortune => self.fortune = rank,
        }
    }

    /// Might damage bonus: +5% damage per rank.
    pub fn might_bonus(&self) -> f32 {
        self.might_rank() as f32 * MIGHT_BONUS_PER_RANK
    }

    /// Might damage multiplier: 1.0 + might_bonus (e.g. rank 1 = 1.05, rank 5 = 1.25).
    pub fn damage_mult(&self) -> f32 {
        1.0 + self.might_bonus()
    }

    /// Vitality bonus: +10% max HP per rank.
    pub fn vitality_bonus(&self) -> f32 {
        self.vitality_rank() as f32 * VITALITY_BONUS_PER_RANK
    }

    /// Vitality max HP multiplier: 1.0 + vitality_bonus (e.g. rank 1 = 1.10, rank 5 = 1.50).
    pub fn max_hp_mult(&self) -> f32 {
        1.0 + self.vitality_bonus()
    }

    /// Haste bonus: +5% attack speed / CDR per rank.
    pub fn haste_bonus(&self) -> f32 {
        self.haste_rank() as f32 * HASTE_BONUS_PER_RANK
    }

    /// Haste attack speed multiplier: 1.0 + haste_bonus (e.g. rank 1 = 1.05, rank 5 = 1.25).
    pub fn haste_mult(&self) -> f32 {
        1.0 + self.haste_bonus()
    }

    /// Cooldown multiplier reduction factor: 1.0 / (1.0 + haste_bonus).
    pub fn cooldown_factor(&self) -> f32 {
        1.0 / (1.0 + self.haste_bonus())
    }

    /// Magnet bonus radius in yards: +1.5 yd per rank.
    pub fn magnet_bonus_yards(&self) -> f32 {
        self.magnet_rank() as f32 * MAGNET_BONUS_PER_RANK
    }

    /// Dash cooldown reduction in seconds: -0.3s per rank.
    pub fn dash_cd_reduction_secs(&self) -> f32 {
        self.dash_cd_rank() as f32 * DASH_CD_REDUCTION_PER_RANK
    }

    /// Greed bonus: +10% gold/xp per rank.
    pub fn greed_bonus(&self) -> f32 {
        self.greed_rank() as f32 * GREED_BONUS_PER_RANK
    }

    /// Greed gold/xp multiplier: 1.0 + greed_bonus (e.g. rank 1 = 1.10, rank 5 = 1.50).
    pub fn greed_mult(&self) -> f32 {
        1.0 + self.greed_bonus()
    }

    /// Channeler bonus: +10% ultimate charge speed per rank.
    pub fn channeler_bonus(&self) -> f32 {
        self.channeler_rank() as f32 * CHANNELER_BONUS_PER_RANK
    }

    /// Ultimate charge multiplier: 1.0 + channeler_bonus (e.g. rank 1 = 1.10, rank 5 = 1.50).
    pub fn ultimate_charge_mult(&self) -> f32 {
        1.0 + self.channeler_bonus()
    }

    /// Fortune bonus: +5% chest drop luck and card rarity per rank.
    pub fn fortune_bonus(&self) -> f32 {
        self.fortune_rank() as f32 * FORTUNE_BONUS_PER_RANK
    }
}

/// Resolves the absolute path to `survivors-account.toml` in the benilla state folder.
pub(crate) fn account_path() -> Option<PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-account.toml"))
}

impl AccountSave {
    /// Loads account state from `survivors-account.toml`.
    ///
    /// If the file is missing or invalid, falls back to `AccountSave::default()`.
    pub(crate) fn load() -> Self {
        let Some(path) = account_path() else {
            return Self::default();
        };
        let Ok(text) = crate::local_state::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }

    /// Serializes the account to pretty TOML and writes atomically to `survivors-account.toml`.
    pub(crate) fn save(&self) -> std::io::Result<()> {
        let path = account_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "local_state home unavailable")
        })?;
        let toml_str = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::local_state::write_atomic(&path, &toml_str)
    }

    /// Adds gold to the account and persists to disk.
    pub(crate) fn add_gold(&mut self, amount: u64) {
        self.gold = self.gold.saturating_add(amount);
        let _ = self.save();
    }

    /// Returns the current rank of an upgrade kind.
    pub(crate) fn upgrade_rank(&self, kind: UpgradeKind) -> u8 {
        self.upgrades.get_rank(kind)
    }

    /// Gold cost for the next rank of an upgrade, or None if already maxed (rank 5).
    pub(crate) fn upgrade_cost(&self, kind: UpgradeKind) -> Option<u64> {
        let rank = self.upgrade_rank(kind);
        UpgradeKind::cost_for_rank(rank)
    }

    /// Checks if the next rank of `kind` can be purchased with the current gold.
    pub(crate) fn can_purchase_upgrade(&self, kind: UpgradeKind) -> bool {
        match self.upgrade_cost(kind) {
            Some(cost) => self.gold >= cost,
            None => false,
        }
    }

    /// Purchases the next rank of `kind` if affordable and under max rank (5).
    /// Deducts gold, increments rank, persists to disk, and returns true.
    /// Returns false if unaffordable or already at max rank.
    pub(crate) fn purchase_upgrade(&mut self, kind: UpgradeKind) -> bool {
        let Some(cost) = self.upgrade_cost(kind) else {
            return false;
        };
        if self.gold < cost {
            return false;
        }
        self.gold -= cost;
        let rank = self.upgrade_rank(kind);
        self.upgrades.set_rank(kind, rank + 1);
        let _ = self.save();
        true
    }

    /// Resets all upgrades to rank 0, refunds the total gold spent, persists to disk,
    /// and returns the total refunded gold amount.
    pub(crate) fn refund_upgrades(&mut self) -> u64 {
        let mut refund = 0u64;
        for &kind in &UpgradeKind::ALL {
            let rank = self.upgrades.get_rank(kind);
            for r in 0..rank {
                if let Some(cost) = UpgradeKind::cost_for_rank(r) {
                    refund = refund.saturating_add(cost);
                }
            }
            self.upgrades.set_rank(kind, 0);
        }
        self.gold = self.gold.saturating_add(refund);
        let _ = self.save();
        refund
    }

    /// Records run results for a zone. If survival seconds, highest level, or
    /// difficulty rank improves, updates the zone record and persists to disk.
    /// Returns true if any metric was improved.
    pub(crate) fn record_run_result(
        &mut self,
        zone: &str,
        survival_secs: f32,
        level: u32,
        difficulty: &str,
    ) -> bool {
        let record = self.high_scores.entry(zone.to_string()).or_default();
        let mut improved = false;
        if survival_secs > record.best_survival_secs {
            record.best_survival_secs = survival_secs;
            improved = true;
        }
        if level > record.highest_level {
            record.highest_level = level;
            improved = true;
        }
        if record.highest_difficulty.is_empty()
            || difficulty_tier(difficulty) > difficulty_tier(&record.highest_difficulty)
        {
            record.highest_difficulty = difficulty.to_string();
            improved = true;
        }
        if improved {
            let _ = self.save();
        }
        improved
    }

    /// Checks if an achievement has already been unlocked.
    pub(crate) fn has_achievement(&self, id: &str) -> bool {
        self.achievements.iter().any(|a| a == id)
    }

    /// Unlocks an achievement, grants bounty gold, persists to disk, and returns true if newly unlocked.
    pub(crate) fn unlock_achievement(&mut self, id: &str, bounty: u64) -> bool {
        if self.has_achievement(id) {
            return false;
        }
        self.achievements.push(id.to_string());
        self.gold = self.gold.saturating_add(bounty);
        let _ = self.save();
        true
    }

    /// Equips an item from the armory into its designated slot.
    /// If an item was already equipped in that slot, it is returned to the armory.
    pub(crate) fn equip_item(&mut self, armory_idx: usize) -> bool {
        if armory_idx < self.armory.len() {
            let item = self.armory.remove(armory_idx);
            let slot = item.slot;
            if !self.transmog_unlocked.contains(&item.display_id) {
                self.transmog_unlocked.push(item.display_id);
            }
            if let Some(old) = self.equipped.insert(slot, item) {
                self.armory.push(old);
            }
            let _ = self.save();
            true
        } else {
            false
        }
    }

    /// Unequips an item from a slot and returns it to the armory.
    pub(crate) fn unequip_item(&mut self, slot: super::gear::EquipSlot) -> bool {
        if let Some(item) = self.equipped.remove(&slot) {
            self.armory.push(item);
            let _ = self.save();
            true
        } else {
            false
        }
    }

    /// Salvages an item from the armory for gold, unlocking its transmog appearance.
    pub(crate) fn salvage_item(&mut self, armory_idx: usize) -> Option<u64> {
        if armory_idx < self.armory.len() {
            let item = self.armory.remove(armory_idx);
            if !self.transmog_unlocked.contains(&item.display_id) {
                self.transmog_unlocked.push(item.display_id);
            }
            let val = item.salvage_value();
            self.gold = self.gold.saturating_add(val);
            let _ = self.save();
            Some(val)
        } else {
            None
        }
    }

    /// Returns the active title definition, if one is set.
    pub(crate) fn active_title_def(&self) -> Option<&'static super::titles::TitleDef> {
        self.active_title.and_then(super::titles::get_title_def)
    }

    /// Unlocks a title and persists to disk if newly unlocked.
    pub(crate) fn unlock_title(&mut self, id: super::titles::TitleId) -> bool {
        if !self.unlocked_titles.contains(&id) {
            self.unlocked_titles.push(id);
            let _ = self.save();
            true
        } else {
            false
        }
    }

    /// Sets the currently active title (or None to clear) and saves.
    pub(crate) fn set_active_title(&mut self, id: Option<super::titles::TitleId>) {
        self.active_title = id;
        let _ = self.save();
    }

    /// Gets current determination pity stacks for a difficulty tier (0..=5).
    pub(crate) fn determination_for(&self, diff: &str) -> u8 {
        self.determination.get(diff).copied().unwrap_or(0).min(5)
    }

    /// Adds a determination stack on wipe for this difficulty (capped at 5).
    pub(crate) fn add_determination(&mut self, diff: &str) {
        let entry = self.determination.entry(diff.to_string()).or_insert(0);
        *entry = (*entry + 1).min(5);
        let _ = self.save();
    }

    /// Resets determination stacks back to 0 on victory for this difficulty.
    pub(crate) fn reset_determination(&mut self, diff: &str) {
        if let Some(entry) = self.determination.get_mut(diff) {
            *entry = 0;
            let _ = self.save();
        }
    }
}

/// Numerical rank for difficulty tiers used for comparison in high score records.
fn difficulty_tier(diff: &str) -> u8 {
    match diff.trim().to_ascii_lowercase().as_str() {
        "nightmare" => 4,
        "mythic" => 3,
        "heroic" => 2,
        "normal" => 1,
        _ => 0,
    }
}

/// Bevy resource holding the account save state.
#[derive(Resource, Default, Debug, Clone)]
pub(crate) struct Account(pub AccountSave);

impl Deref for Account {
    type Target = AccountSave;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Account {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Account {
    /// Loads the account from disk.
    pub(crate) fn load() -> Self {
        Self(AccountSave::load())
    }

    /// Saves the account to disk.
    pub(crate) fn save(&self) -> std::io::Result<()> {
        self.0.save()
    }

    /// Adds gold and persists to disk.
    pub(crate) fn add_gold(&mut self, amount: u64) {
        self.0.add_gold(amount);
    }

    /// Purchases an upgrade rank and persists to disk.
    pub(crate) fn purchase_upgrade(&mut self, kind: UpgradeKind) -> bool {
        self.0.purchase_upgrade(kind)
    }

    /// Resets all upgrades to rank 0, refunds gold spent, and persists to disk.
    pub(crate) fn refund_upgrades(&mut self) -> u64 {
        self.0.refund_upgrades()
    }

    /// Current rank of an upgrade kind.
    pub(crate) fn upgrade_rank(&self, kind: UpgradeKind) -> u8 {
        self.0.upgrade_rank(kind)
    }

    /// Gold cost for the next rank of an upgrade, or None if already maxed.
    pub(crate) fn upgrade_cost(&self, kind: UpgradeKind) -> Option<u64> {
        self.0.upgrade_cost(kind)
    }

    /// Checks if the next rank can be purchased with the current gold.
    pub(crate) fn can_purchase_upgrade(&self, kind: UpgradeKind) -> bool {
        self.0.can_purchase_upgrade(kind)
    }

    /// Records run results and persists to disk if improved.
    pub(crate) fn record_run_result(
        &mut self,
        zone: &str,
        survival_secs: f32,
        level: u32,
        difficulty: &str,
    ) -> bool {
        self.0.record_run_result(zone, survival_secs, level, difficulty)
    }
}

/// Startup system: loads the persisted account from disk into the `Account` resource.
pub(crate) fn load_account(mut account: ResMut<Account>) {
    account.0 = AccountSave::load();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_account_is_empty() {
        let acc = AccountSave::default();
        assert_eq!(acc.gold, 0);
        assert_eq!(acc.upgrades.might, 0);
        assert_eq!(acc.upgrades.vitality, 0);
        assert_eq!(acc.upgrades.haste, 0);
        assert_eq!(acc.upgrades.magnet, 0);
        assert_eq!(acc.upgrades.dash_cd, 0);
        assert_eq!(acc.upgrades.greed, 0);
        assert_eq!(acc.upgrades.channeler, 0);
        assert_eq!(acc.upgrades.fortune, 0);
        assert!(acc.high_scores.is_empty());
    }

    #[test]
    fn upgrade_bonus_calculations() {
        let mut u = AccountUpgrades::default();
        u.might = 2;
        u.vitality = 3;
        u.haste = 1;
        u.magnet = 4;
        u.dash_cd = 5;
        u.greed = 2;
        u.channeler = 3;
        u.fortune = 4;

        assert!((u.might_bonus() - 0.10).abs() < 1e-5);
        assert!((u.damage_mult() - 1.10).abs() < 1e-5);
        assert!((u.vitality_bonus() - 0.30).abs() < 1e-5);
        assert!((u.max_hp_mult() - 1.30).abs() < 1e-5);
        assert!((u.haste_bonus() - 0.05).abs() < 1e-5);
        assert!((u.haste_mult() - 1.05).abs() < 1e-5);
        assert!((u.magnet_bonus_yards() - 6.0).abs() < 1e-5);
        assert!((u.dash_cd_reduction_secs() - 1.5).abs() < 1e-5);
        assert!((u.greed_bonus() - 0.20).abs() < 1e-5);
        assert!((u.greed_mult() - 1.20).abs() < 1e-5);
        assert!((u.channeler_bonus() - 0.30).abs() < 1e-5);
        assert!((u.ultimate_charge_mult() - 1.30).abs() < 1e-5);
        assert!((u.fortune_bonus() - 0.20).abs() < 1e-5);
    }

    #[test]
    fn upgrade_clamped_to_max_rank() {
        let mut u = AccountUpgrades::default();
        u.set_rank(UpgradeKind::Might, 10);
        assert_eq!(u.might_rank(), 5);
        assert_eq!(u.might, 5);
        assert!((u.might_bonus() - 0.25).abs() < 1e-5);
        assert!((u.damage_mult() - 1.25).abs() < 1e-5);
    }

    #[test]
    fn toml_serialization_round_trip() {
        let mut acc = AccountSave::default();
        acc.gold = 500;
        acc.upgrades.might = 2;
        acc.high_scores.insert(
            "Barrens".to_string(),
            ZoneRecord {
                best_survival_secs: 420.5,
                highest_level: 25,
                highest_difficulty: "Heroic".to_string(),
            },
        );

        let serialized = toml::to_string_pretty(&acc).unwrap();
        let deserialized: AccountSave = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.gold, 500);
        assert_eq!(deserialized.upgrades.might, 2);
        let rec = deserialized.high_scores.get("Barrens").unwrap();
        assert!((rec.best_survival_secs - 420.5).abs() < 1e-5);
        assert_eq!(rec.highest_level, 25);
        assert_eq!(rec.highest_difficulty, "Heroic");
    }

    #[test]
    fn purchase_and_refund_flow() {
        let mut acc = AccountSave::default();
        acc.gold = 350; // enough for rank 1 (100) and rank 2 (250) of Might
        assert_eq!(acc.upgrade_cost(UpgradeKind::Might), Some(100));
        assert!(acc.can_purchase_upgrade(UpgradeKind::Might));
        assert!(acc.purchase_upgrade(UpgradeKind::Might));
        assert_eq!(acc.gold, 250);
        assert_eq!(acc.upgrades.might, 1);

        assert_eq!(acc.upgrade_cost(UpgradeKind::Might), Some(250));
        assert!(acc.purchase_upgrade(UpgradeKind::Might));
        assert_eq!(acc.gold, 0);
        assert_eq!(acc.upgrades.might, 2);

        // Can't afford rank 3 (500)
        assert_eq!(acc.upgrade_cost(UpgradeKind::Might), Some(500));
        assert!(!acc.can_purchase_upgrade(UpgradeKind::Might));
        assert!(!acc.purchase_upgrade(UpgradeKind::Might));

        // Refund gives back 350 gold and resets rank to 0
        let refunded = acc.refund_upgrades();
        assert_eq!(refunded, 350);
        assert_eq!(acc.gold, 350);
        assert_eq!(acc.upgrades.might, 0);
    }

    #[test]
    fn record_run_result_improvements() {
        let mut acc = AccountSave::default();
        assert!(acc.record_run_result("Barrens", 300.0, 15, "Normal"));
        let rec = acc.high_scores.get("Barrens").unwrap();
        assert_eq!(rec.best_survival_secs, 300.0);
        assert_eq!(rec.highest_level, 15);
        assert_eq!(rec.highest_difficulty, "Normal");

        // Better time and level
        assert!(acc.record_run_result("Barrens", 350.0, 18, "Normal"));
        let rec = acc.high_scores.get("Barrens").unwrap();
        assert_eq!(rec.best_survival_secs, 350.0);
        assert_eq!(rec.highest_level, 18);

        // Worse time, but higher difficulty
        assert!(acc.record_run_result("Barrens", 200.0, 10, "Heroic"));
        let rec = acc.high_scores.get("Barrens").unwrap();
        assert_eq!(rec.best_survival_secs, 350.0);
        assert_eq!(rec.highest_level, 18);
        assert_eq!(rec.highest_difficulty, "Heroic");
    }

    #[test]
    fn determination_pity_system_flow() {
        let mut acc = AccountSave::default();
        assert_eq!(acc.determination_for("Heroic"), 0);

        // Wipe 1
        acc.add_determination("Heroic");
        assert_eq!(acc.determination_for("Heroic"), 1);

        // Wipe up to 6 times (should cap at 5)
        for _ in 0..5 {
            acc.add_determination("Heroic");
        }
        assert_eq!(acc.determination_for("Heroic"), 5);

        // Different difficulty is unaffected
        assert_eq!(acc.determination_for("Mythic"), 0);

        // Victory resets to 0
        acc.reset_determination("Heroic");
        assert_eq!(acc.determination_for("Heroic"), 0);
    }

    #[test]
    fn title_unlock_and_active_perk() {
        use crate::survivors::titles::TitleId;
        use crate::survivors::gear::StatType;

        let mut acc = AccountSave::default();
        assert!(acc.unlocked_titles.is_empty());
        assert!(acc.active_title.is_none());

        assert!(acc.unlock_title(TitleId::TheImmortal));
        assert!(!acc.unlock_title(TitleId::TheImmortal)); // idempotent
        assert_eq!(acc.unlocked_titles.len(), 1);

        acc.set_active_title(Some(TitleId::TheImmortal));
        let def = acc.active_title_def().expect("should have active title");
        assert_eq!(def.title, "the Immortal");
        assert_eq!(def.stat, StatType::Stamina);
    }
}
