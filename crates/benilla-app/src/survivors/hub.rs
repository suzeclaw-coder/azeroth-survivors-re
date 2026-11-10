//! 3D Town Hub & The Inn for Warcraft Survivors.
//!
//! Provides the interactive 3D pre-run town hub featuring interactive NPCs:
//! - Innkeeper Allison: Meta-progression upgrades, Rested buff (+15% XP)
//! - Quartermaster Durgan Ironbelly: Armory, gear vendor, mysterious cache & salvage
//! - Sayge <Darkmoon Faire>: Glyphs, fortune buffs (War, Turtle, Wind) & Rewritten Fate
//! - Run Master Thalor: Stage, difficulty, affixes, hardcore mode, and expedition launch
//! - Target Dummy: Stationary combat test dummy for DPS and synergy testing
//! - Ilsa Runeweaver: Mystic slot enchanter
//!
//! Ported from Hordes of Azeroth (`VanillaBonkInnkeeper.lua`, `VanillaBonkQuartermaster.lua`).

#![allow(dead_code)]

use bevy::prelude::*;
use benilla_world::collision::WorldCollision;
use super::data::{self, EnemyDef};
use super::enemies::Enemy;
use super::gear::{self, EquipSlot, ItemQuality};
use super::hero::{self, Hero};
use super::hud::{BaseColor, HudFont};
use super::save::{self, Account, UpgradeKind};
use super::units::{self, flat, wow_to_bevy, yaw_toward, UnitSpec};
use super::{Owned, Phase, Rng, Run, RunEntity, Selection};
use crate::sound::GlueSound;

/// Map ID containing the 3D town hub (Map 1 = Kalimdor for GM Island).
pub const HUB_MAP_ID: u32 = 1;

/// Map name that contains the 3D tavern hub.
pub const HUB_MAP_NAME: &'static str = "GM Island";

/// WoW world coordinates for GM Island Camp and Cottage plaza (Kalimdor, Map 1).
pub const HUB_CENTER_WOW: [f32; 3] = [16228.0, 16300.0, 16.5];

/// Maximum leash radius in yards from the GM Island camp center (covers camp, tavern, stairs, and loft).
pub const HUB_LEASH: f32 = 85.0;

/// Center of the town hub in Bevy engine coordinates.
pub fn hub_center() -> Vec3 {
    wow_to_bevy(HUB_CENTER_WOW)
}

/// Town Hub NPC identifiers.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HubNpcKind {
    Innkeeper,
    QuartermasterDurgan,
    Sayge,
    RunMaster,
    TargetDummy,
    EnchanterIlsa,
    Banker,
    Transmogrifier,
}

impl HubNpcKind {
    pub const ALL: [HubNpcKind; 8] = [
        HubNpcKind::Innkeeper,
        HubNpcKind::QuartermasterDurgan,
        HubNpcKind::Sayge,
        HubNpcKind::RunMaster,
        HubNpcKind::TargetDummy,
        HubNpcKind::EnchanterIlsa,
        HubNpcKind::Banker,
        HubNpcKind::Transmogrifier,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Innkeeper => "Tarn Oakhoof",
            Self::QuartermasterDurgan => "Quartermaster Durgan Ironbelly",
            Self::Sayge => "Sayge",
            Self::RunMaster => "Gizzik Bonkwhistle",
            Self::TargetDummy => "Target Dummy",
            Self::EnchanterIlsa => "Ilsa Runeweaver",
            Self::Banker => "Bixi Vaultbolt",
            Self::Transmogrifier => "Esmeralda Glimmerstitch",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Innkeeper => "<Innkeeper>",
            Self::QuartermasterDurgan => "<Armory & Blacksmith>",
            Self::Sayge => "<Darkmoon Faire>",
            Self::RunMaster => "<Run Master>",
            Self::TargetDummy => "<Damage Test Dummy>",
            Self::EnchanterIlsa => "<Mystic Enchanter>",
            Self::Banker => "<Banker & Vaultkeeper>",
            Self::Transmogrifier => "<Transmogrifier>",
        }
    }

    pub fn display_id(&self) -> u32 {
        match self {
            Self::Innkeeper => 2106,
            Self::QuartermasterDurgan => 312,
            Self::Sayge => 14856,
            Self::RunMaster => 7172,
            Self::TargetDummy => 3019,
            Self::EnchanterIlsa => 176,
            Self::Banker => 3120,
            Self::Transmogrifier => 3344,
        }
    }

    /// Offset from `hub_center()` in yards.
    pub fn offset(&self) -> Vec3 {
        match self {
            // Inside GM Island Tavern (Inn Cottage) at counter/hearth:
            Self::Innkeeper => Vec3::new(-15.0, 4.35, -24.0),
            // Inside GM Island Tavern at expedition table:
            Self::RunMaster => Vec3::new(-17.0, 4.35, -19.0),
            // Outdoor forge/anvils beside tavern porch:
            Self::QuartermasterDurgan => Vec3::new(-8.0, 0.0, -10.0),
            // Darkmoon fortune tent:
            Self::Sayge => Vec3::new(6.0, 0.0, -5.0),
            // Training yard:
            Self::TargetDummy => Vec3::new(14.0, 0.0, -10.0),
            // Arcane lectern:
            Self::EnchanterIlsa => Vec3::new(4.0, 0.0, 8.0),
            // Extraction vault safe:
            Self::Banker => Vec3::new(-4.0, 0.5, -14.0),
            // Wardrobe mirror:
            Self::Transmogrifier => Vec3::new(-6.0, 0.0, 6.0),
        }
    }

    /// Thematic classic WoW gossip dialogue greeting.
    pub fn greeting(&self) -> &'static str {
        match self {
            Self::Innkeeper => "Make yourself at home, friend. Tell old Tarn how you like things, and the Inn will keep it that way. Rest by the hearth, or bolster your heroic talents before venturing out into the wild.",
            Self::QuartermasterDurgan => "Hail! Keep your blade sharp and your armor sturdy. Inspect your armory stash, salvage unwanted spoils for coin, or purchase a mysterious equipment cache.",
            Self::Sayge => "The mists of destiny part for those who dare look. Seek a Darkmoon divination fortune, or rewrite the threads of fate for your next expedition.",
            Self::RunMaster => "The waygates stand ready! Select your destination continent, tune the challenge difficulty, and embark on your next survivor trial.",
            Self::TargetDummy => "Reinforced straw target dummy designed for combat trials. Unleash your abilities and check the AZS Spell Meter ([M]) to gauge your damage output.",
            Self::EnchanterIlsa => "Arcane currents ebb and flow through Azeroth. Allow me to weave potent magical enchantments into your active battle gear.",
            Self::Banker => "Welcome to the Extraction Vault! Safely store gear brought home from your runs with the Hearthstone, or prepare equipment for future expeditions.",
            Self::Transmogrifier => "Every piece you find adds its look to your collection! Choose a slot, inspect your wardrobe, and customize your battle attire.",
        }
    }

    /// Sound effect kit played when opening interaction dialog with this NPC.
    pub fn greeting_sound(&self) -> &'static str {
        match self {
            Self::Innkeeper => "igQuestListOpen",
            Self::QuartermasterDurgan => "igCharacterInfoOpen",
            Self::Sayge => "SPELL_NATURE_MAGIC_IMMUNITY",
            Self::RunMaster => "igQuestListOpen",
            Self::TargetDummy => "igCreatureAggroSelect",
            Self::EnchanterIlsa => "SPELL_HOLY_HOLYPROTECTION",
            Self::Banker => "igBackPackOpen",
            Self::Transmogrifier => "igCharacterInfoTab",
        }
    }
}

/// Fortune buffs awarded by Sayge for the next expedition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaygeFortune {
    War,    // +12% Might (Damage)
    Turtle, // +20% Armor
    Wind,   // +15% Movement Speed
}

impl SaygeFortune {
    pub fn name(&self) -> &'static str {
        match self {
            Self::War => "Fortune of War (+12% Might)",
            Self::Turtle => "Fortune of the Turtle (+20% Armor)",
            Self::Wind => "Fortune of the Wind (+15% Speed)",
        }
    }
}

/// Tag component for the target dummy.
#[derive(Component)]
pub struct TargetDummy;

/// Persistent resource tracking hub interaction state.
#[derive(Resource, Default)]
pub struct HubState {
    /// Currently focused NPC within interaction reach.
    pub focused_npc: Option<HubNpcKind>,
    /// Currently open NPC dialogue window (None if closed).
    pub open_dialog: Option<HubNpcKind>,
    /// Active tab in Quartermaster Durgan's shop (0: Armor, 1: Weapons, 2: Upgrades, 3: Salvage).
    pub qm_tab: u8,
    /// Whether the in-hub talent upgrade panel is currently open.
    pub show_talents: bool,
    /// Has the player rested at the Innkeeper for the +15% XP bonus?
    pub rested_buff: bool,
    /// Active Sayge divination fortune for the next expedition.
    pub sayge_fortune: Option<SaygeFortune>,
    /// Active Sayge tarot card glyph blessing for the next expedition.
    pub sayge_glyph: Option<super::glyphs::GlyphKind>,
    /// Extra rerolls earned for the next expedition.
    pub extra_rerolls: u32,
    /// Temporary banner text and duration timer.
    pub banner: Option<(String, f32)>,
    /// Version counter for UI redraws.
    pub version: u32,
}

static TARGET_DUMMY_DEF: EnemyDef = EnemyDef {
    name: "Target Dummy",
    display: 3019,
    hp: 1_000_000.0,
    speed: 0.0,
    damage: 0.0,
    scale: 1.25,
    xp: 0,
    level: 60,
    big: false,
    abilities: &[],
};

// =============================================================================
// UI MARKER COMPONENTS & ACTIONS
// =============================================================================

#[derive(Component)]
pub struct HubRoot;

#[derive(Component)]
pub struct HubTopBar;

#[derive(Component)]
pub struct HubInteractPrompt;

#[derive(Component)]
pub struct HubInteractPromptText;

#[derive(Component)]
pub struct HubDialogRoot;

#[derive(Component)]
pub struct HubDialogContent;

#[derive(Component)]
pub struct HubTalentsRoot;

#[derive(Component)]
pub struct HubBannerText;

#[derive(Component)]
pub struct HubTopBarText;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HubAction {
    // Header Bar Actions
    OpenArmory,
    OpenTalents,
    OpenExpedition,
    ReturnToCampfire,
    CloseDialog,
    CloseTalents,
    InteractFocused,

    // Innkeeper Actions
    InnkeeperTalents,
    InnkeeperRest,
    InnkeeperToggleRangeRings,
    InnkeeperToggleAutoLoot,
    InnkeeperCampfire,

    // Quartermaster Actions
    QuartermasterArmory,
    QuartermasterSalvageJunk,
    QuartermasterBuyCache,
    QuartermasterTab(u8),
    QuartermasterBuySlot(gear::EquipSlot, u64),
    QuartermasterUpgradeItem(u32),
    QuartermasterRerollItem(u32, gear::StatType),

    // Sayge Actions
    SaygeFortuneWar,
    SaygeFortuneTurtle,
    SaygeFortuneWind,
    SaygeDrawTarotCard,
    SaygeRewrittenFate,

    // Run Master Actions
    RunMasterCycleMap(isize),
    RunMasterSetDifficulty(data::Difficulty),
    RunMasterCycleDifficulty,
    RunMasterAdjustKeystone(i32),
    RunMasterCycleAffixes,
    RunMasterToggleHardcore,
    RunMasterStartExpedition,

    // Enchanter Actions
    EnchantFiery,
    EnchantCrusader,
    EnchantSpellPower,

    // Target Dummy Actions
    ResetDummyMeter,

    // Banker Actions
    BankerOpenVault,

    // Transmogrifier Actions
    TransmogOpenWardrobe,

    // In-Hub Talent Purchase
    TalentUpgrade(UpgradeKind),
}

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const DIM: Color = Color::srgb(0.65, 0.65, 0.65);
const GREEN: Color = Color::srgb(0.2, 0.9, 0.3);
const BLUE: Color = Color::srgb(0.3, 0.7, 1.0);
const PURPLE: Color = Color::srgb(0.8, 0.4, 1.0);
const BUTTON_BG: Color = Color::srgba(0.18, 0.06, 0.04, 0.95);
const BUTTON_HOT: Color = Color::srgba(0.36, 0.12, 0.06, 0.98);

fn hub_txt(f: &Handle<Font>, content: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: f.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

fn hub_btn(action: HubAction, width: f32, height: f32) -> impl Bundle {
    (
        Button,
        action,
        BaseColor(BUTTON_BG),
        BackgroundColor(BUTTON_BG),
        BorderColor::all(Color::srgb(0.55, 0.45, 0.25)),
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            margin: UiRect::all(Val::Px(2.0)),
            ..default()
        },
    )
}

// =============================================================================
// SETUP & ENTER HUB
// =============================================================================

/// Called upon entering `Phase::Hub` to populate the town hub with the player hero and NPCs.
pub(super) fn enter_hub(
    mut commands: Commands,
    selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut player: ResMut<crate::player::Player>,
    mut hub_state: ResMut<HubState>,
    mut current_map: Option<ResMut<benilla_world::world_map::CurrentMap>>,
    rig: Option<ResMut<hero::CamRig>>,
    world: benilla_world::world_point::WorldPoint,
    collide: WorldCollision,
    heroes: Query<Entity, With<Hero>>,
    existing_entities: Query<Entity, With<RunEntity>>,
    mut sounds: MessageWriter<GlueSound>,
) {
    // 1. Despawn any existing run entities (monsters, chests, old hub npcs)
    for e in &existing_entities {
        commands.entity(e).despawn();
    }
    for e in &heroes {
        commands.entity(e).despawn();
    }

    // 2. Set world map to Kalimdor (Map 1) for GM Island Camp
    if let Some(ref mut cm) = current_map {
        cm.0 = HUB_MAP_ID;
    }

    let center = hub_center();
    let mut hero_pos = center;
    let probe_origin = hero_pos + Vec3::Y * 5.0;
    if let Some(hit) = collide.ray_body(probe_origin, Dir3::NEG_Y, 15.0).filter(|h| h.normal.y >= 0.45) {
        hero_pos.y = probe_origin.y - hit.distance;
    } else if let Some(h) = world.terrain_height_under(hero_pos) {
        hero_pos.y = hero_pos.y.max(h);
    }

    // 3. Anchor world streaming & camera to GM Island Camp
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = hero_pos;

    // 4. Set 3D eye-level camera for Town Hub
    if let Some(mut r) = rig {
        r.mode = hero::CamMode::Orbit;
        r.pitch = hero::HUB_PITCH;
        r.dist = hero::HUB_DIST;
        r.yaw = 0.0;
    }

    // 4. Resolve character selection
    let class_idx = selection.class.unwrap_or(0);
    let class_def = &data::CLASSES[class_idx];
    let look = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .copied()
        .unwrap_or_else(|| {
            selection.looks.first().copied().unwrap_or((0, 0, 0))
        });

    let serial = run.guid();
    hero::spawn_hero(
        &mut commands,
        class_def,
        look.0,
        look.1,
        look.2,
        serial,
        hero_pos,
    );

    // 5. Setup starting ability so player can practice against the Target Dummy
    if run.abilities.is_empty() {
        run.abilities.push(Owned {
            def: data::ability(class_def.start),
            level: 1,
            cd: 0.5,
        });
    }
    run.dead_t = None;
    run.won = false;
    run.hp = run.stats().max_hp;

    // 6. Spawn all 6 Town Hub NPCs around the square
    spawn_hub_npcs_internal(&mut commands, center, &world, &collide);

    // 7. Reset hub interaction flags
    hub_state.focused_npc = None;
    hub_state.open_dialog = None;
    hub_state.show_talents = false;
    hub_state.version += 1;

    // 8. Play welcoming town soundscape cue
    sounds.write(GlueSound("SPELL_HOLY_HOLYPROTECTION"));
}

/// Spawns the town hub NPCs around the tavern square with grounded terrain heights and inward facing.
pub(super) fn spawn_hub_npcs(mut commands: Commands) {
    // Legacy fallback stub required by module signature
    let center = hub_center();
    for kind in HubNpcKind::ALL {
        let pos = center + kind.offset();
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial: 0x9000_0000 | (kind.display_id() as u64),
                display: kind.display_id(),
                scale: if kind == HubNpcKind::TargetDummy { 1.25 } else { 1.0 },
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

fn spawn_hub_npcs_internal(
    commands: &mut Commands,
    center: Vec3,
    world: &benilla_world::world_point::WorldPoint,
    collide: &WorldCollision,
) {
    for kind in HubNpcKind::ALL {
        let mut pos = center + kind.offset();
        // Probe downwards from chest/eye height (1.8 yd) down to floor/terrain (8.0 yd).
        // Indoor NPCs start inside the tavern room beneath the ceiling, snapping cleanly to the floorboards.
        let probe_origin = Vec3::new(pos.x, pos.y + 1.8, pos.z);
        if let Some(hit) = collide.ray_body(probe_origin, Dir3::NEG_Y, 8.0).filter(|h| h.normal.y >= 0.45) {
            pos.y = probe_origin.y - hit.distance;
        } else if let Some(h) = world.terrain_height_under(pos) {
            pos.y = pos.y.max(h);
        }

        let to_center = flat(center - pos);
        let yaw = if to_center.length() > 0.1 {
            yaw_toward(to_center)
        } else {
            0.0
        };

        let is_dummy = kind == HubNpcKind::TargetDummy;
        let e = units::spawn_unit(
            commands,
            &UnitSpec {
                serial: 0x9000_0000 | (kind.display_id() as u64),
                display: kind.display_id(),
                scale: if is_dummy { 1.25 } else { 1.0 },
                level: 60,
                faction: if is_dummy { units::FACTION_MONSTER } else { units::FACTION_FRIENDLY },
                bytes0: 0,
                weapons: &[],
                sheath: 0,
            },
            pos,
            yaw,
        );

        let mut ent_cmd = commands.entity(e);
        ent_cmd.insert((kind, RunEntity));

        // Target Dummy receives combat hits, DoTs, and FCT
        if is_dummy {
            ent_cmd.insert((
                TargetDummy,
                Enemy {
                    def: &TARGET_DUMMY_DEF,
                    hp: 1_000_000.0,
                    max_hp: 1_000_000.0,
                    speed: 0.0,
                    damage: 0.0,
                    radius: 1.2,
                    xp: 0,
                    boss: false,
                    champion: None,
                    champion_shield: 0.0,
                    molten_timer: 0.0,
                    attack_cd: 999999.0,
                    slow: 0.0,
                    slow_t: 0.0,
                    root_t: 0.0,
                    stun_t: 0.0,
                    fear_t: 0.0,
                    weaken: 0.0,
                    weaken_t: 0.0,
                    dots: Vec::new(),
                    dead: false,
                    ability_cd: 999999.0,
                    cast_t: 0.0,
                    current_ability: None,
                    ability_idx: 0,
                    enraged: false,
                },
            ));

            // Secondary Training Dummy (Cleave / AoE Testing)
            let mut cleave_pos = pos + Vec3::new(2.4, 0.0, 0.0);
            let cleave_probe = Vec3::new(cleave_pos.x, cleave_pos.y + 1.8, cleave_pos.z);
            if let Some(hit) = collide.ray_body(cleave_probe, Dir3::NEG_Y, 8.0).filter(|h| h.normal.y >= 0.45) {
                cleave_pos.y = cleave_probe.y - hit.distance;
            } else if let Some(h) = world.terrain_height_under(cleave_pos) {
                cleave_pos.y = cleave_pos.y.max(h);
            }

            let e2 = units::spawn_unit(
                commands,
                &UnitSpec {
                    serial: 0x9000_0000 | (kind.display_id() as u64 + 1),
                    display: kind.display_id(),
                    scale: 1.25,
                    level: 60,
                    faction: units::FACTION_MONSTER,
                    bytes0: 0,
                    weapons: &[],
                    sheath: 0,
                },
                cleave_pos,
                yaw,
            );
            commands.entity(e2).insert((
                kind,
                RunEntity,
                TargetDummy,
                Enemy {
                    def: &TARGET_DUMMY_DEF,
                    hp: 1_000_000.0,
                    max_hp: 1_000_000.0,
                    speed: 0.0,
                    damage: 0.0,
                    radius: 1.2,
                    xp: 0,
                    boss: false,
                    champion: None,
                    champion_shield: 0.0,
                    molten_timer: 0.0,
                    attack_cd: 999999.0,
                    slow: 0.0,
                    slow_t: 0.0,
                    root_t: 0.0,
                    stun_t: 0.0,
                    fear_t: 0.0,
                    weaken: 0.0,
                    weaken_t: 0.0,
                    dots: Vec::new(),
                    dead: false,
                    ability_cd: 999999.0,
                    cast_t: 0.0,
                    current_ability: None,
                    ability_idx: 0,
                    enraged: false,
                },
            ));
        }
    }
}

// =============================================================================
// PER-FRAME HUB TICK & INPUT
// =============================================================================

/// Ticks hub distances, highlights nearest NPC, resets dummy health, and rotates NPCs toward hero.
pub(super) fn tick_hub(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    mut hub_state: ResMut<HubState>,
    hero_query: Query<&Transform, With<Hero>>,
    mut npcs: Query<(&HubNpcKind, &mut Transform), Without<Hero>>,
    mut dummies: Query<&mut Enemy, With<TargetDummy>>,
) {
    if *phase.get() != Phase::Hub {
        return;
    }

    let dt = time.delta_secs();

    // Decrement banner
    if let Some((_, timer)) = hub_state.banner.as_mut() {
        *timer -= dt;
        if *timer <= 0.0 {
            hub_state.banner = None;
            hub_state.version += 1;
        }
    }

    // Keep target dummy alive and at full HP for endless damage testing
    for mut en in &mut dummies {
        en.dead = false;
        if en.hp < en.max_hp {
            en.hp = en.max_hp;
        }
    }

    let Ok(hero_tf) = hero_query.single() else {
        return;
    };
    let hero_pos = hero_tf.translation;

    // Detect closest NPC within 4.5 yards interaction range
    let mut closest_npc = None;
    let mut min_dist = 4.5;
    let mut open_npc_dist = None;
    let mut innkeeper_dist = None;

    for (&kind, mut npc_tf) in &mut npcs {
        let dist = flat(npc_tf.translation - hero_pos).length();
        if dist < min_dist {
            min_dist = dist;
            closest_npc = Some(kind);
        }

        if Some(kind) == hub_state.open_dialog {
            open_npc_dist = Some(dist);
        }
        if kind == HubNpcKind::Innkeeper {
            innkeeper_dist = Some(dist);
        }

        // Smoothly turn NPC toward the player if nearby (except target dummy)
        if kind != HubNpcKind::TargetDummy && dist < 8.0 {
            let to_hero = flat(hero_pos - npc_tf.translation);
            if to_hero.length() > 0.2 {
                let target_rot = Quat::from_rotation_y(yaw_toward(to_hero));
                npc_tf.rotation = npc_tf.rotation.slerp(target_rot, (dt * 5.0).min(1.0));
            }
        }
    }

    if hub_state.focused_npc != closest_npc {
        hub_state.focused_npc = closest_npc;
        hub_state.version += 1;
    }

    // Leash dialogue window: close if player walks beyond 5.5 yards
    if hub_state.open_dialog.is_some() && open_npc_dist.map_or(true, |d| d > 5.5) {
        hub_state.open_dialog = None;
        hub_state.version += 1;
    }

    // Leash talent panel to Innkeeper Tarn Oakhoof: close if player walks beyond 5.5 yards
    if hub_state.show_talents && innkeeper_dist.map_or(true, |d| d > 5.5) {
        hub_state.show_talents = false;
        hub_state.version += 1;
    }
}

/// Keyboard shortcuts in the hub:
/// - [F] / [E]: Talk to focused NPC
/// - [C] / [B]: Open Armory / Character Sheet
/// - [Esc]: Dismiss dialog, close armory, or return to campfire
/// - [Enter]: Embark on expedition (if Run Master active)
pub(super) fn hub_input(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    mut hub_state: ResMut<HubState>,
    mut next: ResMut<NextState<Phase>>,
    mut run: ResMut<Run>,
    mut selection: ResMut<Selection>,
    account: Option<Res<Account>>,
    mut sounds: MessageWriter<GlueSound>,
) {
    if *phase.get() != Phase::Hub {
        return;
    }

    // [F] or [E] interact with focused NPC
    if (keys.just_pressed(KeyCode::KeyF) || keys.just_pressed(KeyCode::KeyE))
        && hub_state.open_dialog.is_none()
        && !hub_state.show_talents
    {
        if let Some(npc) = hub_state.focused_npc {
            hub_state.open_dialog = Some(npc);
            hub_state.version += 1;
            sounds.write(GlueSound(npc.greeting_sound()));
        }
    }

    // [C] or [B] toggle Armory / Character Sheet
    if keys.just_pressed(KeyCode::KeyC) || keys.just_pressed(KeyCode::KeyB) {
        run.vault.is_open = !run.vault.is_open;
        run.version += 1;
        if run.vault.is_open {
            sounds.write(GlueSound("igCharacterInfoOpen"));
        } else {
            sounds.write(GlueSound("igCharacterInfoClose"));
        }
    }

    // [Esc] modal dismissal or return to campfire
    if keys.just_pressed(KeyCode::Escape) {
        if run.vault.is_open {
            run.vault.is_open = false;
            run.version += 1;
            sounds.write(GlueSound("igCharacterInfoClose"));
        } else if hub_state.show_talents {
            hub_state.show_talents = false;
            hub_state.version += 1;
            sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
        } else if hub_state.open_dialog.is_some() {
            hub_state.open_dialog = None;
            hub_state.version += 1;
            sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
        } else {
            next.set(Phase::Menu);
            sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
        }
    }

    // [Enter] embark on expedition if Run Master dialog is open
    if keys.just_pressed(KeyCode::Enter) && hub_state.open_dialog == Some(HubNpcKind::RunMaster) {
        embark_expedition(&mut run, &mut selection, account.as_deref(), &mut hub_state, &mut next, &mut sounds);
    }
}

// =============================================================================
// UI SETUP & DYNAMIC REFRESH
// =============================================================================

/// Sets up the complete Tavern Hub UI hierarchy.
pub(super) fn setup_hub_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;

    commands
        .spawn((
            HubRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // -----------------------------------------------------------------
            // 1. TOP HEADER BAR
            // -----------------------------------------------------------------
            root.spawn((
                HubTopBar,
                Node {
                    width: Val::Percent(96.0),
                    height: Val::Px(48.0),
                    margin: UiRect::top(Val::Px(12.0)),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.90)),
                BorderColor::all(Color::srgb(0.55, 0.45, 0.25)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    HubTopBarText,
                    hub_txt(f, "GM ISLAND CAMP & TAVERN HUB", 14.0, GOLD),
                ));

                bar.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|btns| {
                    btns.spawn(hub_btn(HubAction::OpenArmory, 95.0, 28.0))
                        .with_children(|b| {
                            b.spawn(hub_txt(f, "Armory [C]", 11.0, GOLD));
                        });
                    btns.spawn(hub_btn(HubAction::ReturnToCampfire, 110.0, 28.0))
                        .with_children(|b| {
                            b.spawn(hub_txt(f, "Campfire [Esc]", 11.0, DIM));
                        });
                });
            });

            // -----------------------------------------------------------------
            // 2. CENTER NOTIFICATION BANNER
            // -----------------------------------------------------------------
            root.spawn((
                HubBannerText,
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.04, 0.02, 0.85)),
                BorderColor::all(GOLD),
                Visibility::Hidden,
            ))
            .with_children(|banner| {
                banner.spawn(hub_txt(f, "", 14.0, GOLD));
            });

            // -----------------------------------------------------------------
            // 3. NPC GOSSIP & INTERACTION DIALOG MODAL
            // -----------------------------------------------------------------
            root.spawn((
                HubDialogRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(45.0),
                    margin: UiRect {
                        left: Val::Px(-250.0),
                        top: Val::Px(-180.0),
                        ..default()
                    },
                    width: Val::Px(500.0),
                    padding: UiRect::all(Val::Px(18.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.96)),
                BorderColor::all(Color::srgb(0.55, 0.45, 0.25)),
                Visibility::Hidden,
            ))
            .with_children(|dialog| {
                dialog.spawn((
                    HubDialogContent,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                ));
            });

            // -----------------------------------------------------------------
            // 4. IN-HUB TALENTS MODAL
            // -----------------------------------------------------------------
            root.spawn((
                HubTalentsRoot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Percent(45.0),
                    margin: UiRect {
                        left: Val::Px(-230.0),
                        top: Val::Px(-210.0),
                        ..default()
                    },
                    width: Val::Px(460.0),
                    padding: UiRect::all(Val::Px(16.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.96)),
                BorderColor::all(Color::srgb(0.55, 0.45, 0.25)),
                Visibility::Hidden,
            ));

            // -----------------------------------------------------------------
            // 5. PROXIMITY INTERACT PROMPT (BOTTOM CENTER)
            // -----------------------------------------------------------------
            root.spawn((
                HubInteractPrompt,
                Node {
                    margin: UiRect::bottom(Val::Px(36.0)),
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.08, 0.04, 0.02, 0.90)),
                BorderColor::all(GOLD),
                Visibility::Hidden,
            ))
            .with_children(|prompt| {
                prompt.spawn((
                    HubInteractPromptText,
                    hub_txt(f, "[F / E] Talk to NPC", 14.0, GOLD),
                ));
            });
        });
}

/// Dynamically updates top bar, prompts, and notifications.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn refresh_hub_chrome(
    phase: Res<State<Phase>>,
    hub_state: Res<HubState>,
    selection: Res<Selection>,
    account: Option<Res<Account>>,
    mut ui_set: ParamSet<(
        Query<(&mut Visibility, &mut Node), With<HubRoot>>,
        Query<&mut Text, With<HubTopBarText>>,
        Query<(&mut Visibility, &mut Node), With<HubInteractPrompt>>,
        Query<&mut Text, With<HubInteractPromptText>>,
        Query<(&mut Visibility, &mut Node, &Children), With<HubBannerText>>,
        Query<&mut Text>,
    )>,
    mut last_version: Local<u32>,
    mut last_phase: Local<Phase>,
) {
    let p = *phase.get();
    let is_hub = p == Phase::Hub;

    for (mut vis, mut node) in &mut ui_set.p0() {
        let want_vis = if is_hub { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis {
            *vis = want_vis;
        }
        let want_d = if is_hub { Display::Flex } else { Display::None };
        if node.display != want_d {
            node.display = want_d;
        }
    }

    if !is_hub {
        *last_phase = p;
        return;
    }

    let state_changed = *last_phase != p || *last_version != hub_state.version || selection.is_changed();
    if !state_changed {
        return;
    }
    *last_phase = p;
    *last_version = hub_state.version;

    let gold = account.as_deref().map(|a| a.gold).unwrap_or(0);
    let scrap = account.as_deref().map(|a| a.scrap).unwrap_or(0);
    let class_name = selection.class_def().map(|c| c.name).unwrap_or("Adventurer");

    // 1. Update Top Bar Text
    for mut text in &mut ui_set.p1() {
        let mut buffs_str = String::new();
        if hub_state.rested_buff {
            buffs_str.push_str("  [Rested: +15% XP]");
        }
        if let Some(fortune) = hub_state.sayge_fortune {
            match fortune {
                SaygeFortune::War => buffs_str.push_str("  [Fortune: +12% Might]"),
                SaygeFortune::Turtle => buffs_str.push_str("  [Fortune: +20% Armor]"),
                SaygeFortune::Wind => buffs_str.push_str("  [Fortune: +15% Speed]"),
            }
        }
        if hub_state.extra_rerolls > 0 {
            buffs_str.push_str(&format!("  [Fate: +{} Rerolls]", hub_state.extra_rerolls));
        }

        text.0 = format!(
            "GM ISLAND CAMP & TAVERN HUB  |  {}  |  Gold: {}g  |  Scrap: {}{}",
            class_name, gold, scrap, buffs_str
        );
    }

    // 2. Update Proximity Prompt
    let show_prompt = hub_state.focused_npc.is_some() && hub_state.open_dialog.is_none() && !hub_state.show_talents;
    for (mut vis, mut node) in &mut ui_set.p2() {
        let want_vis = if show_prompt { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis { *vis = want_vis; }
        let want_d = if show_prompt { Display::Flex } else { Display::None };
        if node.display != want_d { node.display = want_d; }
    }
    if show_prompt {
        if let Some(npc) = hub_state.focused_npc {
            for mut text in &mut ui_set.p3() {
                text.0 = format!("[F / E] Talk to {} {}", npc.name(), npc.title());
            }
        }
    }

    // 3. Update Banner Text
    let has_banner = hub_state.banner.is_some();
    let banner_msg = hub_state.banner.as_ref().map(|(msg, _)| msg.clone());
    let mut banner_children = Vec::new();
    for (mut vis, mut node, children) in &mut ui_set.p4() {
        let want_vis = if has_banner { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis { *vis = want_vis; }
        let want_d = if has_banner { Display::Flex } else { Display::None };
        if node.display != want_d { node.display = want_d; }

        if banner_msg.is_some() {
            banner_children.extend(children.iter());
        }
    }
    if let Some(msg) = banner_msg {
        let mut text_q = ui_set.p5();
        for child in banner_children {
            if let Ok(mut txt) = text_q.get_mut(child) {
                txt.0 = msg.clone();
            }
        }
    }
}

/// Dynamically updates the NPC dialog and in-hub talents modal windows.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn refresh_hub_modals(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    hub_state: Res<HubState>,
    selection: Res<Selection>,
    account: Option<Res<Account>>,
    font: Option<Res<HudFont>>,
    mut modals_set: ParamSet<(
        Query<(&mut Visibility, &mut Node), With<HubDialogRoot>>,
        Query<(&mut Visibility, &mut Node, Entity), With<HubTalentsRoot>>,
    )>,
    dialog_content_q: Query<Entity, With<HubDialogContent>>,
    mut last_version: Local<u32>,
    mut last_phase: Local<Phase>,
) {
    let p = *phase.get();
    if p != Phase::Hub {
        *last_phase = p;
        for (mut vis, mut node) in &mut modals_set.p0() {
            *vis = Visibility::Hidden;
            node.display = Display::None;
        }
        for (mut vis, mut node, _) in &mut modals_set.p1() {
            *vis = Visibility::Hidden;
            node.display = Display::None;
        }
        return;
    }

    let state_changed = *last_phase != p || *last_version != hub_state.version || selection.is_changed();
    if !state_changed {
        return;
    }
    *last_phase = p;
    *last_version = hub_state.version;

    let Some(font) = font else { return };
    let f = &font.0;

    // 4. Update Dialog Modal
    let show_dialog = hub_state.open_dialog.is_some() && !hub_state.show_talents;
    for (mut vis, mut node) in &mut modals_set.p0() {
        let want_vis = if show_dialog { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis { *vis = want_vis; }
        let want_d = if show_dialog { Display::Flex } else { Display::None };
        if node.display != want_d { node.display = want_d; }
    }

    if let (Some(npc), Ok(content_ent)) = (hub_state.open_dialog, dialog_content_q.single()) {
        commands.entity(content_ent).despawn_related::<Children>();

        commands.entity(content_ent).with_children(|d| {
            // Header: Name & Title
            d.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Baseline,
                ..default()
            })
            .with_children(|h| {
                h.spawn(hub_txt(f, npc.name(), 16.0, GOLD));
                h.spawn(hub_txt(f, npc.title(), 12.0, DIM));
            });

            // Greeting text
            d.spawn(Node {
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            })
            .insert(BackgroundColor(Color::srgba(0.08, 0.06, 0.04, 0.6)))
            .with_children(|g| {
                g.spawn(hub_txt(f, npc.greeting(), 12.5, Color::srgb(0.9, 0.85, 0.75)));
            });

            // Action Buttons
            d.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            })
            .with_children(|opts| {
                match npc {
                    HubNpcKind::Innkeeper => {
                        opts.spawn(hub_btn(HubAction::InnkeeperTalents, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "📜 View Talents & Meta-Upgrades", 11.5, GOLD));
                            });
                        let rest_lbl = if hub_state.rested_buff {
                            "🍺 Rest by the Hearth (Active: +15% XP Bonus)"
                        } else {
                            "🍺 Rest by the Hearth (+15% XP Bonus for Next Run)"
                        };
                        opts.spawn(hub_btn(HubAction::InnkeeperRest, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, rest_lbl, 11.5, if hub_state.rested_buff { GREEN } else { GOLD }));
                            });
                        let rings_on = account.as_deref().map(|a| a.range_rings_enabled).unwrap_or(true);
                        let rings_lbl = if rings_on {
                            "🎯 Weapon Range Rings: [x] ENABLED (Ground Decals Active)"
                        } else {
                            "🎯 Weapon Range Rings: [ ] Disabled"
                        };
                        opts.spawn(hub_btn(HubAction::InnkeeperToggleRangeRings, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, rings_lbl, 11.5, if rings_on { GOLD } else { DIM }));
                            });
                        let autoloot_on = account.as_deref().map(|a| a.auto_loot_chests).unwrap_or(true);
                        let autoloot_lbl = if autoloot_on {
                            "📦 Auto-Loot Boss Chests: [x] ENABLED"
                        } else {
                            "📦 Auto-Loot Boss Chests: [ ] Disabled"
                        };
                        opts.spawn(hub_btn(HubAction::InnkeeperToggleAutoLoot, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, autoloot_lbl, 11.5, if autoloot_on { GOLD } else { DIM }));
                            });
                        opts.spawn(hub_btn(HubAction::InnkeeperCampfire, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "🏕️ Return to Campfire / Select Different Hero", 11.5, DIM));
                            });
                    }
                    HubNpcKind::QuartermasterDurgan => {
                        let gold = account.as_deref().map(|a| a.gold).unwrap_or(0);
                        let scrap = account.as_deref().map(|a| a.scrap).unwrap_or(0);

                        // 1. Navigation Tabs Header
                        opts.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            width: Val::Px(460.0),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        }).with_children(|tabs| {
                            let t0_color = if hub_state.qm_tab == 0 { GOLD } else { DIM };
                            let t1_color = if hub_state.qm_tab == 1 { GOLD } else { DIM };
                            let t2_color = if hub_state.qm_tab == 2 { GOLD } else { DIM };
                            let t3_color = if hub_state.qm_tab == 3 { GOLD } else { DIM };

                            tabs.spawn(hub_btn(HubAction::QuartermasterTab(0), 108.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, "🛡️ Armor", 11.0, t0_color));
                            });
                            tabs.spawn(hub_btn(HubAction::QuartermasterTab(1), 108.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, "⚔️ Weapons", 11.0, t1_color));
                            });
                            tabs.spawn(hub_btn(HubAction::QuartermasterTab(2), 118.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, "🔨 Upgrades", 11.0, t2_color));
                            });
                            tabs.spawn(hub_btn(HubAction::QuartermasterTab(3), 108.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, "♻️ Salvage", 11.0, t3_color));
                            });
                        });

                        // 2. Tab Content Panels
                        match hub_state.qm_tab {
                            0 => {
                                // Tab 0: Armor Shop
                                opts.spawn(hub_txt(f, &format!("Available Gold: {}g | Curated Class Armor Pieces", gold), 10.5, GOLD));
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::Head, 100), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "👑 Gladiator's Crown (Head) - 100 Gold", 10.5, BLUE));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::Shoulders, 100), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "🛡️ Vanguard Spaulders (Shoulders) - 100 Gold", 10.5, BLUE));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::Chest, 120), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "🥋 Stormwind Breastplate (Chest) - 120 Gold", 10.5, BLUE));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::Legs, 100), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "👖 Ironforge Greaves (Legs) - 100 Gold", 10.5, BLUE));
                                });
                            }
                            1 => {
                                // Tab 1: Weapons & Mystical Trinkets
                                opts.spawn(hub_txt(f, &format!("Available Gold: {}g | Armaments & Relics", gold), 10.5, GOLD));
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::MainHand, 150), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "🗡️ Blade of the Warchief (Main Hand) - 150 Gold", 10.5, BLUE));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::OffHand, 120), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "🛡️ Bulwark of the Ancients (Off-Hand) - 120 Gold", 10.5, BLUE));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterBuySlot(gear::EquipSlot::Trinket1, 140), 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "🧿 Talisman of Ephemeral Power (Trinket) - 140 Gold", 10.5, BLUE));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterBuyCache, 460.0, 24.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "🎁 Mysterious Equipment Cache (Random Quality) - 150 Gold", 10.5, PURPLE));
                                });
                            }
                            2 => {
                                // Tab 2: Forge Upgrades & Stat Reforging
                                opts.spawn(hub_txt(f, &format!("Available Scrap: {} | Upgrade (+10% Stats) or Reforge Stat Lines", scrap), 10.5, GOLD));
                                if let Some(acc) = account.as_deref() {
                                    let mut items: Vec<&gear::GearItem> = Vec::new();
                                    for item in acc.equipped.values() {
                                        items.push(item);
                                    }
                                    for item in acc.armory.iter().take(3) {
                                        items.push(item);
                                    }

                                    if items.is_empty() {
                                        opts.spawn(hub_txt(f, "  (No gear equipped or in Armory to forge. Buy from shop or extract gear!)", 10.0, DIM));
                                    } else {
                                        for item in items.iter().take(3) {
                                            let upg_cost_opt = item.upgrade_cost();
                                            let upg_str = match upg_cost_opt {
                                                Some(c) => format!("🔨 Upgrade [{}] (+10% stats, +2 iLvl) [{} Scrap] (Rank {}/3)", item.name, c, item.upgrades),
                                                None => format!("⭐ [{}] - MAX UPGRADE RANK (3/3)", item.name),
                                            };
                                            let can_upg = upg_cost_opt.map(|c| scrap >= c).unwrap_or(false);
                                            opts.spawn(hub_btn(HubAction::QuartermasterUpgradeItem(item.id), 460.0, 22.0)).with_children(|b| {
                                                b.spawn(hub_txt(f, &upg_str, 9.5, if can_upg { GREEN } else if upg_cost_opt.is_some() { DIM } else { GOLD }));
                                            });

                                            // Reforge option for first secondary stat line
                                            if let Some((&first_stat, _)) = item.stats.iter().next() {
                                                let cost = item.reroll_cost();
                                                let reroll_str = format!("🎲 Reforge [{}] {} Line ({} Gold)", item.name, first_stat.name(), cost);
                                                let can_reroll = gold >= cost;
                                                opts.spawn(hub_btn(HubAction::QuartermasterRerollItem(item.id, first_stat), 460.0, 20.0)).with_children(|b| {
                                                    b.spawn(hub_txt(f, &reroll_str, 9.0, if can_reroll { Color::srgb(0.4, 0.8, 1.0) } else { DIM }));
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Tab 3: Scrap Recycling Station
                                opts.spawn(hub_txt(f, &format!("Current Scrap: {} | Current Gold: {}g", scrap, gold), 11.0, GOLD));
                                opts.spawn(hub_btn(HubAction::QuartermasterSalvageJunk, 460.0, 28.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "♻️ Salvage All Common/Poor Armory Items (Yields Gold & Scrap)", 11.0, GOLD));
                                });
                                opts.spawn(hub_btn(HubAction::QuartermasterArmory, 460.0, 28.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "⚔️ Open Armory to Inspect & Salvage Individual Pieces [C]", 11.0, BLUE));
                                });
                            }
                        }
                    }
                    HubNpcKind::Sayge => {
                        opts.spawn(hub_btn(HubAction::SaygeFortuneWar, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "⚔️ Divination: Fortune of War (+12% Might)", 11.0, GOLD));
                            });
                        opts.spawn(hub_btn(HubAction::SaygeFortuneTurtle, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "🛡️ Divination: Fortune of the Turtle (+20% Armor)", 11.0, BLUE));
                            });
                        opts.spawn(hub_btn(HubAction::SaygeFortuneWind, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "💨 Divination: Fortune of the Wind (+15% Speed)", 11.0, GREEN));
                            });
                        let tarot_lbl = if let Some(g) = hub_state.sayge_glyph {
                            format!("🎴 Darkmoon Tarot: [{}] Active!", g.name())
                        } else {
                            "🎴 Draw Darkmoon Tarot Card (Ability Glyph Blessing)".into()
                        };
                        opts.spawn(hub_btn(HubAction::SaygeDrawTarotCard, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, &tarot_lbl, 11.0, if hub_state.sayge_glyph.is_some() { PURPLE } else { GOLD }));
                            });
                        let reroll_lbl = format!("🃏 Rewritten Fate: +1 Extra Reroll (100g) [Current: +{}]", hub_state.extra_rerolls);
                        opts.spawn(hub_btn(HubAction::SaygeRewrittenFate, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, &reroll_lbl, 11.0, PURPLE));
                            });
                    }
                    HubNpcKind::RunMaster => {
                        let map_def = selection.map_def();
                        let keystone = selection.keystone();

                        // 1. Visual Difficulty Tier Plaques Header
                        opts.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            width: Val::Px(460.0),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        }).with_children(|tiers| {
                            let is_norm = selection.difficulty == data::Difficulty::Normal;
                            let is_hero = selection.difficulty == data::Difficulty::Heroic;
                            let is_myth = selection.difficulty == data::Difficulty::Mythic;
                            let is_night = selection.difficulty == data::Difficulty::Nightmare;

                            tiers.spawn(hub_btn(HubAction::RunMasterSetDifficulty(data::Difficulty::Normal), 108.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, if is_norm { "★ Normal" } else { "Normal" }, 10.5, if is_norm { GOLD } else { DIM }));
                            });
                            tiers.spawn(hub_btn(HubAction::RunMasterSetDifficulty(data::Difficulty::Heroic), 108.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, if is_hero { "★ Heroic" } else { "Heroic" }, 10.5, if is_hero { BLUE } else { DIM }));
                            });
                            tiers.spawn(hub_btn(HubAction::RunMasterSetDifficulty(data::Difficulty::Mythic), 108.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, if is_myth { "★ Mythic" } else { "Mythic" }, 10.5, if is_myth { PURPLE } else { DIM }));
                            });
                            tiers.spawn(hub_btn(HubAction::RunMasterSetDifficulty(data::Difficulty::Nightmare), 118.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, if is_night { "★ Nightmare" } else { "Nightmare" }, 10.5, if is_night { Color::srgb(1.0, 0.4, 0.2) } else { DIM }));
                            });
                        });

                        // 2. Mythic Keystone Stepper
                        if matches!(selection.difficulty, data::Difficulty::Mythic | data::Difficulty::Nightmare) {
                            let hp_pct = 120 + (keystone - 1) * 15;
                            let dmg_pct = 60 + (keystone - 1) * 10;
                            let reward_pct = 60 + (keystone - 1) * 20;

                            opts.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                width: Val::Px(460.0),
                                margin: UiRect::bottom(Val::Px(2.0)),
                                ..default()
                            }).with_children(|keystone_row| {
                                keystone_row.spawn(hub_btn(HubAction::RunMasterAdjustKeystone(-1), 60.0, 22.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "[-1 Level]", 9.5, GOLD));
                                });
                                keystone_row.spawn(hub_txt(
                                    f,
                                    &format!("🔑 Keystone +{}  (+{}% HP | +{}% Dmg | +{}% Loot)", keystone, hp_pct, dmg_pct, reward_pct),
                                    10.0,
                                    PURPLE,
                                ));
                                keystone_row.spawn(hub_btn(HubAction::RunMasterAdjustKeystone(1), 60.0, 22.0)).with_children(|b| {
                                    b.spawn(hub_txt(f, "[+1 Level]", 9.5, GOLD));
                                });
                            });
                        }

                        // 3. Zone Map Navigator Card
                        opts.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            width: Val::Px(460.0),
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        }).with_children(|zone_nav| {
                            zone_nav.spawn(hub_btn(HubAction::RunMasterCycleMap(-1), 75.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, "◀ Previous", 10.0, GOLD));
                            });
                            zone_nav.spawn(hub_txt(f, &format!("🗺️ {} ({})", map_def.name, map_def.zone), 12.0, GOLD));
                            zone_nav.spawn(hub_btn(HubAction::RunMasterCycleMap(1), 75.0, 24.0)).with_children(|b| {
                                b.spawn(hub_txt(f, "Next ▶", 10.0, GOLD));
                            });
                        });

                        // 4. Zone High Score Record
                        let record_str = if let Some(rec) = account.as_deref().and_then(|a| a.high_scores.get(map_def.name)) {
                            format!("🏆 Personal Record: Survived {:.1}s | Lv {} | Tier: {}", rec.best_survival_secs, rec.highest_level, rec.highest_difficulty)
                        } else {
                            "🏆 Personal Record: No expedition records recorded yet".into()
                        };
                        opts.spawn(hub_txt(f, &record_str, 9.5, DIM));

                        // 5. Challenge Affixes & Hardcore Mode
                        let affix_str = format!("⚡ Challenge Affixes: {} (Click to Cycle)", selection.affixes.label());
                        opts.spawn(hub_btn(HubAction::RunMasterCycleAffixes, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, &affix_str, 10.5, BLUE));
                            });

                        let hc_str = if selection.is_hardcore {
                            "☠️ Hardcore Mode: [x] ENABLED (Mortal Permadeath - Graveyard Persisted)"
                        } else {
                            "🛡️ Hardcore Mode: [ ] Disabled (Softcore Expedition)"
                        };
                        opts.spawn(hub_btn(HubAction::RunMasterToggleHardcore, 460.0, 24.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, hc_str, 10.5, if selection.is_hardcore { Color::srgb(1.0, 0.3, 0.3) } else { DIM }));
                            });

                        // 6. Embark Button
                        opts.spawn(hub_btn(HubAction::RunMasterStartExpedition, 460.0, 32.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "🚀 >>> EMBARK ON EXPEDITION <<<", 13.5, GREEN));
                            });
                    }
                    HubNpcKind::EnchanterIlsa => {
                        opts.spawn(hub_btn(HubAction::EnchantFiery, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "🔥 Enchant Weapon: Fiery Weapon (Fire Burst on Hit)", 11.5, Color::srgb(1.0, 0.4, 0.1)));
                            });
                        opts.spawn(hub_btn(HubAction::EnchantCrusader, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "✝️ Enchant Weapon: Crusader (Holy Healing & Strength)", 11.5, GOLD));
                            });
                        opts.spawn(hub_btn(HubAction::EnchantSpellPower, 460.0, 26.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "✨ Enchant Chest: Spell Power (+Might & Crit)", 11.5, BLUE));
                            });
                    }
                    HubNpcKind::TargetDummy => {
                        opts.spawn(hub_btn(HubAction::ResetDummyMeter, 460.0, 28.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "🔄 Reset AZS Spell Meter & DPS Tally", 12.0, GOLD));
                            });
                    }
                    HubNpcKind::Banker => {
                        opts.spawn(hub_btn(HubAction::BankerOpenVault, 460.0, 30.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "🏦 Open Extraction Vault & Account Stash [C]", 12.0, GOLD));
                            });
                    }
                    HubNpcKind::Transmogrifier => {
                        opts.spawn(hub_btn(HubAction::TransmogOpenWardrobe, 460.0, 30.0))
                            .with_children(|b| {
                                b.spawn(hub_txt(f, "👗 Open Wardrobe & Dressing Room", 12.0, PURPLE));
                            });
                    }
                }

                // Close Button
                opts.spawn(hub_btn(HubAction::CloseDialog, 460.0, 24.0))
                    .with_children(|b| {
                        b.spawn(hub_txt(f, "Close [Esc]", 11.0, DIM));
                    });
            });
        });
    }

    // 5. Update In-Hub Talents Panel
    for (mut vis, mut node, talents_ent) in &mut modals_set.p1() {
        let want_vis = if hub_state.show_talents { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want_vis { *vis = want_vis; }
        let want_d = if hub_state.show_talents { Display::Flex } else { Display::None };
        if node.display != want_d { node.display = want_d; }

        if hub_state.show_talents {
            commands.entity(talents_ent).despawn_related::<Children>();

            commands.entity(talents_ent).with_children(|t| {
                t.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|h| {
                    h.spawn(hub_txt(f, "INNKEEPER TALENT TREE & UPGRADES", 14.0, GOLD));
                    h.spawn(hub_btn(HubAction::CloseTalents, 60.0, 22.0))
                        .with_children(|b| {
                            b.spawn(hub_txt(f, "Close", 10.0, DIM));
                        });
                });

                if let Some(acc) = account.as_deref() {
                    for &kind in UpgradeKind::ALL.iter() {
                        let rank = acc.upgrade_rank(kind);
                        let cost_str = match acc.upgrade_cost(kind) {
                            Some(c) => format!("+{}g", c),
                            None => "MAX".to_string(),
                        };
                        let can_afford = acc.can_purchase_upgrade(kind);

                        t.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                            border: UiRect::bottom(Val::Px(1.0)),
                            ..default()
                        })
                        .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.2)))
                        .with_children(|row| {
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                width: Val::Px(300.0),
                                ..default()
                            })
                            .with_children(|info| {
                                info.spawn(hub_txt(
                                    f,
                                    &format!("{} [{}/{}]", kind.name(), rank, save::MAX_UPGRADE_RANK),
                                    11.0,
                                    GOLD,
                                ));
                                info.spawn(hub_txt(f, kind.description(), 9.5, DIM));
                            });

                            if rank < save::MAX_UPGRADE_RANK {
                                row.spawn(hub_btn(HubAction::TalentUpgrade(kind), 70.0, 24.0))
                                    .with_children(|b| {
                                        b.spawn(hub_txt(f, &cost_str, 10.0, if can_afford { GREEN } else { DIM }));
                                    });
                            } else {
                                row.spawn(hub_txt(f, "MAX", 11.0, GREEN));
                            }
                        });
                    }
                }
            });
        }
    }
}

// =============================================================================
// BUTTON INTERACTION SYSTEM
// =============================================================================

/// Handles button clicks across the Hub UI.
#[allow(clippy::too_many_arguments)]
pub(super) fn hub_ui_buttons(
    mut clicks: Query<
        (&Interaction, &HubAction, &BaseColor, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut hub_state: ResMut<HubState>,
    mut run: ResMut<Run>,
    mut selection: ResMut<Selection>,
    mut account: Option<ResMut<Account>>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
    mut sounds: MessageWriter<GlueSound>,
    mut range_settings: Option<ResMut<super::hero::RangeRingSettings>>,
) {
    for (interaction, action, base, mut bg) in &mut clicks {
        match interaction {
            Interaction::Hovered => bg.0 = BUTTON_HOT,
            Interaction::None => bg.0 = base.0,
            Interaction::Pressed => {
                bg.0 = BUTTON_HOT;
                match *action {
                    HubAction::OpenArmory | HubAction::QuartermasterArmory => {
                        run.vault.is_open = true;
                        run.version += 1;
                        sounds.write(GlueSound("igCharacterInfoTab"));
                    }
                    HubAction::OpenTalents | HubAction::InnkeeperTalents => {
                        hub_state.show_talents = true;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igSpellBookOpen"));
                    }
                    HubAction::OpenExpedition => {
                        hub_state.open_dialog = Some(HubNpcKind::RunMaster);
                        hub_state.version += 1;
                        sounds.write(GlueSound("igQuestListOpen"));
                    }
                    HubAction::ReturnToCampfire | HubAction::InnkeeperCampfire => {
                        hub_state.open_dialog = None;
                        hub_state.show_talents = false;
                        next.set(Phase::Menu);
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
                    }
                    HubAction::CloseDialog => {
                        hub_state.open_dialog = None;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
                    }
                    HubAction::CloseTalents => {
                        hub_state.show_talents = false;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
                    }
                    HubAction::InteractFocused => {
                        if let Some(npc) = hub_state.focused_npc {
                            hub_state.open_dialog = Some(npc);
                            hub_state.version += 1;
                            sounds.write(GlueSound(npc.greeting_sound()));
                        }
                    }
                    HubAction::InnkeeperRest => {
                        hub_state.rested_buff = true;
                        hub_state.banner = Some(("Well Rested! +15% Bonus XP on your next run.".into(), 3.5));
                        hub_state.version += 1;
                        sounds.write(GlueSound("LEVELUPSOUND"));
                    }
                    HubAction::InnkeeperToggleRangeRings => {
                        if let Some(acc) = account.as_deref_mut() {
                            acc.range_rings_enabled = !acc.range_rings_enabled;
                            let new_val = acc.range_rings_enabled;
                            if let Some(ref mut rs) = range_settings {
                                rs.enabled = new_val;
                            }
                            let _ = acc.save();
                            hub_state.banner = Some((
                                format!("Weapon Range Rings: {}", if new_val { "ENABLED" } else { "DISABLED" }),
                                2.5,
                            ));
                            hub_state.version += 1;
                            sounds.write(GlueSound(if new_val { "igMainMenuOptionCheckBoxOn" } else { "igMainMenuOptionCheckBoxOff" }));
                        }
                    }
                    HubAction::InnkeeperToggleAutoLoot => {
                        if let Some(acc) = account.as_deref_mut() {
                            acc.auto_loot_chests = !acc.auto_loot_chests;
                            let new_val = acc.auto_loot_chests;
                            let _ = acc.save();
                            hub_state.banner = Some((
                                format!("Auto-Loot Boss Chests: {}", if new_val { "ENABLED" } else { "DISABLED" }),
                                2.5,
                            ));
                            hub_state.version += 1;
                            sounds.write(GlueSound(if new_val { "igMainMenuOptionCheckBoxOn" } else { "igMainMenuOptionCheckBoxOff" }));
                        }
                    }
                    HubAction::QuartermasterTab(tab) => {
                        hub_state.qm_tab = tab;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igCharacterInfoTab"));
                    }
                    HubAction::QuartermasterBuySlot(slot, cost) => {
                        if let Some(acc) = account.as_deref_mut() {
                            if acc.gold >= cost {
                                acc.gold -= cost;
                                let mut item = gear::generate_gear(&mut rng, 25, 0.45);
                                item.slot = slot;
                                let item_name = item.name.clone();
                                acc.armory.push(item);
                                let _ = acc.save();
                                hub_state.banner = Some((format!("Acquired [{}]! Stashed in your Armory.", item_name), 3.5));
                                sounds.write(GlueSound("igBackPackOpen"));
                            } else {
                                hub_state.banner = Some((format!("Not enough gold! Cost is {}g.", cost), 2.5));
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::QuartermasterUpgradeItem(item_id) => {
                        if let Some(acc) = account.as_deref_mut() {
                            match acc.upgrade_gear_item(item_id) {
                                Ok(new_rank) => {
                                    hub_state.banner = Some((format!("Forged Upgrade! Item upgraded to Rank [{}/3] (+10% stats, +2 iLvl)!", new_rank), 3.5));
                                    sounds.write(GlueSound("LEVELUPSOUND"));
                                }
                                Err(err) => {
                                    hub_state.banner = Some((format!("Upgrade Failed: {}", err), 2.5));
                                }
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::QuartermasterRerollItem(item_id, target_stat) => {
                        if let Some(acc) = account.as_deref_mut() {
                            match acc.reroll_gear_stat(item_id, target_stat, &mut rng) {
                                Ok((new_stat, val)) => {
                                    hub_state.banner = Some((format!("Reforged Stat Line to +{} {}!", val as i32, new_stat.name()), 3.5));
                                    sounds.write(GlueSound("SPELL_NATURE_MAGIC_IMMUNITY"));
                                }
                                Err(err) => {
                                    hub_state.banner = Some((format!("Reforge Failed: {}", err), 2.5));
                                }
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::QuartermasterSalvageJunk => {
                        if let Some(acc) = account.as_deref_mut() {
                            let mut salvaged_count = 0;
                            let mut gold_earned = 0;
                            let mut scrap_earned = 0;
                            let mut idx = 0;
                            while idx < acc.armory.len() {
                                if acc.armory[idx].quality <= ItemQuality::Common {
                                    if let Some((g, s)) = acc.salvage_item_for_scrap(idx) {
                                        salvaged_count += 1;
                                        gold_earned += g;
                                        scrap_earned += s;
                                        continue;
                                    }
                                }
                                idx += 1;
                            }
                            if salvaged_count > 0 {
                                hub_state.banner = Some((
                                    format!("Salvaged {} junk items for +{} Gold & +{} Scrap!", salvaged_count, gold_earned, scrap_earned),
                                    3.5,
                                ));
                                sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
                            } else {
                                hub_state.banner = Some(("No common/poor items in your Armory to salvage.".into(), 2.5));
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::QuartermasterBuyCache => {
                        if let Some(acc) = account.as_deref_mut() {
                            if acc.gold >= 150 {
                                acc.gold -= 150;
                                let item = gear::generate_gear(&mut rng, 30, 0.40);
                                let item_name = item.name.clone();
                                acc.armory.push(item);
                                let _ = acc.save();
                                hub_state.banner = Some((format!("Acquired [{}]! Added to your Armory.", item_name), 3.5));
                                sounds.write(GlueSound("igBackPackOpen"));
                            } else {
                                hub_state.banner = Some(("Not enough gold! Mysterious Cache costs 150g.".into(), 2.5));
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::SaygeFortuneWar => {
                        hub_state.sayge_fortune = Some(SaygeFortune::War);
                        hub_state.banner = Some(("Darkmoon Fortune of War granted: +12% Might for next run!".into(), 3.5));
                        hub_state.version += 1;
                        sounds.write(GlueSound("SPELL_NATURE_MAGIC_IMMUNITY"));
                    }
                    HubAction::SaygeFortuneTurtle => {
                        hub_state.sayge_fortune = Some(SaygeFortune::Turtle);
                        hub_state.banner = Some(("Darkmoon Fortune of the Turtle granted: +20% Armor for next run!".into(), 3.5));
                        hub_state.version += 1;
                        sounds.write(GlueSound("SPELL_NATURE_MAGIC_IMMUNITY"));
                    }
                    HubAction::SaygeFortuneWind => {
                        hub_state.sayge_fortune = Some(SaygeFortune::Wind);
                        hub_state.banner = Some(("Darkmoon Fortune of the Wind granted: +15% Move Speed for next run!".into(), 3.5));
                        hub_state.version += 1;
                        sounds.write(GlueSound("SPELL_NATURE_MAGIC_IMMUNITY"));
                    }
                    HubAction::SaygeRewrittenFate => {
                        if let Some(acc) = account.as_deref_mut() {
                            if acc.gold >= 100 && hub_state.extra_rerolls < 4 {
                                acc.gold -= 100;
                                hub_state.extra_rerolls += 1;
                                let _ = acc.save();
                                hub_state.banner = Some((format!("Rewritten Fate! +{} extra reroll for your next expedition.", hub_state.extra_rerolls), 3.0));
                                sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
                            } else if hub_state.extra_rerolls >= 4 {
                                hub_state.banner = Some(("Maximum fate rerolls reached (4 max).".into(), 2.5));
                            } else {
                                hub_state.banner = Some(("Not enough gold! Rewritten Fate costs 100g.".into(), 2.5));
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::SaygeDrawTarotCard => {
                        if let Some(acc) = account.as_deref_mut() {
                            if acc.gold >= 50 {
                                acc.gold -= 50;
                                let _ = acc.save();
                                let pool = [
                                    super::glyphs::GlyphKind::MovingBlizzard,
                                    super::glyphs::GlyphKind::PiercingShadow,
                                    super::glyphs::GlyphKind::ConsecrationHeal,
                                    super::glyphs::GlyphKind::RuptureBurst,
                                ];
                                let picked = pool[rng.index(pool.len())];
                                hub_state.sayge_glyph = Some(picked);
                                hub_state.banner = Some((format!("The Tarot reveals: [{}]! Active on your next run.", picked.name()), 4.0));
                                sounds.write(GlueSound("SPELL_NATURE_MAGIC_IMMUNITY"));
                            } else {
                                hub_state.banner = Some(("Not enough gold! A Tarot reading costs 50g.".into(), 2.5));
                            }
                            hub_state.version += 1;
                        }
                    }
                    HubAction::RunMasterCycleMap(delta) => {
                        let total = data::MAPS.len() as isize;
                        let next_idx = ((selection.map as isize + delta).rem_euclid(total)) as usize;
                        selection.map = next_idx;
                        selection.version += 1;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOn"));
                    }
                    HubAction::RunMasterSetDifficulty(diff) => {
                        selection.difficulty = diff;
                        selection.version += 1;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOn"));
                    }
                    HubAction::RunMasterCycleDifficulty => {
                        selection.difficulty = selection.difficulty.next();
                        selection.version += 1;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOn"));
                    }
                    HubAction::RunMasterAdjustKeystone(delta) => {
                        let current = selection.keystone();
                        let next_level = ((current as i32 + delta).clamp(1, 25)) as u32;
                        selection.keystone_level = next_level;
                        selection.version += 1;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOn"));
                    }
                    HubAction::RunMasterCycleAffixes => {
                        selection.affixes = selection.affixes.next();
                        selection.version += 1;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOn"));
                    }
                    HubAction::RunMasterToggleHardcore => {
                        selection.is_hardcore = !selection.is_hardcore;
                        selection.version += 1;
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOn"));
                    }
                    HubAction::RunMasterStartExpedition => {
                        embark_expedition(&mut run, &mut selection, account.as_deref(), &mut hub_state, &mut next, &mut sounds);
                    }
                    HubAction::EnchantFiery => {
                        run.enchanter.enchant_slot(EquipSlot::MainHand, super::enchanter::EnchantKind::FieryWeapon, super::enchanter::EnchantGrade::Greater);
                        hub_state.banner = Some(("Enchanted Main Hand with Greater Fiery Weapon!".into(), 3.0));
                        hub_state.version += 1;
                        sounds.write(GlueSound("SPELL_FIRE_FLAMEBLADES"));
                    }
                    HubAction::EnchantCrusader => {
                        run.enchanter.enchant_slot(EquipSlot::MainHand, super::enchanter::EnchantKind::Crusader, super::enchanter::EnchantGrade::Greater);
                        hub_state.banner = Some(("Enchanted Main Hand with Greater Crusader!".into(), 3.0));
                        hub_state.version += 1;
                        sounds.write(GlueSound("SPELL_HOLY_HOLYPROTECTION"));
                    }
                    HubAction::EnchantSpellPower => {
                        run.enchanter.enchant_slot(EquipSlot::Chest, super::enchanter::EnchantKind::SpellPower, super::enchanter::EnchantGrade::Greater);
                        hub_state.banner = Some(("Enchanted Chest with Greater Spell Power (+Might & Crit)!".into(), 3.0));
                        hub_state.version += 1;
                        sounds.write(GlueSound("SPELL_HOLY_HOLYPROTECTION"));
                    }
                    HubAction::ResetDummyMeter => {
                        run.meter = Default::default();
                        hub_state.banner = Some(("AZS Spell Meter reset!".into(), 2.0));
                        hub_state.version += 1;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
                    }
                    HubAction::BankerOpenVault => {
                        run.vault.is_open = true;
                        run.version += 1;
                        sounds.write(GlueSound("igBackPackOpen"));
                    }
                    HubAction::TransmogOpenWardrobe => {
                        run.vault.is_open = true;
                        run.version += 1;
                        sounds.write(GlueSound("igCharacterInfoOpen"));
                    }
                    HubAction::TalentUpgrade(kind) => {
                        if let Some(acc) = account.as_deref_mut() {
                            if acc.purchase_upgrade(kind) {
                                hub_state.version += 1;
                                sounds.write(GlueSound("LEVELUPSOUND"));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Starts an expedition from the Run Master, applying all active hub buffs.
fn embark_expedition(
    run: &mut Run,
    selection: &mut Selection,
    account: Option<&save::Account>,
    hub_state: &mut HubState,
    next: &mut NextState<Phase>,
    sounds: &mut MessageWriter<GlueSound>,
) {
    if super::hud::start_run(run, selection, account) {
        // Apply active Hub Fortune and Buffs
        if hub_state.rested_buff {
            run.banner = Some(("Well Rested: +15% XP Active!".into(), 3.0));
        }
        if let Some(fortune) = hub_state.sayge_fortune {
            run.sayge_fortune = Some(fortune);
        }
        if let Some(glyph) = hub_state.sayge_glyph {
            if !run.has_glyph(glyph) {
                run.glyphs.push(glyph);
            }
            glyph.apply_to_run(run, 1);
        }
        if hub_state.extra_rerolls > 0 {
            run.rerolls += hub_state.extra_rerolls;
        }

        // Consume one-time buffs
        hub_state.open_dialog = None;
        hub_state.show_talents = false;
        hub_state.rested_buff = false;
        hub_state.sayge_fortune = None;
        hub_state.sayge_glyph = None;
        hub_state.extra_rerolls = 0;

        next.set(Phase::Playing);
        sounds.write(GlueSound("SPELL_PORTAL_CAST"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hub_npc_properties() {
        assert_eq!(HubNpcKind::Innkeeper.name(), "Tarn Oakhoof");
        assert_eq!(HubNpcKind::Innkeeper.display_id(), 2106);
        assert_eq!(HubNpcKind::QuartermasterDurgan.display_id(), 312);
        assert_eq!(HubNpcKind::Sayge.name(), "Sayge");
        assert_eq!(HubNpcKind::Sayge.display_id(), 14856);
        assert_eq!(HubNpcKind::RunMaster.display_id(), 7172);
        assert_eq!(HubNpcKind::TargetDummy.name(), "Target Dummy");
        assert_eq!(HubNpcKind::TargetDummy.display_id(), 3019);
        assert_eq!(HubNpcKind::EnchanterIlsa.name(), "Ilsa Runeweaver");
        assert_eq!(HubNpcKind::EnchanterIlsa.display_id(), 176);
        assert_eq!(HubNpcKind::Banker.name(), "Bixi Vaultbolt");
        assert_eq!(HubNpcKind::Banker.display_id(), 3120);
        assert_eq!(HubNpcKind::Transmogrifier.name(), "Esmeralda Glimmerstitch");
        assert_eq!(HubNpcKind::Transmogrifier.display_id(), 3344);

        for npc in HubNpcKind::ALL {
            assert!(!npc.greeting_sound().is_empty());
        }
    }

    #[test]
    fn test_sayge_fortunes() {
        assert!(SaygeFortune::War.name().contains("Might"));
        assert!(SaygeFortune::Turtle.name().contains("Armor"));
        assert!(SaygeFortune::Wind.name().contains("Speed"));
    }

    #[test]
    fn test_hub_systems_param_validation() {
        use bevy::ecs::system::IntoSystem;
        let mut world = World::new();
        let mut s1 = IntoSystem::into_system(refresh_hub_chrome);
        s1.initialize(&mut world);
        let mut s2 = IntoSystem::into_system(refresh_hub_modals);
        s2.initialize(&mut world);
        let mut s3 = IntoSystem::into_system(tick_hub);
        s3.initialize(&mut world);
        let mut s4 = IntoSystem::into_system(hub_input);
        s4.initialize(&mut world);
        let mut s5 = IntoSystem::into_system(hub_ui_buttons);
        s5.initialize(&mut world);
    }

    #[test]
    fn test_hub_schedule_validation() {
        use bevy::ecs::schedule::Schedule;
        let mut world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems((
            tick_hub,
            hub_input,
            hub_ui_buttons,
            refresh_hub_chrome,
            refresh_hub_modals,
        ).chain());
        schedule.initialize(&mut world).unwrap();
    }
}
