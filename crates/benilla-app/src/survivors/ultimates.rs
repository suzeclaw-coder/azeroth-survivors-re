//! Class Signature Ultimates: charged by combat kills, activated with Space or Controller LT / North,
//! bringing unique tactical depth to each class.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::data::Effect;
use super::enemies::Enemy;
use super::hero::{self, Hero};
use super::units::{flat, Fx, COLOR_HEAL, COLOR_MELEE, COLOR_SPELL};
use super::{Phase, Rng, Run, Selection};

/// The eight class signature ultimates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum UltimateKind {
    /// Scale up, +100% damage, CC immunity, radial shockwave every 1.5s for 10s (Spell 1022).
    WarriorAvatar,
    /// +250% cooldown recovery & haste, chrono particles for 8s (Spell 80353).
    MageTimeWarp,
    /// Glowing wings (Spell 31884), instant 100% heal, holy shock obliteration bursts for 10s.
    PaladinAvengingWrath,
    /// Blinks across 12 nearest targets in 1.5s with guaranteed critical strikes (Spell 51690).
    RogueKillingSpree,
    /// Demon form, Immolation Aura ticking 80 dmg/s to all nearby enemies for 10s (Spell 47241).
    WarlockMetamorphosis,
    /// Storm elemental form, continuous chain lightning barrage every 0.25s for 8s (Spell 114049).
    ShamanAscendance,
    /// Red glowing rage aura, stampede tramples enemies in a wide forward corridor for 10s (Spell 19574).
    HunterBestialWrath,
    /// Falling arcane stars across arena + 20% HP/s tranquility healing rain for 8s (Spell 48465/740).
    DruidStarfall,
}

impl UltimateKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::WarriorAvatar => "Avatar",
            Self::MageTimeWarp => "Time Warp",
            Self::PaladinAvengingWrath => "Avenging Wrath",
            Self::RogueKillingSpree => "Killing Spree",
            Self::WarlockMetamorphosis => "Metamorphosis",
            Self::ShamanAscendance => "Ascendance",
            Self::HunterBestialWrath => "Bestial Wrath",
            Self::DruidStarfall => "Starfall",
        }
    }

    pub fn spell(&self) -> u32 {
        match self {
            Self::WarriorAvatar => 1022,
            Self::MageTimeWarp => 80353,
            Self::PaladinAvengingWrath => 31884,
            Self::RogueKillingSpree => 51690,
            Self::WarlockMetamorphosis => 47241,
            Self::ShamanAscendance => 114049,
            Self::HunterBestialWrath => 19574,
            Self::DruidStarfall => 48465,
        }
    }

    pub fn duration(&self) -> f32 {
        match self {
            Self::WarriorAvatar => 10.0,
            Self::MageTimeWarp => 8.0,
            Self::PaladinAvengingWrath => 10.0,
            Self::RogueKillingSpree => 1.5,
            Self::WarlockMetamorphosis => 10.0,
            Self::ShamanAscendance => 8.0,
            Self::HunterBestialWrath => 10.0,
            Self::DruidStarfall => 8.0,
        }
    }

    pub fn for_class(class_id: u8) -> Option<Self> {
        match class_id {
            1 => Some(Self::WarriorAvatar),
            2 => Some(Self::PaladinAvengingWrath),
            3 => Some(Self::HunterBestialWrath),
            4 => Some(Self::RogueKillingSpree),
            5 => Some(Self::PaladinAvengingWrath), // Priest fallback to holy signature
            7 => Some(Self::ShamanAscendance),
            8 => Some(Self::MageTimeWarp),
            9 => Some(Self::WarlockMetamorphosis),
            11 => Some(Self::DruidStarfall),
            _ => None,
        }
    }
}

/// Normal kill: +0.6%, Elite kill: +6.0%, Boss kill: +25.0%. Scaled by Channeler upgrade. Clamped to 100.0.
pub(super) fn charge_ultimate_on_kill(run: &mut Run, is_elite: bool, is_boss: bool) {
    let base = if is_boss {
        25.0
    } else if is_elite {
        6.0
    } else {
        0.6
    };
    let mult = 1.0 + (run.upgrades.channeler as f32 * 0.10);
    run.ultimate_charge = (run.ultimate_charge + base * mult).clamp(0.0, 100.0);
}

/// Listens for `KeyCode::Space` or Controller `GamepadButton::LeftTrigger2` / `GamepadButton::North`.
/// When `run.ultimate_charge >= 100.0` and no ultimate is active:
///   Drains charge to 0.0.
///   Determines class from `selection.class` (or hero's starting class def).
///   Activates the ultimate for its duration, plays cast VFX via `Fx`, sound, and displays banner.
#[allow(clippy::too_many_arguments)]
pub(super) fn cast_ultimate(
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    hero: Query<Entity, With<Hero>>,
    mut rig: ResMut<hero::CamRig>,
    mut fx: Fx,
    mut sounds: MessageWriter<crate::sound::GlueSound>,
) {
    if *phase.get() != Phase::Playing || run.dead_t.is_some() {
        return;
    }
    let gp_cast = gamepads.iter().any(|gp| {
        gp.just_pressed(GamepadButton::LeftTrigger2) || gp.just_pressed(GamepadButton::North)
    });
    let key_cast = keys.just_pressed(KeyCode::Space);
    if !key_cast && !gp_cast {
        return;
    }
    if run.ultimate_charge < 100.0 || run.ultimate_active.is_some() {
        return;
    }
    let Ok(hero_e) = hero.single() else {
        return;
    };

    let class_id = selection
        .class_def()
        .map(|c| c.id)
        .unwrap_or(run.class.id);

    let Some(kind) = UltimateKind::for_class(class_id) else {
        return;
    };

    run.ultimate_charge = 0.0;
    let dur = kind.duration();
    run.ultimate_active = Some((kind, dur));
    fx.cast(hero_e, kind.spell(), &[hero_e], None);
    rig.shake(1.5, 0.45);
    sounds.write(crate::sound::GlueSound("gsCharacterSelectionEnterWorld"));

    // Synergy banner announcements
    let is_orc_warrior = run.race == 2 && class_id == 1;
    let is_ne_rogue = run.race == 4 && class_id == 4;
    let is_human_mage = run.race == 1 && class_id == 8;

    let banner_text = if is_orc_warrior && kind == UltimateKind::WarriorAvatar {
        "★ SYNERGY: ORC BLOOD FURY AVATAR (+200% DMG)!".to_string()
    } else if is_ne_rogue && kind == UltimateKind::RogueKillingSpree {
        "★ SYNERGY: ELUSIVE KILLING SPREE (100% EVASION)!".to_string()
    } else if is_human_mage && kind == UltimateKind::MageTimeWarp {
        "★ SYNERGY: ARCANE VACUUM (+100% MAGNET)!".to_string()
    } else {
        format!("{} UNLEASHED!", kind.name())
    };
    run.banner = Some((banner_text, 3.5));

    // Special instant effects on activation
    if kind == UltimateKind::PaladinAvengingWrath {
        let stats = run.stats();
        run.hp = stats.max_hp;
        fx.number(hero_e, stats.max_hp, true, COLOR_HEAL);
    }
}

/// Ticking active ultimate duration, applying periodic effects (shockwaves, immolation aura, tranquility heals, lightning bolts) using `Fx` and `MessageWriter<Hit>`.
/// Strictly disjoint queries: `hero: Query<(Entity, &mut Transform), With<Hero>>`, `enemies: Query<(Entity, &Transform, &mut Enemy), Without<Hero>>`.
#[allow(clippy::too_many_arguments)]
pub(super) fn tick_ultimates(
    time: Res<Time>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    world: WorldPoint,
    selection: Res<Selection>,
    mut hero: Query<(Entity, &mut Transform), With<Hero>>,
    enemies: Query<(Entity, &Transform, &mut Enemy), Without<Hero>>,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
    mut ticker: Local<f32>,
    mut heal_timer: Local<f32>,
    mut spree_step: Local<f32>,
) {
    let dt = time.delta_secs();
    if *phase.get() != Phase::Playing || run.dead_t.is_some() || dt <= 0.0 {
        return;
    }

    let Ok((hero_e, mut htf)) = hero.single_mut() else {
        return;
    };

    let Some((kind, mut rem)) = run.ultimate_active else {
        // Reset hero scale if not in Avatar
        if htf.scale != Vec3::ONE {
            htf.scale = Vec3::ONE;
        }
        *ticker = 0.0;
        *heal_timer = 0.0;
        *spree_step = 0.0;
        return;
    };

    rem -= dt;
    if rem <= 0.0 {
        run.ultimate_active = None;
        if htf.scale != Vec3::ONE {
            htf.scale = Vec3::ONE;
        }
        *ticker = 0.0;
        *heal_timer = 0.0;
        *spree_step = 0.0;
        return;
    }
    run.ultimate_active = Some((kind, rem));

    let hpos = htf.translation;
    let stats = run.stats();
    let facing = flat(htf.rotation * Vec3::NEG_Z).normalize_or(Vec3::NEG_Z);

    let ground = |p: Vec3| world.terrain_height_under(p).unwrap_or(p.y);
    let map = selection.map_def();
    let center = hero::arena(map);
    let leash = |p: Vec3| {
        let off = flat(p - center);
        if off.length() > map.leash {
            center + off.normalize() * map.leash
        } else {
            p
        }
    };

    match kind {
        UltimateKind::WarriorAvatar => {
            // Scale up and CC immunity
            htf.scale = Vec3::splat(1.35);
            run.iframe_t = run.iframe_t.max(dt);

            // Radial shockwave every 1.5s for 10s (Spell 1022)
            *ticker += dt;
            if *ticker >= 1.5 {
                *ticker -= 1.5;
                fx.cast(hero_e, 1022, &[hero_e], Some(hpos));
                for (e, tf, en) in &enemies {
                    if !en.dead && hero::reach(hpos, tf.translation) <= 12.0 {
                        fx.impact(e, 1022);
                        hits.write(Hit {
                            spell: 1022,
                            key: "avatar_shockwave",
                            target: e,
                            damage: 180.0 * stats.might,
                            dot: 0.0,
                            dot_dur: 0.0,
                            effect: Effect::Stun(1.5),
                            mark: 0,
                            mark_dur: 0.0,
                            splash: 0.0,
                            crit: rng.f32() < stats.crit,
                            color: COLOR_MELEE,
                        });
                    }
                }
            }
        }
        UltimateKind::MageTimeWarp => {
            // Chrono particles for 8s (Spell 80353)
            *ticker += dt;
            if *ticker >= 1.0 {
                *ticker -= 1.0;
                fx.cast(hero_e, 80353, &[hero_e], None);
            }
        }
        UltimateKind::PaladinAvengingWrath => {
            // Glowing wings (Spell 31884) & Holy shock obliteration bursts
            *ticker += dt;
            if *ticker >= 0.75 {
                *ticker -= 0.75;
                fx.cast(hero_e, 31884, &[hero_e], None);

                let mut near: Vec<(Entity, Vec3, f32)> = enemies
                    .iter()
                    .filter(|(_, _, en)| !en.dead)
                    .map(|(e, tf, _)| (e, tf.translation, hero::reach(hpos, tf.translation)))
                    .filter(|&(_, _, d)| d <= 22.0)
                    .collect();
                near.sort_by(|a, b| a.2.total_cmp(&b.2));

                for (e, pos, _) in near.into_iter().take(3) {
                    fx.cast(hero_e, 20473, &[e], Some(pos));
                    hits.write(Hit {
                        spell: 20473,
                        key: "holy_shock_burst",
                        target: e,
                        damage: 160.0 * stats.might,
                        dot: 0.0,
                        dot_dur: 0.0,
                        effect: Effect::None,
                        mark: 0,
                        mark_dur: 0.0,
                        splash: 0.0,
                        crit: true,
                        color: COLOR_SPELL,
                    });
                }
            }
        }
        UltimateKind::RogueKillingSpree => {
            // Night Elf Rogue synergy: 100% evasion / i-frames during Killing Spree
            if run.race == 4 {
                run.iframe_t = run.iframe_t.max(0.2);
            }
            // Blinks across 12 nearest targets in 1.5s with guaranteed critical strikes (Spell 51690)
            *spree_step += dt;
            if *spree_step >= 0.125 {
                *spree_step -= 0.125;
                let target = enemies
                    .iter()
                    .filter(|(_, _, en)| !en.dead)
                    .map(|(e, tf, _)| (e, tf.translation, hero::reach(hpos, tf.translation)))
                    .filter(|&(_, _, d)| d <= 24.0)
                    .min_by(|a, b| a.2.total_cmp(&b.2));

                if let Some((e, pos, _)) = target {
                    let dir = flat(pos - hpos).normalize_or(facing);
                    let mut dest = leash(pos - dir * 1.2);
                    dest.y = ground(dest);
                    htf.translation = dest;
                    htf.rotation = Quat::from_rotation_y(super::units::yaw_toward(dir));

                    fx.cast(hero_e, 51690, &[e], None);
                    fx.swing(hero_e, e, 150.0 * stats.might);
                    hits.write(Hit {
                        spell: 51690,
                        key: "killing_spree",
                        target: e,
                        damage: 150.0 * stats.might,
                        dot: 0.0,
                        dot_dur: 0.0,
                        effect: Effect::None,
                        mark: 0,
                        mark_dur: 0.0,
                        splash: 0.0,
                        crit: true,
                        color: COLOR_MELEE,
                    });
                }
            }
        }
        UltimateKind::WarlockMetamorphosis => {
            // Demon form, Immolation Aura ticking 80 dmg/s to all nearby enemies for 10s (Spell 47241)
            *ticker += dt;
            if *ticker >= 0.5 {
                *ticker -= 0.5;
                fx.cast(hero_e, 47241, &[hero_e], None);
                for (e, tf, en) in &enemies {
                    if !en.dead && hero::reach(hpos, tf.translation) <= 10.0 {
                        fx.impact(e, 47241);
                        hits.write(Hit {
                            spell: 47241,
                            key: "immolation_aura",
                            target: e,
                            damage: 40.0 * stats.might,
                            dot: 0.0,
                            dot_dur: 0.0,
                            effect: Effect::None,
                            mark: 0,
                            mark_dur: 0.0,
                            splash: 0.0,
                            crit: rng.f32() < stats.crit,
                            color: COLOR_SPELL,
                        });
                    }
                }
                // Undead Warlock synergy: Soul Siphon Aura (heals 2% max HP per tick)
                if run.race == 5 {
                    let leech = stats.max_hp * 0.02;
                    run.hp = (run.hp + leech).min(stats.max_hp);
                    fx.number(hero_e, leech, false, COLOR_HEAL);
                }
            }
        }
        UltimateKind::ShamanAscendance => {
            // Storm elemental form, continuous chain lightning barrage every 0.25s for 8s (Spell 114049)
            *ticker += dt;
            if *ticker >= 0.25 {
                *ticker -= 0.25;
                let primary = enemies
                    .iter()
                    .filter(|(_, _, en)| !en.dead)
                    .map(|(e, tf, _)| (e, tf.translation, hero::reach(hpos, tf.translation)))
                    .filter(|&(_, _, d)| d <= 25.0)
                    .min_by(|a, b| a.2.total_cmp(&b.2));

                if let Some((prime_e, prime_pos, _)) = primary {
                    fx.cast(hero_e, 114049, &[prime_e], None);
                    hits.write(Hit {
                        spell: 114049,
                        key: "ascendance_lightning",
                        target: prime_e,
                        damage: 65.0 * stats.might,
                        dot: 0.0,
                        dot_dur: 0.0,
                        effect: Effect::None,
                        mark: 0,
                        mark_dur: 0.0,
                        splash: 0.0,
                        crit: rng.f32() < stats.crit,
                        color: COLOR_SPELL,
                    });

                    let chain_targets: Vec<Entity> = enemies
                        .iter()
                        .filter(|(e, _, en)| *e != prime_e && !en.dead)
                        .filter(|(_, tf, _)| hero::reach(prime_pos, tf.translation) <= 12.0)
                        .take(4)
                        .map(|(e, _, _)| e)
                        .collect();

                    for ce in chain_targets {
                        fx.impact(ce, 114049);
                        hits.write(Hit {
                            spell: 114049,
                            key: "ascendance_chain",
                            target: ce,
                            damage: 55.0 * stats.might,
                            dot: 0.0,
                            dot_dur: 0.0,
                            effect: Effect::None,
                            mark: 0,
                            mark_dur: 0.0,
                            splash: 0.0,
                            crit: rng.f32() < stats.crit,
                            color: COLOR_SPELL,
                        });
                    }
                }
            }
        }
        UltimateKind::HunterBestialWrath => {
            // Red glowing rage aura, stampede tramples enemies in a wide forward corridor (Spell 19574)
            *ticker += dt;
            if *ticker >= 0.75 {
                *ticker -= 0.75;
                fx.cast(hero_e, 19574, &[hero_e], None);

                let fwd = facing;
                let right = Vec3::new(-fwd.z, 0.0, fwd.x);

                for (e, tf, en) in &enemies {
                    if en.dead {
                        continue;
                    }
                    let off = flat(tf.translation - hpos);
                    let dist_fwd = off.dot(fwd);
                    let dist_lat = off.dot(right).abs();

                    if dist_fwd >= 0.0 && dist_fwd <= 26.0 && dist_lat <= 4.5 {
                        fx.impact(e, 19574);
                        hits.write(Hit {
                            spell: 19574,
                            key: "stampede_trample",
                            target: e,
                            damage: 95.0 * stats.might,
                            dot: 0.0,
                            dot_dur: 0.0,
                            effect: Effect::Stun(0.8),
                            mark: 0,
                            mark_dur: 0.0,
                            splash: 0.0,
                            crit: rng.f32() < stats.crit,
                            color: COLOR_MELEE,
                        });
                    }
                }
            }
        }
        UltimateKind::DruidStarfall => {
            // Falling arcane stars across arena + 20% HP/s tranquility healing rain for 8s (Spell 48465/740)
            *ticker += dt;
            if *ticker >= 0.3 {
                *ticker -= 0.3;
                let mut valid_targets: Vec<(Entity, Vec3)> = enemies
                    .iter()
                    .filter(|(_, _, en)| !en.dead)
                    .map(|(e, tf, _)| (e, tf.translation))
                    .filter(|(_, pos)| hero::reach(hpos, *pos) <= 26.0)
                    .collect();

                for _ in 0..3 {
                    if valid_targets.is_empty() {
                        break;
                    }
                    let idx = rng.index(valid_targets.len());
                    let (e, pos) = valid_targets.swap_remove(idx);
                    fx.cast(hero_e, 48465, &[e], Some(pos));
                    hits.write(Hit {
                        spell: 48465,
                        key: "starfall",
                        target: e,
                        damage: 80.0 * stats.might,
                        dot: 0.0,
                        dot_dur: 0.0,
                        effect: Effect::None,
                        mark: 0,
                        mark_dur: 0.0,
                        splash: 0.0,
                        crit: rng.f32() < stats.crit,
                        color: COLOR_SPELL,
                    });
                }
            }

            // 20% HP/s tranquility healing rain
            let heal_per_frame = stats.max_hp * 0.20 * dt;
            run.hp = (run.hp + heal_per_frame).min(stats.max_hp);

            *heal_timer += dt;
            if *heal_timer >= 1.0 {
                *heal_timer -= 1.0;
                fx.cast(hero_e, 740, &[hero_e], None);
                fx.number(hero_e, stats.max_hp * 0.20, false, COLOR_HEAL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_charge_ultimate_on_kill() {
        let mut run = Run::default();
        assert_eq!(run.ultimate_charge, 0.0);

        charge_ultimate_on_kill(&mut run, false, false);
        assert!((run.ultimate_charge - 0.6).abs() < 1e-4);

        charge_ultimate_on_kill(&mut run, true, false);
        assert!((run.ultimate_charge - 6.6).abs() < 1e-4);

        charge_ultimate_on_kill(&mut run, false, true);
        assert!((run.ultimate_charge - 31.6).abs() < 1e-4);

        // Clamp test
        for _ in 0..10 {
            charge_ultimate_on_kill(&mut run, false, true);
        }
        assert_eq!(run.ultimate_charge, 100.0);

        // Channeler bonus test (rank 5 = +50% charge speed)
        let mut ch_run = Run::default();
        ch_run.upgrades.channeler = 5;
        charge_ultimate_on_kill(&mut ch_run, false, false);
        assert!((ch_run.ultimate_charge - 0.9).abs() < 1e-4);
    }

    #[test]
    fn test_ultimate_for_class() {
        assert_eq!(UltimateKind::for_class(1), Some(UltimateKind::WarriorAvatar));
        assert_eq!(UltimateKind::for_class(2), Some(UltimateKind::PaladinAvengingWrath));
        assert_eq!(UltimateKind::for_class(3), Some(UltimateKind::HunterBestialWrath));
        assert_eq!(UltimateKind::for_class(4), Some(UltimateKind::RogueKillingSpree));
        assert_eq!(UltimateKind::for_class(7), Some(UltimateKind::ShamanAscendance));
        assert_eq!(UltimateKind::for_class(8), Some(UltimateKind::MageTimeWarp));
        assert_eq!(UltimateKind::for_class(9), Some(UltimateKind::WarlockMetamorphosis));
        assert_eq!(UltimateKind::for_class(11), Some(UltimateKind::DruidStarfall));
    }

    #[test]
    fn test_ultimate_properties() {
        let avatar = UltimateKind::WarriorAvatar;
        assert_eq!(avatar.name(), "Avatar");
        assert_eq!(avatar.spell(), 1022);
        assert_eq!(avatar.duration(), 10.0);
    }
}
