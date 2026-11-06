//! Tactical Combat Agency: class-specific manual skillshots aimed toward the cursor on Left-Click.

use bevy::prelude::*;

use super::enemies::Enemy;
use super::fct::{CombatTextKind, CombatTextMsg};
use super::hero::{reach, AimPoint, CamRig, Hero};
use super::units::{flat, Fx};
use super::Run;
use crate::sound::GlueSound;

/// Definition for a class's manual skillshot.
#[derive(Clone, Copy, Debug)]
pub struct SkillshotDef {
    pub name: &'static str,
    pub spell: u32,
    pub cooldown: f32,
    #[allow(dead_code)]
    pub description: &'static str,
}

/// Returns the skillshot definition for a given class id.
pub fn class_skillshot(class_id: u8) -> SkillshotDef {
    match class_id {
        1 => SkillshotDef {
            name: "Heroic Throw",
            spell: 57755,
            cooldown: 2.2,
            description: "Piercing throw that stuns enemies in a line for 1.0s.",
        },
        2 => SkillshotDef {
            name: "Judgement",
            spell: 20271,
            cooldown: 2.4,
            description: "Holy strike at cursor; heals hero for 20% of damage dealt.",
        },
        3 => SkillshotDef {
            name: "Concussive Shot",
            spell: 5116,
            cooldown: 2.0,
            description: "Piercing shot slowing enemies by 60% for 3.0s.",
        },
        4 => SkillshotDef {
            name: "Shadowstep Strike",
            spell: 36554,
            cooldown: 2.5,
            description: "Guaranteed critical strike on closest enemy to cursor.",
        },
        5 => SkillshotDef {
            name: "Shadow Word: Death",
            spell: 32379,
            cooldown: 2.5,
            description: "Devastating single-target blast; deals 2.5x damage to foes under 35% HP.",
        },
        7 => SkillshotDef {
            name: "Frost Shock",
            spell: 8056,
            cooldown: 2.0,
            description: "Chilling blast that slows target by 50% and applies Frost mark.",
        },
        8 => SkillshotDef {
            name: "Fire Blast",
            spell: 2136,
            cooldown: 2.2,
            description: "Fiery explosion at cursor splashing in 4.0 yards with Burn.",
        },
        9 => SkillshotDef {
            name: "Shadow Bolt Burst",
            spell: 686,
            cooldown: 2.4,
            description: "High shadow damage bolt inflicting Corruption DoT.",
        },
        11 => SkillshotDef {
            name: "Moonfire",
            spell: 8921,
            cooldown: 2.0,
            description: "Lunar beam from the heavens dealing Arcane damage and burn.",
        },
        _ => SkillshotDef {
            name: "Strike",
            spell: 6603,
            cooldown: 2.0,
            description: "Direct weapon blow.",
        },
    }
}

/// Decrements active skillshot cooldown.
pub(super) fn tick_skillshot(time: Res<Time>, mut run: ResMut<Run>) {
    let dt = time.delta_secs();
    if dt > 0.0 && run.skillshot_cd > 0.0 {
        run.skillshot_cd = (run.skillshot_cd - dt).max(0.0);
    }
}

/// Executes manual aimed skillshots on Left-Click or Gamepad Right Trigger.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn cast_skillshot(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    mut run: ResMut<Run>,
    aim: Res<AimPoint>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut enemies: Query<(Entity, &Transform, &mut Enemy)>,
    mut fx: Fx,
    mut rig: ResMut<CamRig>,
    mut sounds: MessageWriter<GlueSound>,
    mut combat_text: MessageWriter<CombatTextMsg>,
) {
    if run.dead_t.is_some() || run.skillshot_cd > 0.0 {
        return;
    }

    let gp_trigger = gamepads
        .iter()
        .any(|gp| gp.just_pressed(GamepadButton::RightTrigger2));
    let clicked = mouse_buttons.just_pressed(MouseButton::Left) || gp_trigger;
    if !clicked {
        return;
    }

    let Ok((hero_e, htf)) = hero.single() else {
        return;
    };
    let hpos = htf.translation;
    let def = class_skillshot(run.class.id);
    let stats = run.stats();

    // Set cooldown reduced by haste
    let cd = (def.cooldown * (1.0 - stats.haste).max(0.35)).max(0.4);
    run.skillshot_cd = cd;
    run.skillshot_max_cd = cd;

    let target_pos = aim.0.unwrap_or_else(|| hpos + htf.forward() * 12.0);
    let aim_dir = (flat(target_pos - hpos)).normalize_or_zero();

    // Impact sounds and rig shake
    rig.shake(0.8, 0.15);
    sounds.write(GlueSound("igSpellBookOpen"));

    let base_damage = 75.0 * stats.might;

    match run.class.id {
        1 => {
            // Warrior: Heroic Throw in a piercing line (24.0 yds)
            let mut hit_count = 0;
            for (foe_e, foe_tf, mut foe) in &mut enemies {
                if foe.dead {
                    continue;
                }
                let to_foe = flat(foe_tf.translation - hpos);
                let proj = to_foe.dot(aim_dir);
                if proj > 0.0 && proj < 24.0 {
                    let perp = (to_foe - aim_dir * proj).length();
                    if perp <= 2.2 + foe.radius {
                        foe.hp -= base_damage;
                        foe.stun_t = 1.0;
                        fx.impact(foe_e, def.spell);
                        combat_text.write(CombatTextMsg {
                            pos: foe_tf.translation,
                            amount: base_damage,
                            is_crit: false,
                            kind: CombatTextKind::Physical,
                        });
                        run.meter
                            .record_hit(def.spell, "heroic_throw", base_damage, false);
                        hit_count += 1;
                        if hit_count >= 5 {
                            break;
                        }
                    }
                }
            }
        }
        2 => {
            // Paladin: Judgement at cursor, heals hero
            let mut closest: Option<(Entity, f32)> = None;
            for (foe_e, foe_tf, foe) in &enemies {
                if foe.dead {
                    continue;
                }
                let d = reach(target_pos, foe_tf.translation);
                if d <= 6.0 && reach(hpos, foe_tf.translation) <= 22.0 {
                    if closest.map_or(true, |(_, cd)| d < cd) {
                        closest = Some((foe_e, d));
                    }
                }
            }
            if let Some((target_e, _)) = closest {
                if let Ok((_, target_tf, mut foe)) = enemies.get_mut(target_e) {
                    let dmg = base_damage * 1.15;
                    let target_pos = target_tf.translation;
                    foe.hp -= dmg;
                    fx.impact(target_e, def.spell);
                    combat_text.write(CombatTextMsg {
                        pos: target_pos,
                        amount: dmg,
                        is_crit: true,
                        kind: CombatTextKind::Holy,
                    });
                    run.meter.record_hit(def.spell, "judgement", dmg, true);

                    // Heal hero
                    let heal = (dmg * 0.20).max(5.0);
                    run.hp = (run.hp + heal).min(stats.max_hp);
                    combat_text.write(CombatTextMsg {
                        pos: hpos,
                        amount: heal,
                        is_crit: false,
                        kind: CombatTextKind::Heal,
                    });
                }
            }
        }
        3 => {
            // Hunter: Concussive Shot piercing line with 60% slow
            for (foe_e, foe_tf, mut foe) in &mut enemies {
                if foe.dead {
                    continue;
                }
                let to_foe = flat(foe_tf.translation - hpos);
                let proj = to_foe.dot(aim_dir);
                if proj > 0.0 && proj < 28.0 {
                    let perp = (to_foe - aim_dir * proj).length();
                    if perp <= 1.8 + foe.radius {
                        let dmg = base_damage * 0.95;
                        foe.hp -= dmg;
                        foe.slow = 0.60;
                        foe.slow_t = 3.0;
                        fx.impact(foe_e, def.spell);
                        combat_text.write(CombatTextMsg {
                            pos: foe_tf.translation,
                            amount: dmg,
                            is_crit: false,
                            kind: CombatTextKind::Physical,
                        });
                        run.meter
                            .record_hit(def.spell, "concussive_shot", dmg, false);
                    }
                }
            }
        }
        4 => {
            // Rogue: Shadowstep Strike (100% critical strike on closest to cursor)
            let mut closest: Option<Entity> = None;
            let mut min_d = 8.0;
            for (foe_e, foe_tf, foe) in &enemies {
                if foe.dead {
                    continue;
                }
                let d = reach(target_pos, foe_tf.translation);
                if d < min_d && reach(hpos, foe_tf.translation) <= 16.0 {
                    min_d = d;
                    closest = Some(foe_e);
                }
            }
            if let Some(target_e) = closest {
                if let Ok((_, target_tf, mut foe)) = enemies.get_mut(target_e) {
                    let crit_dmg = base_damage * 2.2;
                    let target_pos = target_tf.translation;
                    foe.hp -= crit_dmg;
                    fx.impact(target_e, def.spell);
                    combat_text.write(CombatTextMsg {
                        pos: target_pos,
                        amount: crit_dmg,
                        is_crit: true,
                        kind: CombatTextKind::Physical,
                    });
                    run.meter
                        .record_hit(def.spell, "shadowstep_strike", crit_dmg, true);
                }
            }
        }
        5 => {
            // Priest: Shadow Word: Death (2.5x execute if < 35% HP)
            let mut closest: Option<Entity> = None;
            let mut min_d = 7.0;
            for (foe_e, foe_tf, foe) in &enemies {
                if foe.dead {
                    continue;
                }
                let d = reach(target_pos, foe_tf.translation);
                if d < min_d && reach(hpos, foe_tf.translation) <= 22.0 {
                    min_d = d;
                    closest = Some(foe_e);
                }
            }
            if let Some(target_e) = closest {
                if let Ok((_, target_tf, mut foe)) = enemies.get_mut(target_e) {
                    let is_execute = foe.hp < foe.max_hp * 0.35;
                    let mult = if is_execute { 2.5 } else { 1.2 };
                    let dmg = base_damage * mult;
                    let target_pos = target_tf.translation;
                    foe.hp -= dmg;
                    fx.impact(target_e, def.spell);
                    combat_text.write(CombatTextMsg {
                        pos: target_pos,
                        amount: dmg,
                        is_crit: is_execute,
                        kind: CombatTextKind::Shadow,
                    });
                    run.meter
                        .record_hit(def.spell, "shadow_word_death", dmg, is_execute);
                }
            }
        }
        7 => {
            // Shaman: Frost Shock with Frost mark & 50% slow
            let mut closest: Option<Entity> = None;
            let mut min_d = 7.0;
            for (foe_e, foe_tf, foe) in &enemies {
                if foe.dead {
                    continue;
                }
                let d = reach(target_pos, foe_tf.translation);
                if d < min_d && reach(hpos, foe_tf.translation) <= 20.0 {
                    min_d = d;
                    closest = Some(foe_e);
                }
            }
            if let Some(target_e) = closest {
                if let Ok((_, target_tf, mut foe)) = enemies.get_mut(target_e) {
                    let dmg = base_damage * 1.1;
                    let target_pos = target_tf.translation;
                    foe.hp -= dmg;
                    foe.slow = 0.50;
                    foe.slow_t = 4.0;
                    fx.impact(target_e, def.spell);
                    combat_text.write(CombatTextMsg {
                        pos: target_pos,
                        amount: dmg,
                        is_crit: false,
                        kind: CombatTextKind::Frost,
                    });
                    run.meter.record_hit(def.spell, "frost_shock", dmg, false);
                }
            }
        }
        8 => {
            // Mage: Fire Blast (AoE explosion splashing 4.0 yds)
            fx.impact(hero_e, 2136);
            for (foe_e, foe_tf, mut foe) in &mut enemies {
                if foe.dead {
                    continue;
                }
                if reach(target_pos, foe_tf.translation) <= 4.0 + foe.radius {
                    let dmg = base_damage * 1.25;
                    foe.hp -= dmg;
                    fx.impact(foe_e, 2136);
                    combat_text.write(CombatTextMsg {
                        pos: foe_tf.translation,
                        amount: dmg,
                        is_crit: true,
                        kind: CombatTextKind::Fire,
                    });
                    run.meter.record_hit(def.spell, "fire_blast", dmg, true);
                }
            }
        }
        9 => {
            // Warlock: Shadow Bolt with corruption
            let mut closest: Option<Entity> = None;
            let mut min_d = 8.0;
            for (foe_e, foe_tf, foe) in &enemies {
                if foe.dead {
                    continue;
                }
                let d = reach(target_pos, foe_tf.translation);
                if d < min_d && reach(hpos, foe_tf.translation) <= 22.0 {
                    min_d = d;
                    closest = Some(foe_e);
                }
            }
            if let Some(target_e) = closest {
                if let Ok((_, target_tf, mut foe)) = enemies.get_mut(target_e) {
                    let dmg = base_damage * 1.35;
                    let target_pos = target_tf.translation;
                    foe.hp -= dmg;
                    fx.impact(target_e, def.spell);
                    combat_text.write(CombatTextMsg {
                        pos: target_pos,
                        amount: dmg,
                        is_crit: false,
                        kind: CombatTextKind::Shadow,
                    });
                    run.meter.record_hit(def.spell, "shadow_bolt", dmg, false);
                }
            }
        }
        _ => {
            // Druid & Default: Moonfire
            let mut closest: Option<Entity> = None;
            let mut min_d = 8.0;
            for (foe_e, foe_tf, foe) in &enemies {
                if foe.dead {
                    continue;
                }
                let d = reach(target_pos, foe_tf.translation);
                if d < min_d && reach(hpos, foe_tf.translation) <= 22.0 {
                    min_d = d;
                    closest = Some(foe_e);
                }
            }
            if let Some(target_e) = closest {
                if let Ok((_, target_tf, mut foe)) = enemies.get_mut(target_e) {
                    let dmg = base_damage * 1.15;
                    let target_pos = target_tf.translation;
                    foe.hp -= dmg;
                    fx.impact(target_e, def.spell);
                    combat_text.write(CombatTextMsg {
                        pos: target_pos,
                        amount: dmg,
                        is_crit: true,
                        kind: CombatTextKind::Nature,
                    });
                    run.meter.record_hit(def.spell, "moonfire", dmg, true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_class_skillshots_defined() {
        for class_id in [1, 2, 3, 4, 5, 7, 8, 9, 11] {
            let def = class_skillshot(class_id);
            assert!(!def.name.is_empty());
            assert!(def.cooldown > 0.0);
            assert!(def.spell > 0);
        }
    }
}
