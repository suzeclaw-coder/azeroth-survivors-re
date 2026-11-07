//! Interactive Character Sheet, Equipment Paperdoll, and Armory / Extraction Vault UI.
//!
//! Provides the visual modal to inspect equipped gear, view active set bonuses and hero stats,
//! equip/unequip items, manage extraction vault slots, and salvage spare gear for gold.
//! Ported and adapted from Hordes of Azeroth (`VanillaBonkGear.lua` & `VanillaBonkVault.lua`).

#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::HashMap;

use super::gear::{EquipSlot, GearItem, GearSet, StatType};
use super::hud::{BaseColor, HudFont};
use super::save::Account;
use super::{Phase, Run, Selection};
use crate::sound::GlueSound;

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const EDGE: Color = Color::srgb(0.55, 0.45, 0.25);
const BUTTON: Color = Color::srgba(0.18, 0.06, 0.04, 0.95);
const BUTTON_HOT: Color = Color::srgba(0.36, 0.12, 0.06, 0.98);
const DIM: Color = Color::srgb(0.6, 0.6, 0.6);
const GREEN: Color = Color::srgb(0.2, 0.9, 0.3);

/// Root node for the character sheet & inventory modal.
#[derive(Component)]
pub(super) struct SheetRoot;

/// Container for the left-side equipped slots list.
#[derive(Component)]
pub(super) struct SheetEquippedContainer;

/// Container for the left-side stats & set bonuses summary.
#[derive(Component)]
pub(super) struct SheetStatsContainer;

/// Container for the right-side vault / inventory / armory items.
#[derive(Component)]
pub(super) struct SheetInventoryContainer;

/// Dynamic title text for the sheet.
#[derive(Component)]
pub(super) struct SheetTitleText;

/// Action emitted when interacting with the Character Sheet / Vault modal.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SheetAction {
    Close,
    EquipVault(usize),
    EquipBackpack(usize),
    EquipArmory(usize),
    Unequip(EquipSlot),
    Secure(usize),
    Unsecure(usize),
    SalvageBackpack(usize),
    SalvageArmory(usize),
}

/// Helper to construct a standard button.
fn sheet_btn(action: SheetAction, width: f32, height: f32) -> impl Bundle {
    (
        Button,
        action,
        BaseColor(BUTTON),
        BackgroundColor(BUTTON),
        BorderColor::all(EDGE),
        Node {
            width: Val::Px(width),
            height: Val::Px(height),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
            ..default()
        },
    )
}

/// Helper to construct standard text.
fn sheet_txt(f: &Handle<Font>, text_str: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text_str),
        TextFont {
            font: f.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    )
}

/// Formats a short summary of primary stats on an item.
fn format_stats_short(item: &GearItem) -> String {
    let mut parts = Vec::new();
    if let Some(&v) = item.stats.get(&StatType::Stamina) {
        parts.push(format!("+{} Sta", v as i32));
    }
    if let Some(&v) = item.stats.get(&StatType::Strength) {
        parts.push(format!("+{} Str", v as i32));
    }
    if let Some(&v) = item.stats.get(&StatType::Agility) {
        parts.push(format!("+{} Agi", v as i32));
    }
    if let Some(&v) = item.stats.get(&StatType::Intellect) {
        parts.push(format!("+{} Int", v as i32));
    }
    if let Some(&v) = item.stats.get(&StatType::Armor) {
        parts.push(format!("+{} Arm", v as i32));
    }
    if let Some(&v) = item.stats.get(&StatType::Crit) {
        parts.push(format!("+{}% Crit", v as i32));
    }
    if let Some(&v) = item.stats.get(&StatType::Haste) {
        parts.push(format!("+{}% Haste", v as i32));
    }
    if parts.is_empty() {
        "No bonus stats".to_string()
    } else {
        parts.join(", ")
    }
}

/// Spawns the static framework for the Character Sheet modal at app startup.
pub(super) fn setup_character_sheet(mut commands: Commands, asset_server: Res<AssetServer>) {
    let font = crate::char_select::wow_font(&asset_server);
    let f = &font;

    commands
        .spawn((
            SheetRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                margin: UiRect {
                    left: Val::Px(-370.0),
                    top: Val::Px(-270.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                },
                width: Val::Px(740.0),
                height: Val::Px(540.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(EDGE),
            BackgroundColor(Color::srgba(0.06, 0.04, 0.03, 0.96)),
            GlobalZIndex(450),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // Header bar
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                border: UiRect::bottom(Val::Px(1.0)),
                padding: UiRect::bottom(Val::Px(6.0)),
                ..default()
            })
            .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.4)))
            .with_children(|hdr| {
                hdr.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                })
                .with_children(|left| {
                    left.spawn((SheetTitleText, sheet_txt(f, "CHARACTER SHEET & ARMORY VAULT", 18.0, GOLD)));
                    left.spawn(sheet_txt(
                        f,
                        "Inspect & equip gear, manage vault extraction, or salvage for Gold. [C] or [B] to toggle.",
                        11.0,
                        DIM,
                    ));
                });

                // Close [X] button
                hdr.spawn(sheet_btn(SheetAction::Close, 56.0, 24.0))
                    .with_children(|b| {
                        b.spawn(sheet_txt(f, "[X] Close", 11.0, GOLD));
                    });
            });

            // Two-column layout
            root.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(12.0),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|cols| {
                // Left Column: Equipped Gear & Hero Stats
                cols.spawn(Node {
                    width: Val::Px(350.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    border: UiRect::right(Val::Px(1.0)),
                    padding: UiRect::right(Val::Px(10.0)),
                    ..default()
                })
                .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.3)))
                .with_children(|left| {
                    left.spawn(sheet_txt(f, "EQUIPPED GEAR", 13.0, GOLD));
                    left.spawn((
                        SheetEquippedContainer,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            flex_grow: 1.0,
                            ..default()
                        },
                    ));

                    left.spawn(sheet_txt(f, "HERO ATTRIBUTES & SET BONUSES", 12.0, GOLD));
                    left.spawn((
                        SheetStatsContainer,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                    ))
                    .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.3)))
                    .insert(BackgroundColor(Color::srgba(0.03, 0.02, 0.01, 0.8)));
                });

                // Right Column: Extraction Vault / Armory Inventory
                cols.spawn(Node {
                    width: Val::Px(350.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|right| {
                    right.spawn((
                        SheetInventoryContainer,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(5.0),
                            flex_grow: 1.0,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ));
                });
            });
        });
}

/// Drives the Character Sheet & Vault modal: toggles visibility and updates dynamic contents.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn refresh_character_sheet(
    mut commands: Commands,
    run: Res<Run>,
    account: Option<Res<Account>>,
    _selection: Res<Selection>,
    phase: Res<State<Phase>>,
    font: Option<Res<HudFont>>,
    mut root_query: Query<(&mut Visibility, &mut Node), With<SheetRoot>>,
    equipped_query: Query<Entity, With<SheetEquippedContainer>>,
    stats_query: Query<Entity, With<SheetStatsContainer>>,
    inv_query: Query<Entity, With<SheetInventoryContainer>>,
    mut last_version: Local<u32>,
    mut last_open: Local<bool>,
) {
    let Some(font) = font else {
        return;
    };
    let f = &font.0;
    let is_open = run.vault.is_open;
    let p = *phase.get();

    let state_changed = *last_open != is_open;
    let version_changed = *last_version != run.version;

    if !state_changed && !version_changed {
        return;
    }
    *last_open = is_open;
    *last_version = run.version;

    for (mut vis, mut node) in &mut root_query {
        let want = if is_open {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
        let d = if is_open { Display::Flex } else { Display::None };
        if node.display != d {
            node.display = d;
        }
    }

    if !is_open {
        return;
    }

    let is_in_run = p != Phase::Menu && p != Phase::GameOver && p != Phase::Hub;

    // -------------------------------------------------------------------------
    // 1. REFRESH EQUIPPED SLOTS (LEFT COLUMN)
    // -------------------------------------------------------------------------
    if let Ok(eq_ent) = equipped_query.single() {
        commands.entity(eq_ent).despawn_related::<Children>();

        // Gather currently equipped items
        let equipped_map: HashMap<EquipSlot, GearItem> = if is_in_run {
            run.equipped.iter().map(|item| (item.slot, item.clone())).collect()
        } else if let Some(acc) = account.as_deref() {
            acc.equipped.clone()
        } else {
            HashMap::new()
        };

        commands.entity(eq_ent).with_children(|container| {
            for slot in EquipSlot::ALL {
                let equipped_item = equipped_map.get(&slot);

                container
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                        border: UiRect::bottom(Val::Px(1.0)),
                        ..default()
                    })
                    .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.15)))
                    .with_children(|row| {
                        row.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(1.0),
                            width: Val::Px(270.0),
                            ..default()
                        })
                        .with_children(|item_info| {
                            let slot_label = format!("{}: ", slot.name());
                            if let Some(item) = equipped_item {
                                let qual_col = item.quality.color();
                                let set_tag = item
                                    .set
                                    .map(|s| format!(" [{}]", s.name()))
                                    .unwrap_or_default();

                                item_info.spawn(Node {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|title_row| {
                                    title_row.spawn(sheet_txt(f, &slot_label, 11.0, GOLD));
                                    title_row.spawn(sheet_txt(f, &item.name, 11.0, qual_col));
                                });

                                let sub = format!(
                                    "iLvl {} | {}{}",
                                    item.item_level,
                                    format_stats_short(item),
                                    set_tag
                                );
                                item_info.spawn(sheet_txt(f, &sub, 9.5, DIM));
                            } else {
                                item_info.spawn(Node {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|title_row| {
                                    title_row.spawn(sheet_txt(f, &slot_label, 11.0, DIM));
                                    title_row.spawn(sheet_txt(f, "(Empty)", 11.0, Color::srgba(0.5, 0.5, 0.5, 0.6)));
                                });
                            }
                        });

                        if equipped_item.is_some() {
                            row.spawn(sheet_btn(SheetAction::Unequip(slot), 54.0, 18.0))
                                .with_children(|b| {
                                    b.spawn(sheet_txt(f, "Unequip", 9.5, GOLD));
                                });
                        }
                    });
            }
        });
    }

    // -------------------------------------------------------------------------
    // 2. REFRESH STATS & SET BONUSES (LEFT BOTTOM)
    // -------------------------------------------------------------------------
    if let Ok(st_ent) = stats_query.single() {
        commands.entity(st_ent).despawn_related::<Children>();

        let stats = run.stats_with_account(account.as_deref());

        // Calculate active set counts
        let equipped_items: Vec<&GearItem> = if is_in_run {
            run.equipped.iter().collect()
        } else if let Some(acc) = account.as_deref() {
            acc.equipped.values().collect()
        } else {
            Vec::new()
        };

        let mut set_counts: HashMap<GearSet, usize> = HashMap::new();
        for it in &equipped_items {
            if let Some(s) = it.set {
                *set_counts.entry(s).or_insert(0) += 1;
            }
        }

        commands.entity(st_ent).with_children(|container| {
            // Stat line 1
            container.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|r| {
                r.spawn(sheet_txt(f, &format!("Health: {:.0} / {:.0}", run.hp.max(1.0), stats.max_hp), 10.0, Color::WHITE));
                r.spawn(sheet_txt(f, &format!("Might: +{:.0}%", stats.might * 100.0), 10.0, Color::WHITE));
                r.spawn(sheet_txt(f, &format!("Armor: {:.0}%", stats.armor * 100.0), 10.0, Color::WHITE));
            });

            // Stat line 2
            container.spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            })
            .with_children(|r| {
                r.spawn(sheet_txt(f, &format!("Crit: {:.1}%", stats.crit * 100.0), 10.0, Color::WHITE));
                r.spawn(sheet_txt(f, &format!("Haste: {:.1}% CDR", stats.haste * 100.0), 10.0, Color::WHITE));
                r.spawn(sheet_txt(f, &format!("Regen: +{:.1}/s", stats.regen), 10.0, Color::WHITE));
            });

            // Set bonuses
            container.spawn(sheet_txt(f, "Set Synergies:", 10.0, GOLD));
            let mut any_set = false;
            for set in GearSet::ALL {
                let count = *set_counts.get(&set).unwrap_or(&0);
                if count >= 2 {
                    any_set = true;
                    let desc = if count >= 4 {
                        format!("{} (4/4): {} & {}", set.name(), set.bonus_2pc_desc(), set.bonus_4pc_desc())
                    } else {
                        format!("{} (2/4): {}", set.name(), set.bonus_2pc_desc())
                    };
                    container.spawn(sheet_txt(f, &desc, 9.5, GREEN));
                }
            }
            if !any_set {
                container.spawn(sheet_txt(
                    f,
                    "No active set bonuses. Equip 2 or 4 matching set pieces to activate synergies.",
                    9.0,
                    DIM,
                ));
            }
        });
    }

    // -------------------------------------------------------------------------
    // 3. REFRESH INVENTORY / EXTRACTION VAULT (RIGHT COLUMN)
    // -------------------------------------------------------------------------
    if let Ok(inv_ent) = inv_query.single() {
        commands.entity(inv_ent).despawn_related::<Children>();

        commands.entity(inv_ent).with_children(|container| {
            if is_in_run {
                // Section A: Extraction Vault
                let vault_hdr = format!(
                    "EXTRACTION VAULT ({}/{} SECURED)",
                    run.vault.vaulted.len(),
                    run.vault.capacity
                );
                container.spawn(sheet_txt(f, &vault_hdr, 12.0, Color::srgb(0.4, 0.8, 1.0)));
                container.spawn(sheet_txt(
                    f,
                    "Secured gear survives death and extracts to Account on exit.",
                    10.0,
                    DIM,
                ));

                if run.vault.vaulted.is_empty() {
                    container.spawn(sheet_txt(f, "  (Vault is empty. Loot bosses to find gear)", 10.0, DIM));
                } else {
                    for (idx, item) in run.vault.vaulted.iter().enumerate() {
                        container.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        })
                        .insert(BorderColor::all(Color::srgba(0.4, 0.8, 1.0, 0.4)))
                        .insert(BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 0.8)))
                        .with_children(|row| {
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(1.0),
                                width: Val::Px(200.0),
                                ..default()
                            })
                            .with_children(|info| {
                                info.spawn(sheet_txt(f, &item.name, 10.5, item.quality.color()));
                                info.spawn(sheet_txt(
                                    f,
                                    &format!("[{}] iLvl {} | {}", item.slot.name(), item.item_level, format_stats_short(item)),
                                    9.0,
                                    DIM,
                                ));
                            });

                            row.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(3.0),
                                ..default()
                            })
                            .with_children(|actions| {
                                actions.spawn(sheet_btn(SheetAction::EquipVault(idx), 46.0, 18.0))
                                    .with_children(|b| {
                                        b.spawn(sheet_txt(f, "Equip", 9.5, GOLD));
                                    });
                                actions.spawn(sheet_btn(SheetAction::Unsecure(idx), 52.0, 18.0))
                                    .with_children(|b| {
                                        b.spawn(sheet_txt(f, "Unsecure", 9.0, DIM));
                                    });
                            });
                        });
                    }
                }

                // Section B: Backpack
                let bp_hdr = format!("BACKPACK ({} UNSECURED)", run.vault.backpack.len());
                container.spawn(sheet_txt(f, &bp_hdr, 12.0, Color::srgb(1.0, 0.6, 0.2)));
                container.spawn(sheet_txt(
                    f,
                    "Looted this run. LOST ON DEATH unless secured in the Vault!",
                    10.0,
                    Color::srgb(0.9, 0.5, 0.2),
                ));

                if run.vault.backpack.is_empty() {
                    container.spawn(sheet_txt(f, "  (Backpack is empty)", 10.0, DIM));
                } else {
                    for (idx, item) in run.vault.backpack.iter().enumerate() {
                        let can_secure = run.vault.vaulted.len() < run.vault.capacity;
                        let val = item.salvage_value();

                        container.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        })
                        .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.3)))
                        .insert(BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.8)))
                        .with_children(|row| {
                            row.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(1.0),
                                width: Val::Px(175.0),
                                ..default()
                            })
                            .with_children(|info| {
                                info.spawn(sheet_txt(f, &item.name, 10.5, item.quality.color()));
                                info.spawn(sheet_txt(
                                    f,
                                    &format!("[{}] iLvl {} | {}", item.slot.name(), item.item_level, format_stats_short(item)),
                                    9.0,
                                    DIM,
                                ));
                            });

                            row.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(3.0),
                                ..default()
                            })
                            .with_children(|actions| {
                                if can_secure {
                                    actions.spawn(sheet_btn(SheetAction::Secure(idx), 46.0, 18.0))
                                        .with_children(|b| {
                                            b.spawn(sheet_txt(f, "Secure", 9.0, Color::srgb(0.4, 0.8, 1.0)));
                                        });
                                }
                                actions.spawn(sheet_btn(SheetAction::EquipBackpack(idx), 42.0, 18.0))
                                    .with_children(|b| {
                                        b.spawn(sheet_txt(f, "Equip", 9.5, GOLD));
                                    });
                                actions.spawn(sheet_btn(SheetAction::SalvageBackpack(idx), 58.0, 18.0))
                                    .with_children(|b| {
                                        b.spawn(sheet_txt(f, &format!("+{}g", val), 9.0, GOLD));
                                    });
                            });
                        });
                    }
                }
            } else {
                // Menu / Hub Mode: Account Armory Stash
                let count = account.as_deref().map(|a| a.armory.len()).unwrap_or(0);
                let gold = account.as_deref().map(|a| a.gold).unwrap_or(0);
                let armory_hdr = format!("ACCOUNT ARMORY ({} ITEMS | {} GOLD)", count, gold);
                container.spawn(sheet_txt(f, &armory_hdr, 12.5, GOLD));
                container.spawn(sheet_txt(
                    f,
                    "Permanent gear collection extracted across all runs. Equip or salvage.",
                    10.0,
                    DIM,
                ));

                if let Some(acc) = account.as_deref() {
                    if acc.armory.is_empty() {
                        container.spawn(sheet_txt(
                            f,
                            "  (Armory is empty. Slay bosses and extract gear in-run to store it here!)",
                            10.0,
                            DIM,
                        ));
                    } else {
                        // Display up to 10 items
                        for (idx, item) in acc.armory.iter().enumerate().take(10) {
                            let val = item.salvage_value();
                            container.spawn(Node {
                                flex_direction: FlexDirection::Row,
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            })
                            .insert(BorderColor::all(Color::srgba(0.55, 0.45, 0.25, 0.3)))
                            .insert(BackgroundColor(Color::srgba(0.04, 0.03, 0.02, 0.8)))
                            .with_children(|row| {
                                row.spawn(Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: Val::Px(1.0),
                                    width: Val::Px(210.0),
                                    ..default()
                                })
                                .with_children(|info| {
                                    info.spawn(sheet_txt(f, &item.name, 10.5, item.quality.color()));
                                    info.spawn(sheet_txt(
                                        f,
                                        &format!("[{}] iLvl {} | {}", item.slot.name(), item.item_level, format_stats_short(item)),
                                        9.0,
                                        DIM,
                                    ));
                                });

                                row.spawn(Node {
                                    flex_direction: FlexDirection::Row,
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|actions| {
                                    actions.spawn(sheet_btn(SheetAction::EquipArmory(idx), 46.0, 18.0))
                                        .with_children(|b| {
                                            b.spawn(sheet_txt(f, "Equip", 9.5, GOLD));
                                        });
                                    actions.spawn(sheet_btn(SheetAction::SalvageArmory(idx), 60.0, 18.0))
                                        .with_children(|b| {
                                            b.spawn(sheet_txt(f, &format!("+{}g", val), 9.0, GOLD));
                                        });
                                });
                            });
                        }
                    }
                }
            }
        });
    }
}

/// Dispatches user clicks on buttons in the Character Sheet modal.
#[allow(clippy::too_many_arguments)]
pub(super) fn sheet_buttons(
    mut presses: Query<
        (&Interaction, &SheetAction, &BaseColor, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut run: ResMut<Run>,
    mut account: Option<ResMut<Account>>,
    phase: Res<State<Phase>>,
    mut sounds: MessageWriter<GlueSound>,
) {
    let is_in_run = *phase.get() != Phase::Menu && *phase.get() != Phase::GameOver && *phase.get() != Phase::Hub;

    for (interaction, action, base, mut bg) in &mut presses {
        match interaction {
            Interaction::Hovered => bg.0 = BUTTON_HOT,
            Interaction::None => bg.0 = base.0,
            Interaction::Pressed => {
                bg.0 = BUTTON_HOT;
                match *action {
                    SheetAction::Close => {
                        run.vault.is_open = false;
                        sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
                    }
                    SheetAction::EquipVault(idx) => {
                        if run.equip_item(idx, true) {
                            run.version += 1;
                            sounds.write(GlueSound("igCharacterInfoTab"));
                        }
                    }
                    SheetAction::EquipBackpack(idx) => {
                        if run.equip_item(idx, false) {
                            run.version += 1;
                            sounds.write(GlueSound("igCharacterInfoTab"));
                        }
                    }
                    SheetAction::EquipArmory(idx) => {
                        if let Some(acc) = account.as_deref_mut() {
                            if acc.equip_item(idx) {
                                run.version += 1;
                                sounds.write(GlueSound("igCharacterInfoTab"));
                            }
                        }
                    }
                    SheetAction::Unequip(slot) => {
                        if is_in_run {
                            if run.unequip_item(slot) {
                                run.version += 1;
                                sounds.write(GlueSound("igBackPackOpen"));
                            }
                        } else if let Some(acc) = account.as_deref_mut() {
                            if acc.unequip_item(slot) {
                                run.version += 1;
                                sounds.write(GlueSound("igBackPackOpen"));
                            }
                        }
                    }
                    SheetAction::Secure(idx) => {
                        if run.vault.secure_item(idx) {
                            run.version += 1;
                            sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
                        }
                    }
                    SheetAction::Unsecure(idx) => {
                        if run.vault.unsecure_item(idx) {
                            run.version += 1;
                            sounds.write(GlueSound("igBackPackOpen"));
                        }
                    }
                    SheetAction::SalvageBackpack(idx) => {
                        if let Some(val) = run.vault.salvage_backpack_item(idx) {
                            run.gold = run.gold.saturating_add(val);
                            run.banner = Some((format!("Salvaged gear for +{} Gold!", val), 2.5));
                            run.version += 1;
                            sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
                        }
                    }
                    SheetAction::SalvageArmory(idx) => {
                        if let Some(acc) = account.as_deref_mut() {
                            if let Some(val) = acc.salvage_item(idx) {
                                run.banner = Some((format!("Salvaged armory gear for +{} Gold!", val), 2.5));
                                run.version += 1;
                                sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Keyboard shortcuts for toggling and closing the Character Sheet modal:
/// - [C] or [B]: Toggle character sheet & vault
/// - [Esc]: Close character sheet if open
pub(super) fn sheet_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut run: ResMut<Run>,
    mut sounds: MessageWriter<GlueSound>,
) {
    if keys.just_pressed(KeyCode::KeyC) || keys.just_pressed(KeyCode::KeyB) {
        run.vault.is_open = !run.vault.is_open;
        run.version += 1;
        if run.vault.is_open {
            sounds.write(GlueSound("igCharacterInfoTab"));
        } else {
            sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
        }
    } else if keys.just_pressed(KeyCode::Escape) && run.vault.is_open {
        run.vault.is_open = false;
        run.version += 1;
        sounds.write(GlueSound("igMainMenuOptionCheckBoxOff"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::gear::ItemQuality;

    #[test]
    fn test_format_stats_short() {
        let mut stats = HashMap::new();
        stats.insert(StatType::Stamina, 24.0);
        stats.insert(StatType::Strength, 14.0);
        let item = GearItem {
            id: 1,
            name: "Test Plate".into(),
            slot: EquipSlot::Chest,
            quality: ItemQuality::Epic,
            item_level: 30,
            stats,
            display_id: 1,
            icon_spell: 100,
            set: None,
        };
        let formatted = format_stats_short(&item);
        assert!(formatted.contains("+24 Sta"));
        assert!(formatted.contains("+14 Str"));
    }
}
