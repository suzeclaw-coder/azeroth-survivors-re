//! The run's pick screen, laid out like the 1.12 character create and select glue screens and built
//! from their own widgets and art ([`crate::glue`]): the race, gender and class tower on the left,
//! the zone list in the character-select frame on the right, Enter World at the bottom.
//!
//! The survivors HUD lays out on the 768-unit canvas through `UiScale`, so every glue builder here
//! takes a unit scale.

use bevy::prelude::*;
use bevy::ui_render::ui_material::MaterialNode;

use super::hud::{looks_for, rgb, start_run, HudFont, MenuRoot};
use super::save::{self, UpgradeKind};
use super::{data, Phase, Rng, Run, Selection};
use crate::char_select::race_name;
use super::traits;
use crate::glue::art::{class_tc, race_tc, tc_rect, GlueArt, BTN_BG, BUTTON_TC, DIM, GOLD, NAME_EDGE};
use crate::glue::backdrop::{backdrop_border, tiled_bg_node};
use crate::glue::widgets::{
    abs, glue_button, icon_button, outlined_text, overlay, FallbackFace, GlueBtn, GlueBtnKind,
    GlueDisabled, GlueText, Hilight, HoverLabel, LockHighlight,
};
use crate::glue::GlueClicks;

/// The create screen's two race columns.
const ALLIANCE: [u8; 4] = [1, 3, 4, 7];
const HORDE: [u8; 4] = [2, 5, 6, 8];

/// `CharacterSelectCharacterFrame`'s border and fill tints.
const FRAME_BORDER: Color = Color::srgb(0.8, 0.8, 0.8);
const FRAME_FILL: Color = Color::srgba(0.09, 0.09, 0.09, 0.85);

/// The tower ends under the class grid and Randomize, where the create screen's appearance dials
/// would start: there is nothing to customise here.
const TOWER_H: f32 = 590.0;

/// The zone rows: the character-select row, compacted so every zone fits the frame.
const ROW_TOP: f32 = 44.0;
const ROW_PITCH: f32 = 32.0;
const ROW_H: f32 = 30.0;
const ROW_W: f32 = 228.0;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuAction {
    Race(u8),
    Sex(u8),
    Class(usize),
    Map(usize),
    Randomize,
    Enter,
    CycleDifficulty,
    CycleAffixes,
    ToggleHardcore,
    Upgrade(UpgradeKind),
    RefundUpgrades,
    OpenArmoryModal,
    EnterHub,
}

/// On [`MenuRoot`] once the glue art is in and the screen is built.
#[derive(Component)]
pub(super) struct MenuBuilt;

#[derive(Component)]
pub(super) struct TowerGoldText;

#[derive(Component)]
pub(super) struct ArmoryGoldText;

#[derive(Component)]
pub(super) struct HardcoreText;

#[derive(Component)]
pub(super) struct UpgradeRankText(pub(super) UpgradeKind);

#[derive(Component)]
pub(super) struct UpgradeBtnText(pub(super) UpgradeKind);

#[derive(Component)]
pub(super) struct UpgradeBonusText(pub(super) UpgradeKind);

/// A race button's face, re-cropped to the picked gender.
#[derive(Component)]
pub(super) struct RaceFace(u8);

#[derive(Component)]
pub(super) struct PickName;

#[derive(Component)]
pub(super) struct RacialTraitText;

#[derive(Component)]
pub(super) struct DifficultyText;

#[derive(Component)]
pub(super) struct AffixText;

pub(super) fn difficulty_color(diff: data::Difficulty) -> Color {
    match diff {
        data::Difficulty::Normal => Color::WHITE,
        data::Difficulty::Heroic => Color::srgb(0.0, 0.44, 0.87),
        data::Difficulty::Mythic => Color::srgb(0.64, 0.21, 0.93),
        data::Difficulty::Nightmare => Color::srgb(0.95, 0.15, 0.15),
    }
}

pub(super) fn affix_color(affixes: &data::ChallengeAffixes) -> Color {
    match affixes.active_count() {
        0 => Color::srgb(0.75, 0.75, 0.75),
        1 => Color::srgb(0.2, 0.9, 0.5),
        2 => Color::srgb(0.95, 0.75, 0.2),
        3 => Color::srgb(1.0, 0.5, 0.1),
        _ => Color::srgb(1.0, 0.25, 0.35),
    }
}

/// The saved pick, `benilla-config/survivors-pick.txt` (`localStorage` on the page):
/// `class race sex map diff_idx affix_idx`, restored at the menu so a reload keeps the last hero and zone.
fn pick_path() -> Option<std::path::PathBuf> {
    crate::local_state::home().map(|h| h.join("survivors-pick.txt"))
}

fn save_pick(sel: &Selection) {
    let (Some(class), Some(&(_, race, sex))) = (sel.class, sel.look.and_then(|i| sel.looks.get(i)))
    else {
        return;
    };
    let diff_idx = match sel.difficulty {
        data::Difficulty::Normal => 0,
        data::Difficulty::Heroic => 1,
        data::Difficulty::Mythic => 2,
        data::Difficulty::Nightmare => 3,
    };
    let affix_idx = sel.affixes.preset_idx();
    let hc_val = if sel.is_hardcore { 1 } else { 0 };
    if let Some(path) = pick_path() {
        let _ = crate::local_state::write_atomic(
            &path,
            &format!("{class} {race} {sex} {} {diff_idx} {affix_idx} {hc_val}", sel.map),
        );
    }
}

/// Restore the saved pick once the creature data can resolve its looks; a stale or missing file
/// leaves the default.
pub(super) fn restore_pick(
    creatures: Option<Res<crate::entities::Creatures>>,
    mut selection: ResMut<Selection>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(creatures) = creatures.as_deref() else {
        return;
    };
    *done = true;
    let Some(text) = pick_path().and_then(|p| crate::local_state::read_to_string(&p).ok()) else {
        return;
    };
    let v: Vec<usize> = text
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    if v.len() < 4 {
        return;
    }
    let (class, race, sex, map) = (v[0], v[1], v[2], v[3]);
    let difficulty = match v.get(4).copied() {
        Some(1) => data::Difficulty::Heroic,
        Some(2) => data::Difficulty::Mythic,
        Some(3) => data::Difficulty::Nightmare,
        _ => data::Difficulty::Normal,
    };
    selection.difficulty = difficulty;
    if let Some(&affix_idx) = v.get(5) {
        selection.affixes = data::ChallengeAffixes::from_preset(affix_idx);
    }
    if let Some(&hc_val) = v.get(6) {
        selection.is_hardcore = hc_val != 0;
    }
    if map < data::MAPS.len() {
        selection.map = map;
        selection.stage = map;
        selection.version += 1;
    }
    if class < data::CLASSES.len() {
        let all = all_looks(creatures);
        if let Some(l) = find(&all[class], Some(race as u8), Some(sex as u8)) {
            set(&mut selection, &all, class, l);
        }
    }
}

/// Every class's looks, `(display, race, sex)`, indexed like [`data::CLASSES`].
fn all_looks(creatures: &crate::entities::Creatures) -> Vec<Vec<(u32, u8, u8)>> {
    data::CLASSES
        .iter()
        .map(|c| looks_for(c, creatures))
        .collect()
}

fn find(looks: &[(u32, u8, u8)], race: Option<u8>, sex: Option<u8>) -> Option<usize> {
    looks
        .iter()
        .position(|&(_, r, s)| race.is_none_or(|x| x == r) && sex.is_none_or(|x| x == s))
}

fn set(sel: &mut Selection, all: &[Vec<(u32, u8, u8)>], class: usize, look: usize) {
    sel.class = Some(class);
    sel.looks = all[class].clone();
    sel.look = Some(look);
    sel.version += 1;
}

/// Build the screen into [`MenuRoot`] once the archives are readable, so the glue art is there.
pub(super) fn build_menu(
    mut commands: Commands,
    roots: Query<Entity, (With<MenuRoot>, Without<MenuBuilt>)>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut art: ResMut<GlueArt>,
    mut images: ResMut<Assets<Image>>,
    mut add_mats: ResMut<Assets<crate::glue::add_material::AddUiMaterial>>,
    font: Res<HudFont>,
    selection: Res<Selection>,
    account: Res<save::Account>,
) {
    let Ok(root) = roots.single() else { return };
    let Some(mut assets) = assets else { return };
    art.ensure_loaded(&mut assets, &mut images, &mut add_mats);
    let f = &font.0;
    commands.entity(root).insert(MenuBuilt).with_children(|ui| {
        tower(ui, &art, f, &account);
        armory_frame(ui, &art, f, &account);
        // `CharacterCreateWoWLogo`, 256×128 at TOPLEFT (3,-7), after the tower so it draws over
        // the border art, as the create screen orders it.
        if let Some(logo) = &art.logo {
            ui.spawn((
                ImageNode::new(logo.clone()),
                abs(1.0, 3.0, 7.0, 256.0, 128.0),
            ));
        }
        zone_frame(ui, &art, f, &selection);

        // `CharSelectCharacterName`, GlueFontNormalHuge at BOTTOM (0,120).
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(120.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|c| {
            outlined_text(
                c,
                Node::default(),
                (),
                PickName,
                GlueText {
                    text: "",
                    size: 22.0,
                    color: GOLD,
                    wrap: false,
                },
                f,
                1.0,
            );
        });

        // `CharSelectRacialTrait`, at BOTTOM (0,96).
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(96.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|c| {
            outlined_text(
                c,
                Node::default(),
                (),
                RacialTraitText,
                GlueText {
                    text: "",
                    size: 14.0,
                    color: Color::srgb(0.9, 0.85, 0.55),
                    wrap: false,
                },
                f,
                1.0,
            );
        });
        // `CharSelectEnterWorldButton`, 200×60 at BOTTOM (0,30).
        ui.spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(30.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(16.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|c| {
            glue_button(
                c,
                &art,
                f,
                MenuAction::EnterHub,
                "Tavern Hub [H]",
                170.0,
                60.0,
                GlueBtnKind::Normal,
                1.0,
            );
            glue_button(
                c,
                &art,
                f,
                MenuAction::Enter,
                "Enter World",
                200.0,
                60.0,
                GlueBtnKind::Normal,
                1.0,
            );
        });
    });
}

/// `CharacterCreateConfigurationFrame`, 206×600 at TOPLEFT (28,-74): the race columns under the
/// faction banners, the gender pair, the class grid, the class text and Randomize.
fn tower(
    ui: &mut ChildSpawnerCommands,
    art: &GlueArt,
    f: &Handle<Font>,
    account: &save::Account,
) {
    let px = Val::Px;
    ui.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(28.0),
        top: px(74.0),
        width: px(206.0),
        height: px(TOWER_H),
        ..default()
    })
    .with_children(|tower| {
        if let Some(bg) = &art.tower_bg {
            tower.spawn((
                ImageNode::new(bg.clone()),
                abs(1.0, -3.0, 0.0, 218.0, TOWER_H + 6.0),
            ));
        }
        // The create tower's top piece, then its foot piece cut to end where the class grid does.
        if let Some((border, size)) = &art.tower_border {
            for (top, height, tc) in [
                (0.0, 236.0, [0.0, 0.875, 0.0, 0.9375]),
                (
                    236.0,
                    TOWER_H - 236.0 + 6.0,
                    [
                        0.0,
                        0.875,
                        1.0 - (TOWER_H - 236.0 + 6.0) / 256.0 * 0.8203125 / (210.0 / 256.0),
                        1.0,
                    ],
                ),
            ] {
                tower.spawn((
                    ImageNode {
                        image: border.clone(),
                        rect: Some(tc_rect(*size, tc)),
                        ..default()
                    },
                    abs(1.0, -9.0, top, 224.0, height),
                ));
            }
        }
        if let Some(banners) = &art.banners {
            tower.spawn((
                ImageNode::new(banners.clone()),
                abs(1.0, -27.0, 60.0, 256.0, 259.0),
            ));
        }

        // Account gold display in the tower header
        outlined_text(
            tower,
            Node {
                justify_content: JustifyContent::Center,
                ..abs(1.0, 10.0, 14.0, 186.0, 18.0)
            },
            (),
            TowerGoldText,
            GlueText {
                text: &format!("Gold: {}g", account.gold),
                size: 15.0,
                color: GOLD,
                wrap: false,
            },
            f,
            1.0,
        );

        for (label, center) in [("Alliance", 51.0), ("Horde", 151.0)] {
            outlined_text(
                tower,
                Node {
                    justify_content: JustifyContent::Center,
                    ..abs(1.0, center - 60.0, 40.0, 120.0, 17.0)
                },
                (),
                (),
                GlueText {
                    text: label,
                    size: 15.0,
                    color: GOLD,
                    wrap: false,
                },
                f,
                1.0,
            );
        }

        for (faction, left) in [(ALLIANCE, 33.0), (HORDE, 127.0)] {
            tower
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(left),
                    top: px(68.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(5.0),
                    ..default()
                })
                .with_children(|col| {
                    for race in faction {
                        let face = art.races.as_ref().and_then(|(sheet, size)| {
                            race_tc(race, 0).map(|tc| (sheet.clone(), tc_rect(*size, tc)))
                        });
                        icon_button(
                            col,
                            f,
                            MenuAction::Race(race),
                            Some(RaceFace(race)),
                            face,
                            None::<()>,
                            race_name(race),
                            art,
                            1.0,
                        );
                    }
                });
        }

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(53.0),
                top: px(303.0),
                flex_direction: FlexDirection::Row,
                column_gap: px(5.0),
                ..default()
            })
            .with_children(|g| {
                for (sex, label) in [(0u8, "Male"), (1u8, "Female")] {
                    let face = art.gender.as_ref().map(|(sheet, size)| {
                        let half = if sex == 0 {
                            [0.0, 0.5, 0.0, 1.0]
                        } else {
                            [0.5, 1.0, 0.0, 1.0]
                        };
                        (sheet.clone(), tc_rect(*size, half))
                    });
                    icon_button(
                        g,
                        f,
                        MenuAction::Sex(sex),
                        None::<()>,
                        face,
                        None::<()>,
                        label,
                        art,
                        1.0,
                    );
                }
            });

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                // The gender pair's centre line: 53 + (2·48 + 5)/2 ≈ 103.
                left: px(103.0 - (3.0 * 48.0 + 2.0 * 4.0) / 2.0),
                width: px(3.0 * 48.0 + 2.0 * 4.0),
                top: px(368.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                justify_content: JustifyContent::Center,
                column_gap: px(4.0),
                row_gap: px(4.0),
                ..default()
            })
            .with_children(|grid| {
                for (i, class) in data::CLASSES.iter().enumerate() {
                    let face = art.classes.as_ref().and_then(|(sheet, size)| {
                        class_tc(class.id).map(|tc| (sheet.clone(), tc_rect(*size, tc)))
                    });
                    icon_button(
                        grid,
                        f,
                        MenuAction::Class(i),
                        None::<()>,
                        face,
                        None::<()>,
                        class.name,
                        art,
                        1.0,
                    );
                }
            });

        tower
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(30.0),
                top: px(TOWER_H - 46.0),
                ..default()
            })
            .with_children(|r| {
                glue_button(
                    r,
                    art,
                    f,
                    MenuAction::Randomize,
                    "Randomize",
                    146.0,
                    30.0,
                    GlueBtnKind::Small,
                    1.0,
                );
            });
    });
}

/// The zone list in `CharacterSelectCharacterFrame` (260×642 at TOPRIGHT (-5,-15)), one
/// character-row per zone, lowest level first.
fn zone_frame(
    ui: &mut ChildSpawnerCommands,
    art: &GlueArt,
    f: &Handle<Font>,
    selection: &Selection,
) {
    let px = Val::Px;
    let mut order: Vec<(usize, u32, u32)> = (0..data::MAPS.len())
        .map(|i| {
            let levels = data::roster(i).enemies.iter().map(|e| e.level);
            let lo = levels.clone().min().unwrap_or(1);
            let hi = levels.max().unwrap_or(lo);
            (i, lo, hi)
        })
        .collect();
    order.sort_by_key(|&(i, lo, _)| (lo, data::MAPS[i].name));

    ui.spawn(Node {
        position_type: PositionType::Absolute,
        right: px(5.0),
        top: px(15.0),
        width: px(260.0),
        height: px(696.0),
        ..default()
    })
    .with_children(|frame| {
        if let (Some(bg), Some(border)) = (&art.tooltip_bg, &art.name_border) {
            frame.spawn((
                tiled_bg_node(bg.clone(), NAME_EDGE, 1.0, FRAME_FILL),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(10.0),
                    right: px(5.0),
                    top: px(4.0),
                    bottom: px(9.0),
                    ..default()
                },
            ));
            backdrop_border(frame, border, NAME_EDGE, FRAME_BORDER);
        } else {
            frame.spawn((BackgroundColor(FRAME_FILL), crate::glue::widgets::overlay()));
        }
        outlined_text(
            frame,
            Node {
                position_type: PositionType::Absolute,
                top: px(12.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            (),
            (),
            GlueText {
                text: "Choose a Zone",
                size: 18.0,
                color: DIM,
                wrap: false,
            },
            f,
            1.0,
        );
        for (row, &(i, lo, hi)) in order.iter().enumerate() {
            let map = &data::MAPS[i];
            frame
                .spawn((
                    MenuAction::Map(i),
                    Button,
                    LockHighlight::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(24.0),
                        top: px(ROW_TOP + row as f32 * ROW_PITCH),
                        width: px(ROW_W),
                        height: px(ROW_H),
                        ..default()
                    },
                ))
                .with_children(|b| {
                    if let Some(hi) = &art.select_highlight {
                        b.spawn((
                            Hilight,
                            Visibility::Hidden,
                            MaterialNode(hi.clone()),
                            abs(1.0, -20.0, -6.0, ROW_W + 20.0, ROW_H + 12.0),
                        ));
                    }
                    outlined_text(
                        b,
                        abs(1.0, 0.0, 2.0, ROW_W, 16.0),
                        (),
                        (),
                        GlueText {
                            text: map.name,
                            size: 15.0,
                            color: GOLD,
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                    let levels = if lo == hi {
                        format!("Level {lo}")
                    } else {
                        format!("Level {lo}-{hi}")
                    };
                    outlined_text(
                        b,
                        abs(1.0, 0.0, 19.0, ROW_W, 14.0),
                        (),
                        (),
                        GlueText {
                            text: &format!("{levels}  {}", map.zone),
                            size: 12.0,
                            color: Color::WHITE,
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                });
        }

        let diff = selection.difficulty;
        let affixes = selection.affixes;
        let kind = GlueBtnKind::Normal;

        // Hardcore Mode Toggle Button
        let mut b_hc = frame.spawn((
            MenuAction::ToggleHardcore,
            GlueBtn,
            GlueDisabled(false),
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(24.0),
                bottom: px(98.0),
                width: px(ROW_W),
                height: px(36.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ));
        let tc = kind.tex_coords();
        if let Some(tc) = tc {
            b_hc.insert(tc);
        }
        match &art.button_up {
            Some((up, size)) => {
                b_hc.insert(ImageNode {
                    image: up.clone(),
                    rect: Some(tc_rect(*size, tc.map(|t| t.0).unwrap_or(BUTTON_TC))),
                    ..default()
                });
            }
            None => {
                b_hc.insert((FallbackFace, BackgroundColor(BTN_BG)));
            }
        }
        b_hc.with_children(|inner| {
            if let Some(hi) = &art.button_hi {
                inner.spawn((
                    Hilight,
                    Visibility::Hidden,
                    MaterialNode(hi.clone()),
                    overlay(),
                ));
            }
            let (font_size, offset) = kind.caption();
            let (label, color) = if selection.is_hardcore {
                ("[x] Hardcore (Mortal)", Color::srgb(1.0, 0.25, 0.25))
            } else {
                ("[ ] Standard (Softcore)", Color::srgb(0.8, 0.8, 0.8))
            };
            outlined_text(
                inner,
                Node {
                    left: Val::Px(offset.x),
                    top: Val::Px(1.0 - offset.y),
                    ..default()
                },
                (),
                HardcoreText,
                GlueText {
                    text: label,
                    size: font_size - 1.0,
                    color,
                    wrap: false,
                },
                f,
                1.0,
            );
        });

        // Challenge Affixes Cycle Button
        let mut b_affix = frame.spawn((
            MenuAction::CycleAffixes,
            GlueBtn,
            GlueDisabled(false),
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(24.0),
                bottom: px(56.0),
                width: px(ROW_W),
                height: px(36.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ));
        let tc = kind.tex_coords();
        if let Some(tc) = tc {
            b_affix.insert(tc);
        }
        match &art.button_up {
            Some((up, size)) => {
                b_affix.insert(ImageNode {
                    image: up.clone(),
                    rect: Some(tc_rect(*size, tc.map(|t| t.0).unwrap_or(BUTTON_TC))),
                    ..default()
                });
            }
            None => {
                b_affix.insert((FallbackFace, BackgroundColor(BTN_BG)));
            }
        }
        b_affix.with_children(|inner| {
            if let Some(hi) = &art.button_hi {
                inner.spawn((
                    Hilight,
                    Visibility::Hidden,
                    MaterialNode(hi.clone()),
                    overlay(),
                ));
            }
            let (font_size, offset) = kind.caption();
            outlined_text(
                inner,
                Node {
                    left: Val::Px(offset.x),
                    top: Val::Px(1.0 - offset.y),
                    ..default()
                },
                (),
                AffixText,
                GlueText {
                    text: &affixes.label(),
                    size: font_size - 1.0,
                    color: affix_color(&affixes),
                    wrap: false,
                },
                f,
                1.0,
            );
        });

        // Difficulty Cycle Button
        let mut b = frame.spawn((
            MenuAction::CycleDifficulty,
            GlueBtn,
            GlueDisabled(false),
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(24.0),
                bottom: px(14.0),
                width: px(ROW_W),
                height: px(36.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ));
        let tc = kind.tex_coords();
        if let Some(tc) = tc {
            b.insert(tc);
        }
        match &art.button_up {
            Some((up, size)) => {
                b.insert(ImageNode {
                    image: up.clone(),
                    rect: Some(tc_rect(*size, tc.map(|t| t.0).unwrap_or(BUTTON_TC))),
                    ..default()
                });
            }
            None => {
                b.insert((FallbackFace, BackgroundColor(BTN_BG)));
            }
        }
        b.with_children(|inner| {
            if let Some(hi) = &art.button_hi {
                inner.spawn((
                    Hilight,
                    Visibility::Hidden,
                    MaterialNode(hi.clone()),
                    overlay(),
                ));
            }
            let (font_size, offset) = kind.caption();
            outlined_text(
                inner,
                Node {
                    left: Val::Px(offset.x),
                    top: Val::Px(1.0 - offset.y),
                    ..default()
                },
                (),
                DifficultyText,
                GlueText {
                    text: &format!("Difficulty: {}", diff.name()),
                    size: font_size,
                    color: difficulty_color(diff),
                    wrap: false,
                },
                f,
                1.0,
            );
        });
    });
}

fn current_bonus_str(kind: UpgradeKind, rank: u8) -> String {
    match kind {
        UpgradeKind::Might => format!("Current: +{:.0}% damage", rank as f32 * 5.0),
        UpgradeKind::Vitality => format!("Current: +{:.0}% max HP", rank as f32 * 10.0),
        UpgradeKind::Haste => format!("Current: +{:.0}% haste/CDR", rank as f32 * 5.0),
        UpgradeKind::Magnet => format!("Current: +{:.1} yd radius", rank as f32 * 1.5),
        UpgradeKind::DashCd => format!("Current: -{:.1}s dash cooldown", rank as f32 * 0.3),
        UpgradeKind::Greed => format!("Current: +{:.0}% gold/XP", rank as f32 * 10.0),
        UpgradeKind::Channeler => format!("Current: +{:.0}% ultimate charge", rank as f32 * 10.0),
        UpgradeKind::Fortune => format!("Current: +{:.0}% chest luck & card rarity", rank as f32 * 5.0),
    }
}

fn armory_btn<T: Bundle>(
    parent: &mut ChildSpawnerCommands,
    art: &GlueArt,
    font: &Handle<Font>,
    action: MenuAction,
    tag: T,
    caption: &str,
    left: f32,
    top: f32,
    w: f32,
    h: f32,
) {
    let px = Val::Px;
    let kind = GlueBtnKind::Small;
    let mut b = parent.spawn((
        action,
        GlueBtn,
        GlueDisabled(false),
        Button,
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            width: px(w),
            height: px(h),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ));
    let tc = kind.tex_coords();
    if let Some(tc) = tc {
        b.insert(tc);
    }
    match &art.button_up {
        Some((up, size)) => {
            b.insert(ImageNode {
                image: up.clone(),
                rect: Some(tc_rect(*size, tc.map(|t| t.0).unwrap_or(BUTTON_TC))),
                ..default()
            });
        }
        None => {
            b.insert((FallbackFace, BackgroundColor(BTN_BG)));
        }
    }
    b.with_children(|inner| {
        if let Some(hi) = &art.button_hi {
            inner.spawn((
                Hilight,
                Visibility::Hidden,
                MaterialNode(hi.clone()),
                overlay(),
            ));
        }
        let (font_size, offset) = kind.caption();
        outlined_text(
            inner,
            Node {
                left: Val::Px(offset.x),
                top: Val::Px(1.0 - offset.y),
                ..default()
            },
            (),
            tag,
            GlueText {
                text: caption,
                size: font_size,
                color: GOLD,
                wrap: false,
            },
            font,
            1.0,
        );
    });
}

/// The Armory Upgrades panel, between the character creation tower and the preview hero.
fn armory_frame(
    ui: &mut ChildSpawnerCommands,
    art: &GlueArt,
    f: &Handle<Font>,
    account: &save::Account,
) {
    let px = Val::Px;
    ui.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(250.0),
        top: px(74.0),
        width: px(260.0),
        height: px(TOWER_H),
        ..default()
    })
    .with_children(|frame| {
        if let (Some(bg), Some(border)) = (&art.tooltip_bg, &art.name_border) {
            frame.spawn((
                tiled_bg_node(bg.clone(), NAME_EDGE, 1.0, FRAME_FILL),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(10.0),
                    right: px(5.0),
                    top: px(4.0),
                    bottom: px(9.0),
                    ..default()
                },
            ));
            backdrop_border(frame, border, NAME_EDGE, FRAME_BORDER);
        } else {
            frame.spawn((BackgroundColor(FRAME_FILL), crate::glue::widgets::overlay()));
        }

        outlined_text(
            frame,
            Node {
                position_type: PositionType::Absolute,
                top: px(12.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            (),
            (),
            GlueText {
                text: "Armory Upgrades",
                size: 18.0,
                color: GOLD,
                wrap: false,
            },
            f,
            1.0,
        );

        outlined_text(
            frame,
            Node {
                position_type: PositionType::Absolute,
                top: px(34.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            (),
            ArmoryGoldText,
            GlueText {
                text: &format!(
                    "Gold: {}g | Gear: {}/{} | Trophies: {}/{}",
                    account.gold,
                    account.equipped.len(),
                    account.armory.len(),
                    account.achievements.len(),
                    super::achievements::ACHIEVEMENTS.len()
                ),
                size: 13.0,
                color: GOLD,
                wrap: false,
            },
            f,
            1.0,
        );

        let row_top = 54.0;
        let row_pitch = 64.0;
        let row_w = 236.0;

        for (idx, &kind) in UpgradeKind::ALL.iter().enumerate() {
            let rank = account.upgrade_rank(kind);
            let top_y = row_top + idx as f32 * row_pitch;

            frame
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(12.0),
                    top: px(top_y),
                    width: px(row_w),
                    height: px(58.0),
                    ..default()
                })
                .with_children(|row| {
                    let rank_str = format!("{}  [{}/{}]", kind.name(), rank, save::MAX_UPGRADE_RANK);
                    outlined_text(
                        row,
                        abs(1.0, 4.0, 0.0, 140.0, 16.0),
                        (),
                        UpgradeRankText(kind),
                        GlueText {
                            text: &rank_str,
                            size: 13.0,
                            color: GOLD,
                            wrap: false,
                        },
                        f,
                        1.0,
                    );

                    let cost_str = match account.upgrade_cost(kind) {
                        Some(cost) => format!("+ {}g", cost),
                        None => "MAX".to_string(),
                    };
                    armory_btn(
                        row,
                        art,
                        f,
                        MenuAction::Upgrade(kind),
                        UpgradeBtnText(kind),
                        &cost_str,
                        144.0,
                        -2.0,
                        88.0,
                        26.0,
                    );

                    outlined_text(
                        row,
                        abs(1.0, 4.0, 20.0, row_w, 14.0),
                        (),
                        (),
                        GlueText {
                            text: kind.description(),
                            size: 10.5,
                            color: Color::srgb(0.75, 0.75, 0.75),
                            wrap: false,
                        },
                        f,
                        1.0,
                    );

                    let bonus_str = current_bonus_str(kind, rank);
                    outlined_text(
                        row,
                        abs(1.0, 4.0, 36.0, row_w, 14.0),
                        (),
                        UpgradeBonusText(kind),
                        GlueText {
                            text: &bonus_str,
                            size: 10.5,
                            color: Color::srgb(0.5, 0.9, 0.5),
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                });
        }

        // Graveyard Memorial Hall
        frame
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(12.0),
                top: px(444.0),
                width: px(row_w),
                height: px(142.0),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|g| {
                outlined_text(
                    g,
                    Node {
                        margin: UiRect::bottom(px(4.0)),
                        ..default()
                    },
                    (),
                    (),
                    GlueText {
                        text: "Graveyard Memorial",
                        size: 13.0,
                        color: GOLD,
                        wrap: false,
                    },
                    f,
                    1.0,
                );

                if account.graveyard.is_empty() {
                    outlined_text(
                        g,
                        Node::default(),
                        (),
                        (),
                        GlueText {
                            text: "No fallen legends. Walk with honor!",
                            size: 10.5,
                            color: Color::srgb(0.65, 0.65, 0.65),
                            wrap: false,
                        },
                        f,
                        1.0,
                    );
                } else {
                    for hero in account.graveyard.iter().rev().take(2) {
                        let mins = hero.survived_secs as u32 / 60;
                        let secs = hero.survived_secs as u32 % 60;
                        let line1 = format!("† {} Lvl {} ({})", hero.name, hero.level, hero.zone);
                        let line2 = format!("Slain by {} ({:02}:{:02})", hero.killed_by, mins, secs);

                        outlined_text(
                            g,
                            Node {
                                margin: UiRect::bottom(px(1.0)),
                                ..default()
                            },
                            (),
                            (),
                            GlueText {
                                text: &line1,
                                size: 10.5,
                                color: Color::srgb(0.9, 0.4, 0.4),
                                wrap: false,
                            },
                            f,
                            1.0,
                        );
                        outlined_text(
                            g,
                            Node {
                                margin: UiRect::bottom(px(4.0)),
                                ..default()
                            },
                            (),
                            (),
                            GlueText {
                                text: &line2,
                                size: 9.5,
                                color: Color::srgb(0.7, 0.7, 0.7),
                                wrap: false,
                            },
                            f,
                            1.0,
                        );
                    }
                }
            });

        let btn_h = 28.0;
        let btn_top = TOWER_H - 46.0;

        frame
            .spawn(Node {
                position_type: PositionType::Absolute,
                left: px(12.0),
                top: px(btn_top),
                width: px(236.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|r| {
                glue_button(
                    r,
                    art,
                    f,
                    MenuAction::OpenArmoryModal,
                    "Armory [C]",
                    114.0,
                    btn_h,
                    GlueBtnKind::Small,
                    1.0,
                );
                glue_button(
                    r,
                    art,
                    f,
                    MenuAction::RefundUpgrades,
                    "Refund All",
                    114.0,
                    btn_h,
                    GlueBtnKind::Small,
                    1.0,
                );
            });
    });
}

#[allow(clippy::too_many_arguments)]
pub(super) fn menu_clicks(
    clicks: Res<GlueClicks>,
    actions: Query<(Entity, &MenuAction)>,
    phase: Res<State<Phase>>,
    creatures: Option<Res<crate::entities::Creatures>>,
    mut selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
    mut account: ResMut<save::Account>,
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    let Some(creatures) = creatures.as_deref() else {
        return;
    };
    let Some(action) = actions
        .iter()
        .find(|(e, _)| clicks.hit(*e))
        .map(|(_, a)| *a)
    else {
        return;
    };
    let all = all_looks(creatures);
    let cur = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map(|&(_, r, s)| (r, s));
    let (race, sex) = (cur.map(|c| c.0), cur.map_or(0, |c| c.1));
    match action {
        MenuAction::Map(i) => {
            if selection.map != i {
                selection.map = i;
                selection.version += 1;
            }
        }
        MenuAction::Enter => {
            if start_run(&mut run, &mut selection, Some(&account)) {
                next.set(Phase::Playing);
            }
        }
        MenuAction::EnterHub => {
            next.set(Phase::Hub);
        }
        MenuAction::Upgrade(kind) => {
            account.purchase_upgrade(kind);
        }
        MenuAction::RefundUpgrades => {
            account.refund_upgrades();
        }
        MenuAction::OpenArmoryModal => {
            run.vault.is_open = true;
            run.version += 1;
        }
        MenuAction::Randomize => {
            let picks: Vec<(usize, usize)> = all
                .iter()
                .enumerate()
                .flat_map(|(c, looks)| (0..looks.len()).map(move |l| (c, l)))
                .collect();
            if !picks.is_empty() {
                let (c, l) = picks[rng.index(picks.len())];
                set(&mut selection, &all, c, l);
            }
        }
        // The class keeps the race and gender where it can.
        MenuAction::Class(c) => {
            let looks = &all[c];
            let l = find(looks, race, Some(sex))
                .or_else(|| find(looks, race, None))
                .or_else(|| find(looks, None, Some(sex)))
                .or_else(|| find(looks, None, None));
            if let Some(l) = l {
                set(&mut selection, &all, c, l);
            }
        }
        // The race keeps the class where it can, else takes the first class it has.
        MenuAction::Race(r) => {
            let classes = selection.class.into_iter().chain(0..all.len());
            for c in classes {
                let l = find(&all[c], Some(r), Some(sex)).or_else(|| find(&all[c], Some(r), None));
                if let Some(l) = l {
                    set(&mut selection, &all, c, l);
                    break;
                }
            }
        }
        MenuAction::Sex(s) => {
            if let Some(c) = selection.class {
                if let Some(l) = find(&all[c], race, Some(s)) {
                    set(&mut selection, &all, c, l);
                }
            }
        }
        MenuAction::CycleDifficulty => {
            selection.difficulty = selection.difficulty.next();
            selection.version += 1;
        }
        MenuAction::CycleAffixes => {
            selection.affixes = selection.affixes.next();
            selection.version += 1;
        }
        MenuAction::ToggleHardcore => {
            selection.is_hardcore = !selection.is_hardcore;
            selection.version += 1;
        }
    }
    save_pick(&selection);
}

/// The screen's per-frame state: the locked highlights, hover labels, which races and classes
/// the pick allows, the gender on the race faces, the name line and Enter World.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh_menu_ui(
    selection: Res<Selection>,
    creatures: Option<Res<crate::entities::Creatures>>,
    art: Res<GlueArt>,
    account: Res<save::Account>,
    mut buttons: Query<(
        &MenuAction,
        &Interaction,
        &Children,
        Option<&mut LockHighlight>,
        Option<&mut GlueDisabled>,
        &mut Node,
        &mut Visibility,
    )>,
    mut labels: Query<&mut Visibility, (With<HoverLabel>, Without<MenuAction>)>,
    mut faces: Query<(&RaceFace, &mut ImageNode)>,
    mut texts: ParamSet<(
        Query<(&mut Text, &mut TextColor), With<PickName>>,
        Query<(&mut Text, &mut TextColor), With<DifficultyText>>,
        Query<(
            &mut Text,
            Option<&TowerGoldText>,
            Option<&ArmoryGoldText>,
            Option<&UpgradeRankText>,
            Option<&UpgradeBtnText>,
            Option<&UpgradeBonusText>,
        )>,
        Query<(&mut Text, &mut TextColor), With<RacialTraitText>>,
        Query<(&mut Text, &mut TextColor), With<AffixText>>,
        Query<(&mut Text, &mut TextColor), With<HardcoreText>>,
    )>,
    mut cache: Local<(Option<u32>, bool, Vec<Vec<(u32, u8, u8)>>)>,
) {
    let have = creatures.is_some();
    if cache.0 != Some(selection.version) || cache.1 != have {
        *cache = (
            Some(selection.version),
            have,
            creatures.as_deref().map(all_looks).unwrap_or_default(),
        );
    }
    let all = &cache.2;
    let cur = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map(|&(_, r, s)| (r, s));

    let mut hovered_race = None;
    for (action, interaction, children, lock, disabled, mut node, mut vis) in &mut buttons {
        if let MenuAction::Race(r) = *action {
            if *interaction == Interaction::Hovered {
                hovered_race = Some(r);
            }
        }
        let selected = match *action {
            MenuAction::Race(r) => cur.is_some_and(|c| c.0 == r),
            MenuAction::Sex(s) => cur.is_some_and(|c| c.1 == s),
            MenuAction::Class(i) => selection.class == Some(i),
            MenuAction::Map(i) => selection.map == i,
            MenuAction::Randomize
            | MenuAction::Enter
            | MenuAction::EnterHub
            | MenuAction::CycleDifficulty
            | MenuAction::CycleAffixes
            | MenuAction::ToggleHardcore
            | MenuAction::OpenArmoryModal => false,
            MenuAction::Upgrade(_) | MenuAction::RefundUpgrades => false,
        };
        if let Some(mut lock) = lock {
            if lock.0 != selected {
                lock.0 = selected;
            }
        }
        if let Some(mut disabled) = disabled {
            let off = match *action {
                MenuAction::Enter | MenuAction::EnterHub => cur.is_none(),
                MenuAction::Upgrade(kind) => !account.can_purchase_upgrade(kind),
                MenuAction::RefundUpgrades => account.upgrades == save::AccountUpgrades::default(),
                _ => false,
            };
            if disabled.0 != off {
                disabled.0 = off;
            }
        }
        match *action {
            // As on the create screen, a class the race cannot be leaves the grid.
            MenuAction::Class(i) => {
                let ok = cur.is_none_or(|(r, _)| {
                    all.get(i).is_some_and(|l| find(l, Some(r), None).is_some())
                });
                let display = if ok { Display::Flex } else { Display::None };
                if node.display != display {
                    node.display = display;
                }
            }
            MenuAction::Race(r) => {
                let ok = all.is_empty() || all.iter().any(|l| find(l, Some(r), None).is_some());
                let want = if ok {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *vis != want {
                    *vis = want;
                }
            }
            _ => {}
        }
        let show = selected || *interaction != Interaction::None || art.races.is_none();
        for child in children {
            if let Ok(mut label) = labels.get_mut(*child) {
                let want = if show {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if *label != want {
                    *label = want;
                }
            }
        }
    }

    if let (Some((_, size)), Some((_, sex))) = (&art.races, cur) {
        for (face, mut img) in &mut faces {
            if let Some(tc) = race_tc(face.0, sex) {
                let rect = Some(tc_rect(*size, tc));
                if img.rect != rect {
                    img.rect = rect;
                }
            }
        }
    }

    let class = selection.class_def();
    let display_race = hovered_race.or(cur.map(|c| c.0));
    if let Ok((mut t, mut color)) = texts.p0().single_mut() {
        let want = match (class, display_race) {
            (Some(c), Some(r)) => {
                let base = format!("{} {}", race_name(r), c.name);
                if let Some(title) = account.active_title_def() {
                    format!("{} {}", base, title.title)
                } else {
                    base
                }
            }
            _ => String::new(),
        };
        if t.0 != want {
            t.0 = want;
        }
        // In the class's own colour, as the game's raid frames and chat show it.
        let want = class.map_or(GOLD, |c| rgb(c.color));
        if color.0 != want {
            color.0 = want;
        }
    }

    let trait_info = match display_race {
        Some(r) => {
            let t = traits::race_trait(r);
            let mut info = format!("[Racial] {}: {} ({})", race_name(r), t.name(), t.description());
            if let Some(c) = class {
                if let Some(syn) = traits::synergy_for(r, c.id) {
                    info = format!("{}  |  ★ SYNERGY: {} ({})", info, syn.name, syn.description);
                }
            }
            (
                info,
                if hovered_race.is_some() {
                    Color::srgb(1.0, 0.92, 0.45)
                } else {
                    Color::srgb(0.9, 0.85, 0.6)
                },
            )
        }
        None => (String::new(), Color::NONE),
    };
    if let Ok((mut t, mut color)) = texts.p3().single_mut() {
        if t.0 != trait_info.0 {
            t.0 = trait_info.0;
        }
        if color.0 != trait_info.1 {
            color.0 = trait_info.1;
        }
    }

    for (mut t, mut color) in &mut texts.p1() {
        let diff_name = selection.difficulty.name();
        let det = account.determination_for(diff_name);
        let det_suffix = if det > 0 {
            format!(" (+{}% Pity)", det * 5)
        } else {
            String::new()
        };
        let want = format!("Difficulty: {}{}", diff_name, det_suffix);
        if t.0 != want {
            t.0 = want;
        }
        let want_color = difficulty_color(selection.difficulty);
        if color.0 != want_color {
            color.0 = want_color;
        }
    }

    for (mut t, mut color) in &mut texts.p4() {
        let want = selection.affixes.label();
        if t.0 != want {
            t.0 = want;
        }
        let want_color = affix_color(&selection.affixes);
        if color.0 != want_color {
            color.0 = want_color;
        }
    }

    for (mut t, mut color) in &mut texts.p5() {
        let (want, want_color) = if selection.is_hardcore {
            ("[x] Hardcore (Mortal)", Color::srgb(1.0, 0.25, 0.25))
        } else {
            ("[ ] Standard (Softcore)", Color::srgb(0.8, 0.8, 0.8))
        };
        if t.0 != want {
            t.0 = want.to_string();
        }
        if color.0 != want_color {
            color.0 = want_color;
        }
    }

    for (mut text, is_tower, is_armory, rank_opt, btn_opt, bonus_opt) in &mut texts.p2() {
        if is_armory.is_some() {
            let want = format!(
                "Gold: {}g | Gear: {}/{} | Trophies: {}/{}",
                account.gold,
                account.equipped.len(),
                account.armory.len(),
                account.achievements.len(),
                super::achievements::ACHIEVEMENTS.len()
            );
            if text.0 != want {
                text.0 = want;
            }
        } else if is_tower.is_some() {
            let want = format!("Gold: {}g", account.gold);
            if text.0 != want {
                text.0 = want;
            }
        } else if let Some(tag) = rank_opt {
            let kind = tag.0;
            let rank = account.upgrade_rank(kind);
            let want = format!("{}  [{}/{}]", kind.name(), rank, save::MAX_UPGRADE_RANK);
            if text.0 != want {
                text.0 = want;
            }
        } else if let Some(tag) = btn_opt {
            let kind = tag.0;
            let want = match account.upgrade_cost(kind) {
                Some(cost) => format!("+ {}g", cost),
                None => "MAX".to_string(),
            };
            if text.0 != want {
                text.0 = want;
            }
        } else if let Some(tag) = bonus_opt {
            let kind = tag.0;
            let rank = account.upgrade_rank(kind);
            let want = current_bonus_str(kind, rank);
            if text.0 != want {
                text.0 = want;
            }
        }
    }
}
