//! Warcraft Survivors: a top-down survival mode on the benilla world, started by
//! [`crate::run_survivors`]. There is no server: the hero, the enemies and the spells are local
//! entities dressed by the same attach, animation and spell-visual systems a streamed unit gets,
//! on a battleground picked at the menu ([`data::MAPS`]), with the player controller and FrameXML
//! off ([`crate::run_mode::CaptureMode`]).

mod abilities;
pub mod achievements;
mod auto;
pub mod breakables;
mod cards;
pub(crate) mod chests;
pub mod codex;
pub mod combo;
mod data;
mod enemies;
pub mod enchanter;
pub mod events;
pub mod evolutions;
pub mod fct;
mod fill;
pub mod gear;
pub mod glyphs;
mod grade;
pub mod hazards;
pub mod hero;
pub mod hub;
mod hud;
mod menu;
mod minimap;
pub mod minions;
mod mobility;
pub(crate) mod save;
pub mod sheet;
mod shrines;
pub mod skillshot;
pub mod specs;
mod telegraphs;
pub mod titles;
pub(super) mod traits;
pub mod trinkets;
pub mod transmog;
pub(crate) mod ultimates;
mod units;
pub mod vault;

use std::collections::HashMap;

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

use data::{AbilityDef, ClassDef, PassiveDef, Stat};

/// Set by [`crate::run_survivors`] before the app builds. wasm32 has no process environment, so
/// the flag lives here; `WOW_SURVIVORS` (`?survivors=1` / `__wenilla_env.survivors` on the web)
/// still turns it on from outside.
pub(crate) static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
        || crate::webenv::var("WOW_SURVIVORS").is_some()
}

pub(crate) struct SurvivorsPlugin;

impl Plugin for SurvivorsPlugin {
    fn build(&self, app: &mut App) {
        // Present before `OnEnter(InWorld)`, where the UI lifecycle decides not to load FrameXML.
        app.insert_resource(crate::run_mode::CaptureMode)
            .add_plugins(grade::plugin)
            .init_state::<Phase>()
            .init_resource::<Rng>()
            .init_resource::<Selection>()
            .init_resource::<Run>()
            .init_resource::<save::Account>()
            .init_resource::<hud::HudFont>()
            .init_resource::<hero::CamRig>()
            .init_resource::<hero::AimPoint>()
            .init_resource::<abilities::PendingHits>()
            .init_resource::<enemies::Spawner>()
            .init_resource::<hud::Offer>()
            .init_resource::<cards::PendingPick>()
            .init_resource::<cards::BanishFlash>()
            .init_resource::<telegraphs::TelegraphCache>()
            .init_resource::<events::WorldEventState>()
            .init_resource::<achievements::AchievementState>()
            .init_resource::<hazards::HazardState>()
            .init_resource::<hero::RangeRingSettings>()
            .init_resource::<hub::HubState>()
            .add_message::<abilities::Hit>()
            .add_message::<enemies::Loot>()
            .add_plugins(fct::FctPlugin)
            .add_plugins(bevy::ui_render::UiMaterialPlugin::<hud::SweepUiMaterial>::default())
            .add_plugins(bevy::ui_render::UiMaterialPlugin::<fill::FillMaterial>::default())
            .add_systems(
                Startup,
                (
                    hud::setup_hud,
                    sheet::setup_character_sheet,
                    hub::setup_hub_ui,
                    save::load_account,
                ),
            )
            .add_systems(Update, hero::daylight)
            .add_systems(
                Update,
                (
                    hero::drop_stray_buttons,
                    hero::aim_cursor,
                    mobility::cast_mobility,
                    skillshot::cast_skillshot,
                    ultimates::cast_ultimate,
                    hero::drive_hero,
                    hero::place_camera,
                    hero::update_range_rings,
                )
                    .chain()
                    .in_set(WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    (
                        enemies::spawn_enemies,
                        enemies::move_enemies,
                        enemies::tick_molten_fire,
                        enemies::tick_boss_arena,
                        hazards::tick_hazards,
                        achievements::check_achievements,
                        telegraphs::update_telegraphs,
                        shrines::tick_shrines,
                        chests::tick_chests,
                        breakables::break_barrels,
                        breakables::collect_powerups,
                    ),
                    (
                        combo::tick_combo,
                        trinkets::tick_trinkets,
                        minions::tick_minions,
                        breakables::vacuum_magnet_gems,
                        events::tick_world_events,
                        traits::tick_racial_traits,
                        abilities::cast_abilities,
                        skillshot::tick_skillshot,
                        evolutions::tick_evolutions,
                        ultimates::tick_ultimates,
                    ),
                    (
                        abilities::tick_areas,
                        abilities::apply_hits,
                        enemies::tick_marks,
                        enemies::drop_loot,
                        enemies::collect_gems,
                        hero::hero_vitals,
                    ),
                )
                    .chain()
                    .after(WorldStage::Input)
                    .run_if(in_state(Phase::Playing))
                    // Entering a battleground raises its loading screen: the run waits behind it.
                    .run_if(|l: Res<crate::loading_screen::LoadingScreen>| !l.covering()),
            )
            .add_systems(
                Update,
                (enemies::reap_dead, enemies::still_gems).after(WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    hub::tick_hub,
                    hub::hub_input,
                    hub::hub_ui_buttons,
                    hub::refresh_hub_chrome,
                    hub::refresh_hub_modals,
                )
                    .chain()
                    .after(WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    (
                        hero::follow_map,
                        hud::menu_preview,
                        hud::buttons,
                        hud::keys,
                        sheet::sheet_keys,
                        sheet::sheet_buttons,
                        hud::refresh_menu,
                        menu::restore_pick,
                        menu::build_menu,
                        menu::menu_clicks,
                        menu::refresh_menu_ui.before(crate::glue::GlueVisuals),
                        cards::deal_cards,
                    )
                        .chain(),
                    (
                        (
                            cards::choose_card,
                            cards::animate_cards,
                            hud::refresh_hud,
                            hud::refresh_boss_bar,
                            hud::refresh_meter,
                            hud::refresh_results,
                            hud::fill_icons,
                            hud::update_cooldown_sweeps,
                        ),
                        (
                            hud::refresh_dash,
                            hud::refresh_skillshot,
                            hud::refresh_ultimate,
                            sheet::refresh_character_sheet,
                            hud::refresh_blood_moon,
                            hud::refresh_critical_vignette,
                            hud::refresh_achievement_toast,
                            hud::refresh_combo_hud,
                            hud::refresh_trinkets_hud,
                            hud::refresh_shrine_buff,
                            hud::refresh_mobility,
                            hud::refresh_draft_buttons,
                            hud::show_panels,
                        ),
                    )
                        .chain(),
                )
                    .chain()
                    .after(WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    hud::apply_ui_scale,
                    hud::fill_art,
                    hud::xp_hover,
                    hud::icon_tooltip,
                    hud::hero_plate,
                    minimap::map_buttons,
                    minimap::zone_text,
                    minimap::compose,
                )
                    .chain()
                    .after(hud::show_panels),
            )
            .add_systems(
                OnEnter(Phase::Playing),
                (
                    unpause,
                    shrines::spawn_shrines,
                    breakables::spawn_breakables,
                    minions::setup_starting_companions,
                ),
            )
            .add_systems(OnEnter(Phase::LevelUp), (pause, hud::roll_offer))
            .add_systems(OnEnter(Phase::Paused), pause)
            .add_systems(OnEnter(Phase::PortalChoice), pause)
            .add_systems(OnEnter(Phase::Hub), (unpause, hub::enter_hub))
            .add_systems(OnEnter(Phase::GameOver), (unpause, record_game_over))
            .add_systems(OnEnter(Phase::Menu), (unpause, reset_run));
        if let Some(auto) = auto::from_env() {
            app.insert_resource(auto)
                .add_systems(
                    Update,
                    auto::drive.after(hud::keys).before(hud::refresh_menu),
                )
                .add_systems(Update, auto::plan.before(WorldStage::Input));
        }
    }
}

/// The screen the mode is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    #[default]
    Menu,
    Playing,
    LevelUp,
    Paused,
    PortalChoice,
    GameOver,
    Hub,
}

/// Everything spawned for a run, despawned when the next begins.
#[derive(Component)]
pub(super) struct RunEntity;

/// The class, look and battleground picked at the menu; the battleground holds for the run.
#[derive(Resource, Default)]
pub(super) struct Selection {
    pub class: Option<usize>,
    /// `(display, race, sex)`, one per race and sex the class's trainers come in.
    pub looks: Vec<(u32, u8, u8)>,
    pub look: Option<usize>,
    /// An index into [`data::MAPS`].
    pub map: usize,
    /// The battleground the world is standing in: the last one entered (or the saved pick at
    /// boot). Picking another at the menu changes only `map`, so the menu hero stays put and no
    /// loading screen rises until Enter World.
    pub stage: usize,
    pub difficulty: data::Difficulty,
    pub affixes: data::ChallengeAffixes,
    pub is_hardcore: bool,
    /// Bumped on every change, so the menu rebuilds once.
    pub version: u32,
}

impl Selection {
    pub fn class_def(&self) -> Option<&'static ClassDef> {
        self.class.map(|i| &data::CLASSES[i])
    }

    pub fn map_def(&self) -> &'static data::MapDef {
        &data::MAPS[self.map.min(data::MAPS.len() - 1)]
    }

    pub fn stage_def(&self) -> &'static data::MapDef {
        &data::MAPS[self.stage.min(data::MAPS.len() - 1)]
    }
}

/// An ability the hero has, with its own cooldown.
pub(super) struct Owned {
    pub def: &'static AbilityDef,
    pub level: u8,
    pub cd: f32,
}

impl Owned {
    pub fn damage_mult(&self) -> f32 {
        1.0 + 0.3 * f32::from(self.level - 1)
    }
    pub fn cooldown(&self, stats: &Stats) -> f32 {
        let base = self.def.cooldown * 0.93f32.powi(i32::from(self.level) - 1);
        (base * (1.0 - stats.haste).max(0.35)).max(0.2)
    }
    pub fn count(&self) -> u32 {
        let extra = [3u8, 5, 7].iter().filter(|&&l| self.level >= l).count() as u32;
        self.def.count + extra
    }
    pub fn radius(&self, stats: &Stats) -> f32 {
        self.def.radius * (1.0 + 0.08 * f32::from(self.level - 1)) * stats.area
    }
    pub fn duration(&self) -> f32 {
        self.def.duration * (1.0 + 0.1 * f32::from(self.level - 1))
    }
}

/// The hero's derived numbers, from the base and the passives.
#[derive(Clone, Copy, Default)]
pub(super) struct Stats {
    pub might: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub regen: f32,
    pub haste: f32,
    pub area: f32,
    pub magnet: f32,
    pub growth: f32,
    pub armor: f32,
    pub crit: f32,
    pub xp_bonus: f32,
    pub gold_bonus: f32,
    pub evasion: f32,
    pub stun_res: f32,
}

pub(super) const BASE_HP: f32 = 120.0;
pub(super) const BASE_SPEED: f32 = 7.0;
pub(super) const BASE_MAGNET: f32 = 4.5;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum MeterSort {
    #[default]
    Damage,
    Dps,
    Kills,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum MeterState {
    #[default]
    Expanded,
    Mini,
    Hidden,
}

#[derive(Clone, Debug, Default)]
pub(super) struct SpellStats {
    pub spell: u32,
    pub key: &'static str,
    pub damage: f64,
    pub kills: u32,
    pub hits: u32,
    pub crits: u32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Meter {
    pub spells: HashMap<u32, SpellStats>,
    pub sort: MeterSort,
    pub state: MeterState,
    pub version: u32,
}

impl Meter {
    pub fn record_hit(&mut self, spell: u32, key: &'static str, dmg: f32, crit: bool) {
        let spell = if spell == 0 { 6603 } else { spell };
        let entry = self.spells.entry(spell).or_insert_with(|| SpellStats {
            spell,
            key: if key.is_empty() { "attack" } else { key },
            damage: 0.0,
            kills: 0,
            hits: 0,
            crits: 0,
        });
        entry.damage += f64::from(dmg);
        entry.hits += 1;
        if crit {
            entry.crits += 1;
        }
        self.version = self.version.wrapping_add(1);
    }

    pub fn record_kill(&mut self, spell: u32, key: &'static str) {
        let spell = if spell == 0 { 6603 } else { spell };
        let entry = self.spells.entry(spell).or_insert_with(|| SpellStats {
            spell,
            key: if key.is_empty() { "attack" } else { key },
            damage: 0.0,
            kills: 0,
            hits: 0,
            crits: 0,
        });
        entry.kills += 1;
        self.version = self.version.wrapping_add(1);
    }

    pub fn toggle(&mut self) {
        self.state = match self.state {
            MeterState::Expanded => MeterState::Mini,
            MeterState::Mini => MeterState::Hidden,
            MeterState::Hidden => MeterState::Expanded,
        };
        self.version = self.version.wrapping_add(1);
    }

    pub fn cycle_sort(&mut self) {
        self.sort = match self.sort {
            MeterSort::Damage => MeterSort::Dps,
            MeterSort::Dps => MeterSort::Kills,
            MeterSort::Kills => MeterSort::Damage,
        };
        self.version = self.version.wrapping_add(1);
    }

    pub fn sorted_entries(&self, elapsed: f32) -> Vec<&SpellStats> {
        let mut list: Vec<&SpellStats> = self.spells.values().collect();
        match self.sort {
            MeterSort::Damage => list.sort_by(|a, b| b.damage.total_cmp(&a.damage)),
            MeterSort::Dps => list.sort_by(|a, b| {
                let dps_a = a.damage / f64::from(elapsed.max(1.0));
                let dps_b = b.damage / f64::from(elapsed.max(1.0));
                dps_b.total_cmp(&dps_a)
            }),
            MeterSort::Kills => list.sort_by(|a, b| {
                b.kills
                    .cmp(&a.kills)
                    .then_with(|| b.damage.total_cmp(&a.damage))
            }),
        }
        list
    }
}

/// One run's state.
#[derive(Resource)]
pub(super) struct Run {
    pub class: &'static ClassDef,
    pub race: u8,
    pub elapsed: f32,
    pub kills: u32,
    pub level: u32,
    pub xp: f32,
    pub gold: u64,
    pub upgrades: save::AccountUpgrades,
    pub hp: f32,
    pub abilities: Vec<Owned>,
    pub passives: Vec<(&'static PassiveDef, u8)>,
    pub shield: f32,
    pub shield_t: f32,
    pub immune_t: f32,
    pub thorns: f32,
    pub thorns_t: f32,
    pub thorns_spell: u32,
    pub heal_rate: f32,
    pub heal_t: f32,
    pub dash_cd: f32,
    pub dash_max_cd: f32,
    pub iframe_t: f32,
    pub skillshot_cd: f32,
    pub skillshot_max_cd: f32,
    pub rerolls: u32,
    pub skips: u32,
    pub banishes: u32,
    pub banished: Vec<String>,
    /// The class mobility's cooldown and running effect.
    mobile: mobility::MobilityState,
    pub pending_levels: u32,
    pub bosses: u32,
    pub rings: u32,
    pub won: bool,
    pub overtime: bool,
    pub overtime_rings: u32,
    /// Seconds since the hero fell, while the death plays out.
    pub dead_t: Option<f32>,
    pub next_guid: u64,
    pub damage_done: f64,
    pub meter: Meter,
    /// Bumped whenever abilities or passives change, so the HUD rebuilds the icon rows once.
    pub version: u32,
    /// A centre-screen announcement and its seconds left.
    pub banner: Option<(String, f32)>,
    pub shrine_buff: Option<(data::ShrineKind, f32)>,
    pub evolved: Vec<&'static str>,
    pub vacuum_gems: f32,
    pub ultimate_charge: f32,
    pub ultimate_active: Option<(ultimates::UltimateKind, f32)>,
    pub affixes: data::ChallengeAffixes,
    pub combo: combo::ComboState,
    pub trinkets: Vec<trinkets::TrinketInstance>,
    pub spec: Option<specs::SpecKind>,
    pub minions: Vec<minions::MinionKind>,
    pub pending_minions: Vec<minions::MinionKind>,
    pub is_hardcore: bool,
    pub killer_name: Option<String>,
    pub vault: vault::ExtractionVault,
    pub glyphs: Vec<glyphs::GlyphKind>,
    pub equipped: Vec<gear::GearItem>,
    pub enchanter: enchanter::RunEnchanter,
    pub hearth_channel: Option<f32>,
    pub determination_stacks: u8,
    pub sayge_fortune: Option<hub::SaygeFortune>,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            class: &data::CLASSES[0],
            race: 0,
            elapsed: 0.0,
            kills: 0,
            level: 1,
            xp: 0.0,
            gold: 0,
            upgrades: save::AccountUpgrades::default(),
            hp: BASE_HP,
            abilities: Vec::new(),
            passives: Vec::new(),
            shield: 0.0,
            shield_t: 0.0,
            immune_t: 0.0,
            thorns: 0.0,
            thorns_t: 0.0,
            thorns_spell: 0,
            heal_rate: 0.0,
            heal_t: 0.0,
            dash_cd: 0.0,
            dash_max_cd: 3.0,
            iframe_t: 0.0,
            skillshot_cd: 0.0,
            skillshot_max_cd: 2.0,
            rerolls: 2,
            skips: 1,
            banishes: 2,
            banished: Vec::new(),
            mobile: mobility::MobilityState::default(),
            pending_levels: 0,
            bosses: 0,
            rings: 0,
            won: false,
            overtime: false,
            overtime_rings: 0,
            dead_t: None,
            next_guid: 1,
            damage_done: 0.0,
            meter: Meter::default(),
            version: 0,
            banner: None,
            shrine_buff: None,
            evolved: Vec::new(),
            vacuum_gems: 0.0,
            ultimate_charge: 0.0,
            ultimate_active: None,
            affixes: data::ChallengeAffixes::default(),
            combo: combo::ComboState::default(),
            trinkets: Vec::new(),
            spec: None,
            minions: Vec::new(),
            pending_minions: Vec::new(),
            is_hardcore: false,
            killer_name: None,
            vault: vault::ExtractionVault::default(),
            glyphs: Vec::new(),
            equipped: Vec::new(),
            enchanter: enchanter::RunEnchanter::default(),
            hearth_channel: None,
            determination_stacks: 0,
            sayge_fortune: None,
        }
    }
}

impl Run {
    /// Equips an item into the equipped list from either the vault (from_vault = true) or backpack (from_vault = false).
    pub fn equip_item(&mut self, item_idx: usize, from_vault: bool) -> bool {
        self.vault.equip_item(&mut self.equipped, item_idx, from_vault)
    }

    /// Unequips an item from the equipped list and returns it to the backpack.
    pub fn unequip_item(&mut self, slot: gear::EquipSlot) -> bool {
        self.vault.unequip_item(&mut self.equipped, slot)
    }

    pub fn stats(&self) -> Stats {
        self.stats_with_account(None)
    }

    pub fn stats_with_account(&self, account: Option<&save::Account>) -> Stats {
        let upgrades = account.map(|a| &a.upgrades).unwrap_or(&self.upgrades);
        let mut s = Stats {
            might: 1.0 + upgrades.might_bonus(),
            max_hp: BASE_HP * upgrades.max_hp_mult(),
            speed: BASE_SPEED,
            regen: 0.0,
            haste: upgrades.haste_bonus(),
            area: 1.0,
            magnet: BASE_MAGNET + upgrades.magnet_bonus_yards(),
            growth: 1.0 + upgrades.greed_bonus(),
            armor: 0.0,
            crit: 0.05,
            xp_bonus: 1.0,
            gold_bonus: 1.0,
            evasion: 0.0,
            stun_res: 0.0,
        };
        // Tanky classes start tougher, cloth starts quicker.
        match self.class.id {
            1 | 2 => s.max_hp += 40.0,
            7 | 11 | 3 | 4 => s.max_hp += 15.0,
            _ => s.speed += 0.5,
        }
        for &(p, level) in &self.passives {
            let v = p.per_level * f32::from(level);
            match p.stat {
                Stat::Might => s.might += v,
                Stat::MaxHp => s.max_hp *= 1.0 + v,
                Stat::Speed => s.speed *= 1.0 + v,
                Stat::Regen => s.regen += v,
                Stat::Haste => s.haste += v,
                Stat::Area => s.area += v,
                Stat::Magnet => s.magnet *= 1.0 + v,
                Stat::Growth => s.growth += v,
                Stat::Armor => s.armor += v,
                Stat::Crit => s.crit += v,
            }
        }
        if let Some((data::ShrineKind::Swiftness, _)) = self.shrine_buff {
            s.speed *= 1.5;
        }
        if let Some((data::ShrineKind::Greed, _)) = self.shrine_buff {
            s.magnet *= 4.0;
            s.growth *= 2.0;
        }
        if let Some((data::ShrineKind::CursedGreed, _)) = self.shrine_buff {
            s.gold_bonus *= 3.0;
        }
        if self.vacuum_gems > 0.0 {
            s.magnet = 9999.0;
        }
        if let Some((ultimates::UltimateKind::WarriorAvatar, _)) = self.ultimate_active {
            // Orc Warrior synergy: Blood Fury Avatar (+200% damage instead of +100%)
            let avatar_might = if self.race == 2 && self.class.id == 1 { 2.0 } else { 1.0 };
            s.might += avatar_might;
        }
        if let Some((ultimates::UltimateKind::MageTimeWarp, _)) = self.ultimate_active {
            s.haste = (s.haste + 0.70).min(0.95);
            // Human Mage synergy: Arcane Vacuum (+100% magnet radius)
            if self.race == 1 && self.class.id == 8 {
                s.magnet *= 2.0;
            }
        }
        if let Some((ultimates::UltimateKind::RogueKillingSpree, _)) = self.ultimate_active {
            // Night Elf Rogue synergy: 100% evasion during Killing Spree
            if self.race == 4 && self.class.id == 4 {
                s.evasion = 1.0;
            }
        }
        if self.affixes.glass_cannon {
            s.might *= 1.6;
            s.max_hp *= 0.5;
        }
        let affix_bonus = self.affixes.bonus_percent() as f32 / 100.0;
        s.xp_bonus += affix_bonus;
        s.gold_bonus += affix_bonus;
        if self.affixes.midas_curse {
            s.gold_bonus *= 2.0;
        }

        // Combo momentum scaling
        let combo_spd = (self.combo.count as f32 * 0.0015).min(0.40);
        s.speed *= 1.0 + combo_spd;
        let combo_xp = (self.combo.count as f32 * 0.003).min(0.75);
        s.xp_bonus += combo_xp;

        // Class Specialization bonuses
        if let Some(spec) = self.spec {
            specs::apply_spec_stats(spec, &mut s);
        }

        // Trinket passive bonuses
        for t in &self.trinkets {
            if t.kind == trinkets::TrinketKind::TalismanOfEphemeralPower && t.active_timer > 0.0 {
                s.might += 0.80;
                s.haste = (s.haste + 0.80).min(0.95);
            }
            if t.kind == trinkets::TrinketKind::BlackDiamond {
                s.might += t.stacks as f32 * 0.01;
            }
        }

        if let Some(acc) = account {
            if self.equipped.is_empty() {
                for item in acc.equipped.values() {
                    item.apply_to_stats(&mut s, self.class.id);
                }
                gear::apply_set_bonuses(acc.equipped.values(), &mut s);
            }
            if let Some(title) = acc.active_title_def() {
                match title.stat {
                    gear::StatType::Stamina => s.max_hp *= 1.0 + (title.bonus_value / 100.0),
                    gear::StatType::Strength | gear::StatType::Intellect => {
                        s.might *= 1.0 + (title.bonus_value / 100.0);
                    }
                    gear::StatType::Agility => s.speed *= 1.0 + (title.bonus_value / 100.0),
                    gear::StatType::Crit => s.crit += title.bonus_value / 100.0,
                    gear::StatType::Haste => s.haste += title.bonus_value / 100.0,
                    _ => {}
                }
            }
        }
        for item in &self.equipped {
            item.apply_to_stats(&mut s, self.class.id);
        }
        gear::apply_set_bonuses(&self.equipped, &mut s);

        if let Some(fortune) = self.sayge_fortune {
            match fortune {
                hub::SaygeFortune::War => s.might *= 1.12,
                hub::SaygeFortune::Turtle => s.armor += 0.20,
                hub::SaygeFortune::Wind => s.speed *= 1.15,
            }
        }

        if self.determination_stacks > 0 {
            let det = self.determination_stacks as f32;
            s.might *= 1.0 + (0.05 * det);
            s.max_hp *= 1.0 + (0.05 * det);
        }

        self.enchanter.apply_to_stats(&mut s);

        traits::race_trait(self.race).apply_stats(&mut s, self);
        s
    }

    pub fn has_glyph(&self, kind: glyphs::GlyphKind) -> bool {
        self.glyphs.contains(&kind)
    }

    pub fn is_ultimate_active(&self, kind: ultimates::UltimateKind) -> bool {
        matches!(self.ultimate_active, Some((k, _)) if k == kind)
    }

    pub fn guid(&mut self) -> u64 {
        self.next_guid += 1;
        self.next_guid
    }

    pub fn is_evolved(&self, name: &str) -> bool {
        self.evolved.iter().any(|&e| e == name)
    }
}

/// xorshift64*: the mode's dice, seeded from the clock.
#[derive(Resource)]
pub(super) struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(seed | 1)
    }
}

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len.max(1) as u64) as usize
    }
}

fn pause(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn unpause(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

/// Bank gold and record survival stats on run completion.
fn record_game_over(
    mut account: ResMut<save::Account>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
) {
    account.add_gold(run.gold);
    let zone = selection.map_def().name;
    let diff = selection.difficulty.name();
    account.record_run_result(zone, run.elapsed, run.level, diff);

    // Vault extraction: transfer all vaulted items to persistent account armory
    // If the run was won/extracted, also rescue unsecured backpack items!
    if run.won {
        account.armory.extend(run.vault.backpack.drain(..));
        account.reset_determination(diff);
        titles::check_title_unlocks(&mut account, true, run.is_hardcore, run.combo.max_combo, run.elapsed);
    } else {
        account.add_determination(diff);
        titles::check_title_unlocks(&mut account, false, run.is_hardcore, run.combo.max_combo, run.elapsed);
    }
    run.vault.extract_to_account(&mut account);

    // Synchronize equipped gear changes back to account armory
    for item in run.equipped.drain(..) {
        account.equipped.insert(item.slot, item);
    }

    // Hardcore death handling: record fallen hero in graveyard
    if run.is_hardcore && run.hp <= 0.0 {
        let civil = benilla_ui::civil::from_unix(benilla_ui::civil::unix_seconds());
        let date_utc = format!("{:04}-{:02}-{:02}", civil.year, civil.month, civil.day);
        let fallen = save::FallenHero {
            name: run.class.name.to_string(),
            race: run.race,
            class: run.class.id,
            level: run.level,
            zone: zone.to_string(),
            survived_secs: run.elapsed,
            killed_by: run.killer_name.clone().unwrap_or_else(|| "The Horde".to_string()),
            date_utc,
            highest_spree: run.combo.max_combo,
        };
        account.graveyard.push(fallen);
    }

    // Persist extracted armory gear, high score records, and graveyard additions
    let _ = account.save();
}

/// Back at the menu: everything the last run spawned goes, and the run starts over.
fn reset_run(
    mut commands: Commands,
    spawned: Query<Entity, With<RunEntity>>,
    mut run: ResMut<Run>,
    mut pending: ResMut<abilities::PendingHits>,
    mut spawner: ResMut<enemies::Spawner>,
    mut selection: ResMut<Selection>,
    mut event_state: ResMut<events::WorldEventState>,
) {
    for e in &spawned {
        commands.entity(e).despawn();
    }
    // Guids stay unique across runs: caches keyed by guid may outlive a despawn by a frame.
    let next_guid = run.next_guid;
    *run = Run {
        next_guid,
        is_hardcore: selection.is_hardcore,
        vault: vault::ExtractionVault::new(2),
        ..Run::default()
    };
    pending.0.clear();
    *spawner = enemies::Spawner::default();
    *event_state = events::WorldEventState::default();
    // The preview hero is respawned for the look still picked.
    selection.version += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_survivors_update_schedule_init() {
        let mut world = World::new();
        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(
            (
                (
                    enemies::spawn_enemies,
                    enemies::move_enemies,
                    enemies::tick_molten_fire,
                    enemies::tick_boss_arena,
                    hazards::tick_hazards,
                    achievements::check_achievements,
                    telegraphs::update_telegraphs,
                    shrines::tick_shrines,
                    chests::tick_chests,
                    breakables::break_barrels,
                    breakables::collect_powerups,
                ),
                (
                    combo::tick_combo,
                    trinkets::tick_trinkets,
                    minions::tick_minions,
                    breakables::vacuum_magnet_gems,
                    events::tick_world_events,
                    traits::tick_racial_traits,
                    abilities::cast_abilities,
                    skillshot::tick_skillshot,
                    evolutions::tick_evolutions,
                    ultimates::tick_ultimates,
                ),
                (
                    abilities::tick_areas,
                    abilities::apply_hits,
                    enemies::tick_marks,
                    enemies::drop_loot,
                    enemies::collect_gems,
                    hero::hero_vitals,
                ),
            )
                .chain(),
        );
        schedule.add_systems(
            (
                hub::tick_hub,
                hub::hub_input,
                hub::hub_ui_buttons,
                hub::refresh_hub_chrome,
                hub::refresh_hub_modals,
            )
                .chain(),
        );
        schedule.initialize(&mut world).unwrap();
    }
}

