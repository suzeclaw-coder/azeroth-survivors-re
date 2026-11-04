//! Legendary Artifact Trinkets & Equipment System (3-Slot Relic Bar).
//!
//! Powerful WoW artifacts dropped from timed raid bosses and high-tier treasure chests:
//! - Talisman of Ephemeral Power: Surges every 30s for 8s (+80% Might and Haste).
//! - Insignia of the Grand Marshal: Triggers 3.0s divine invulnerability when HP < 25%.
//! - Shard of the Scale: 10% passive lifesteal on all hero damage.
//! - Zulian Tiger Claw: Dashing leaves exploding shadow trails (160 damage).
//! - Pristine Black Diamond: Collecting gold grants stacking +1% attack damage (up to +100%).
//! - Heart of Hakkar: Slaying foes releases homing blood bolts dealing 90 shadow damage.

use bevy::prelude::*;

use crate::sound::GlueSound;
use super::hero::Hero;
use super::units::Fx;
use super::{Rng, Run};

pub const MAX_TRINKETS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrinketKind {
    TalismanOfEphemeralPower,
    Insignia,
    ShardOfTheScale,
    ZulianTigerClaw,
    BlackDiamond,
    HeartOfHakkar,
}

#[derive(Clone, Copy, Debug)]
pub struct TrinketDef {
    pub kind: TrinketKind,
    pub name: &'static str,
    #[allow(dead_code)]
    pub desc: &'static str,
    pub spell: u32,
    pub cooldown: f32,
    pub duration: f32,
}

pub static TRINKETS: &[TrinketDef] = &[
    TrinketDef {
        kind: TrinketKind::TalismanOfEphemeralPower,
        name: "Talisman of Ephemeral Power",
        desc: "Surges every 30s for 8s, granting +80% Might and Haste.",
        spell: 23271,
        cooldown: 30.0,
        duration: 8.0,
    },
    TrinketDef {
        kind: TrinketKind::Insignia,
        name: "Insignia of the Grand Marshal",
        desc: "When dropping below 25% HP, grants 3s Divine Shield (90s CD).",
        spell: 23454,
        cooldown: 90.0,
        duration: 3.0,
    },
    TrinketDef {
        kind: TrinketKind::ShardOfTheScale,
        name: "Shard of the Scale",
        desc: "Holy radiance siphons 10% of damage dealt back into health.",
        spell: 21625,
        cooldown: 0.0,
        duration: 0.0,
    },
    TrinketDef {
        kind: TrinketKind::ZulianTigerClaw,
        name: "Zulian Tiger Claw",
        desc: "Dashing leaves exploding shadow trails dealing 160 damage.",
        spell: 24343,
        cooldown: 0.0,
        duration: 0.0,
    },
    TrinketDef {
        kind: TrinketKind::BlackDiamond,
        name: "Pristine Black Diamond",
        desc: "Gold pickups add +1% damage to your attacks (up to +100%).",
        spell: 20484,
        cooldown: 0.0,
        duration: 0.0,
    },
    TrinketDef {
        kind: TrinketKind::HeartOfHakkar,
        name: "Heart of Hakkar",
        desc: "Kills trigger homing Blood Bolts dealing 90 shadow damage.",
        spell: 24328,
        cooldown: 0.8,
        duration: 0.0,
    },
];

pub fn find_trinket(kind: TrinketKind) -> &'static TrinketDef {
    TRINKETS
        .iter()
        .find(|t| t.kind == kind)
        .expect("trinket def must exist")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrinketInstance {
    pub kind: TrinketKind,
    pub cd_timer: f32,
    pub active_timer: f32,
    pub stacks: u32,
}

impl TrinketInstance {
    pub fn new(kind: TrinketKind) -> Self {
        let def = find_trinket(kind);
        Self {
            kind,
            cd_timer: if def.cooldown > 0.0 { def.cooldown * 0.2 } else { 0.0 },
            active_timer: 0.0,
            stacks: 0,
        }
    }

    pub fn def(&self) -> &'static TrinketDef {
        find_trinket(self.kind)
    }
}

/// Awards a new trinket or upgrades gold if slots are full.
pub fn award_trinket(run: &mut Run, kind: TrinketKind, sounds: &mut MessageWriter<GlueSound>) {
    if run.trinkets.iter().any(|t| t.kind == kind) {
        // Already have it -> award 250 gold bonus
        run.gold += 250;
        sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
        run.banner = Some((format!("Trinket Duplicate: +250 Gold!"), 3.0));
        return;
    }

    if run.trinkets.len() < MAX_TRINKETS {
        run.trinkets.push(TrinketInstance::new(kind));
        sounds.write(GlueSound("igQuestListOpen"));
        run.banner = Some((format!("LEGENDARY TRINKET ACQUIRED: {}!", find_trinket(kind).name), 4.0));
    } else {
        run.gold += 350;
        sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
        run.banner = Some((format!("Relic Slots Full: +350 Gold!"), 3.0));
    }
}

/// Rolls a random unowned trinket.
pub fn roll_random_unowned(run: &Run, rng: &mut Rng) -> Option<TrinketKind> {
    let unowned: Vec<TrinketKind> = TRINKETS
        .iter()
        .map(|t| t.kind)
        .filter(|&k| !run.trinkets.iter().any(|t| t.kind == k))
        .collect();

    if unowned.is_empty() {
        None
    } else {
        let idx = (rng.f32() * unowned.len() as f32) as usize;
        Some(unowned[idx.min(unowned.len() - 1)])
    }
}

/// Ticks active trinket cooldowns and proc mechanics.
pub(super) fn tick_trinkets(
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut sounds: MessageWriter<GlueSound>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || run.dead_t.is_some() || run.trinkets.is_empty() {
        return;
    }

    let Ok((hero_e, _htf)) = hero.single() else { return };
    let hp_ratio = if run.stats().max_hp > 0.0 {
        run.hp / run.stats().max_hp
    } else {
        1.0
    };

    let mut announcements = Vec::new();
    let mut add_immune = 0.0f32;

    for t in &mut run.trinkets {
        let def = t.def();
        // Cooldown ticks down
        if t.cd_timer > 0.0 {
            t.cd_timer -= dt;
        }

        // Active duration ticks down
        if t.active_timer > 0.0 {
            t.active_timer -= dt;
        }

        match t.kind {
            TrinketKind::TalismanOfEphemeralPower => {
                if t.cd_timer <= 0.0 && t.active_timer <= 0.0 {
                    t.active_timer = def.duration;
                    t.cd_timer = def.cooldown;
                    announcements.push(("Talisman of Ephemeral Power surges with arcane might!", 2.5));
                    sounds.write(GlueSound("igCreatureAggroSelect"));
                    fx.impact(hero_e, 23271);
                }
            }
            TrinketKind::Insignia => {
                if t.cd_timer <= 0.0 && hp_ratio < 0.25 {
                    t.cd_timer = def.cooldown;
                    t.active_timer = def.duration;
                    add_immune = def.duration;
                    announcements.push(("Insignia of the Grand Marshal triggers Divine Shield!", 3.0));
                    sounds.write(GlueSound("gsCharacterSelectionEnterWorld"));
                    fx.impact(hero_e, 23454);
                }
            }
            _ => {}
        }
    }

    if add_immune > 0.0 {
        run.immune_t = run.immune_t.max(add_immune);
    }

    if let Some((msg, dur)) = announcements.into_iter().next() {
        run.banner = Some((msg.to_string(), dur));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_six_trinkets_defined() {
        assert_eq!(TRINKETS.len(), 6);
        for t in TRINKETS {
            assert!(!t.name.is_empty());
            assert!(!t.desc.is_empty());
            assert!(t.spell > 0);
        }
    }

    #[test]
    fn test_trinket_instance_creation() {
        let inst = TrinketInstance::new(TrinketKind::TalismanOfEphemeralPower);
        assert_eq!(inst.kind, TrinketKind::TalismanOfEphemeralPower);
        assert_eq!(inst.def().name, "Talisman of Ephemeral Power");
    }
}
