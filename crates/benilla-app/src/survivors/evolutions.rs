//! Spell Evolutions & Synergy System.
//!
//! When an active ability reaches Max Rank (Rank 5) and the hero owns at least 1 rank of its
//! paired passive, the draft system can offer an Evolution that upgrades the ability into an
//! ultimate form.

use bevy::prelude::*;

use super::data::{self, Effect};
use super::enemies::Enemy;
use super::hero::Hero;
use super::units::{self, flat, Fx};
use super::{Rng, Run};

/// Passive talent requirement for an evolution.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PassiveKind {
    Might,
    Haste,
    Vitality,
    Magnet,
    Agility,
    Greed,
}

impl PassiveKind {
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::Might => "Might",
            Self::Haste => "Haste",
            Self::Vitality => "Vitality",
            Self::Magnet => "Magnet",
            Self::Agility => "Agility",
            Self::Greed => "Greed",
        }
    }

    pub fn matches(self, def: &data::PassiveDef) -> bool {
        match self {
            Self::Might => def.stat == data::Stat::Might || def.key == "might",
            Self::Haste => def.stat == data::Stat::Haste || def.key == "intellect" || def.key == "haste",
            Self::Vitality => def.stat == data::Stat::MaxHp || def.key == "fortitude" || def.key == "vitality",
            Self::Magnet => def.stat == data::Stat::Magnet || def.key == "detect" || def.key == "magnet",
            Self::Agility => def.stat == data::Stat::Speed || def.key == "pack" || def.key == "agility",
            Self::Greed => def.stat == data::Stat::Growth || def.key == "wisdom" || def.key == "greed",
        }
    }
}

/// Definition of a spell evolution.
pub struct EvolutionDef {
    pub base_ability: &'static str,
    pub required_passive: PassiveKind,
    pub evolved_name: &'static str,
    pub description: &'static str,
    pub icon_spell: u32,
}

/// The 6 iconic evolutions.
pub static EVOLUTIONS: &[EvolutionDef] = &[
    EvolutionDef {
        base_ability: "fireball",
        required_passive: PassiveKind::Might,
        evolved_name: "Pyroblast",
        description: "Huge flaming meteor (scale 2.2) dealing 3.0x damage that detonates on impact and leaves a 6-yard burning crater ticking every 0.5s for 4.0s.",
        icon_spell: 11366,
    },
    EvolutionDef {
        base_ability: "chain_lightning",
        required_passive: PassiveKind::Haste,
        evolved_name: "Stormbringer",
        description: "Cooldown drops to 0.6s, chains to 10 targets, and releases static discharges.",
        icon_spell: 421,
    },
    EvolutionDef {
        base_ability: "whirlwind",
        required_passive: PassiveKind::Vitality,
        evolved_name: "Bladestorm",
        description: "Spins continuously, pulling nearby mobs within 8 yards inward and hitting all targets every 0.25s.",
        icon_spell: 46924,
    },
    EvolutionDef {
        base_ability: "blizzard",
        required_passive: PassiveKind::Magnet,
        evolved_name: "Glacial Vortex",
        description: "Freezing ice vortex that drags enemies inward into the storm center while dealing frost damage.",
        icon_spell: 10,
    },
    EvolutionDef {
        base_ability: "blink",
        required_passive: PassiveKind::Agility,
        evolved_name: "Glacial Blink",
        description: "Teleport detonates a Frost Nova (spell 122) at origin and arrival, freezing mobs for 3s.",
        icon_spell: 1953,
    },
    EvolutionDef {
        base_ability: "shadow_bolt",
        required_passive: PassiveKind::Greed,
        evolved_name: "Chaos Bolt",
        description: "Pierces through all enemies in a line, bursts on max range, enemies killed drop double gold.",
        icon_spell: 50796,
    },
];

/// Checks if an evolution is available to draft for the given base ability.
pub fn check_evolution_available(run: &Run, base_name: &str) -> Option<&'static EvolutionDef> {
    let evo = EVOLUTIONS.iter().find(|e| e.base_ability == base_name)?;
    if run.is_evolved(evo.evolved_name) {
        return None;
    }
    let has_rank5 = run.abilities.iter().any(|o| o.def.key == base_name && o.level >= 5)
        || (base_name == "blink" && (run.level >= 5 || run.abilities.iter().any(|o| o.def.key == "blink" && o.level >= 5)));
    if !has_rank5 {
        return None;
    }
    let has_passive = run
        .passives
        .iter()
        .any(|(p, level)| *level >= 1 && evo.required_passive.matches(p));
    if !has_passive {
        return None;
    }
    Some(evo)
}

/// Finds the evolution definition for a given base ability key.
pub fn evolution_for_base(base_name: &str) -> Option<&'static EvolutionDef> {
    EVOLUTIONS.iter().find(|e| e.base_ability == base_name)
}

/// Finds the evolution definition by its evolved name.
#[allow(dead_code)]
pub fn evolution_by_name(evolved_name: &str) -> Option<&'static EvolutionDef> {
    EVOLUTIONS.iter().find(|e| e.evolved_name == evolved_name)
}

/// Component attached to a Blizzard area when evolved into Glacial Vortex.
#[derive(Component)]
pub struct GlacialVortex {
    pub radius: f32,
}

/// Local state for evolution timers (e.g. continuous Bladestorm hits).
#[derive(Default)]
pub struct EvolutionState {
    pub bladestorm_tick: f32,
}

/// Execution system for active continuous evolutions (Bladestorm continuous pull/tick, Glacial Vortex inward drag).
pub(super) fn tick_evolutions(
    time: Res<Time>,
    run: Res<Run>,
    mut rng: ResMut<Rng>,
    mut state: Local<EvolutionState>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy), (Without<Hero>, Without<GlacialVortex>)>,
    vortices: Query<(Entity, &Transform, &GlacialVortex)>,
    mut fx: Fx,
    mut hits: MessageWriter<super::abilities::Hit>,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 {
        return;
    }
    let Ok((me, htf)) = hero.single() else { return };
    let hpos = htf.translation;
    let stats = run.stats();

    // 1. Glacial Vortex inward drag
    let vortex_centers: Vec<(Vec3, f32)> = vortices
        .iter()
        .map(|(_, tf, v)| (tf.translation, v.radius))
        .collect();

    if !vortex_centers.is_empty() {
        for (_, mut etf, en) in &mut enemies {
            if en.dead {
                continue;
            }
            for &(v_pos, radius) in &vortex_centers {
                let d = flat(v_pos - etf.translation);
                let dist = d.length();
                if dist <= radius && dist > 0.4 {
                    let drag = d.normalize() * (5.0 * dt);
                    etf.translation += drag;
                }
            }
        }
    }

    // 2. Bladestorm continuous spin & inward pull (hits every 0.25s)
    if run.is_evolved("Bladestorm") {
        state.bladestorm_tick -= dt;
        let should_hit = if state.bladestorm_tick <= 0.0 {
            state.bladestorm_tick = 0.25;
            true
        } else {
            false
        };

        if should_hit {
            fx.cast(me, 46924, &[me], None);
        }

        let dmg = 24.0 * 0.45 * stats.might;
        for (e, mut etf, en) in &mut enemies {
            if en.dead {
                continue;
            }
            let d = flat(hpos - etf.translation);
            let dist = d.length();
            if dist <= 8.0 {
                if dist > 0.8 {
                    etf.translation += d.normalize() * (4.0 * dt);
                }
                if should_hit {
                    let (dmg_val, crit) = if rng.f32() < stats.crit {
                        (dmg * 2.0, true)
                    } else {
                        (dmg, false)
                    };
                    hits.write(super::abilities::Hit {
                        spell: 46924,
                        key: "bladestorm",
                        target: e,
                        damage: dmg_val,
                        dot: 0.0,
                        dot_dur: 0.0,
                        effect: Effect::None,
                        mark: 0,
                        mark_dur: 0.0,
                        splash: 0.0,
                        crit,
                        color: units::COLOR_MELEE,
                    });
                }
            }
        }
    }
}

/// Detonates Frost Nova at origin and arrival positions for Glacial Blink.
pub(super) fn glacial_blink_detonate(
    caster: Entity,
    origin: Vec3,
    arrival: Vec3,
    run: &mut Run,
    enemies: &mut Query<(Entity, &Transform, &mut Enemy), Without<Hero>>,
    fx: &mut Fx,
) {
    fx.cast(caster, 122, &[], Some(origin));
    fx.cast(caster, 122, &[], Some(arrival));
    let stats = run.stats();
    let dmg = 35.0 * stats.might;

    for (e, etf, mut en) in enemies.iter_mut() {
        if en.dead {
            continue;
        }
        let d1 = flat(etf.translation - origin).length();
        let d2 = flat(etf.translation - arrival).length();
        if d1 <= 9.0 || d2 <= 9.0 {
            en.root_t = en.root_t.max(3.0);
            en.hp -= dmg;
            run.damage_done += f64::from(dmg);
            run.meter.record_hit(122, "glacial_blink", dmg, false);
            fx.number(e, dmg, false, units::COLOR_SPELL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Owned;

    #[test]
    fn test_six_iconic_evolutions_defined() {
        assert_eq!(EVOLUTIONS.len(), 6);

        let pyro = evolution_for_base("fireball").expect("Pyroblast evolution exists");
        assert_eq!(pyro.evolved_name, "Pyroblast");
        assert_eq!(pyro.required_passive, PassiveKind::Might);
        assert_eq!(pyro.icon_spell, 11366);

        let storm = evolution_for_base("chain_lightning").expect("Stormbringer evolution exists");
        assert_eq!(storm.evolved_name, "Stormbringer");
        assert_eq!(storm.required_passive, PassiveKind::Haste);
        assert_eq!(storm.icon_spell, 421);

        let blade = evolution_for_base("whirlwind").expect("Bladestorm evolution exists");
        assert_eq!(blade.evolved_name, "Bladestorm");
        assert_eq!(blade.required_passive, PassiveKind::Vitality);
        assert_eq!(blade.icon_spell, 46924);

        let vortex = evolution_for_base("blizzard").expect("Glacial Vortex evolution exists");
        assert_eq!(vortex.evolved_name, "Glacial Vortex");
        assert_eq!(vortex.required_passive, PassiveKind::Magnet);
        assert_eq!(vortex.icon_spell, 10);

        let blink = evolution_for_base("blink").expect("Glacial Blink evolution exists");
        assert_eq!(blink.evolved_name, "Glacial Blink");
        assert_eq!(blink.required_passive, PassiveKind::Agility);
        assert_eq!(blink.icon_spell, 1953);

        let chaos = evolution_for_base("shadow_bolt").expect("Chaos Bolt evolution exists");
        assert_eq!(chaos.evolved_name, "Chaos Bolt");
        assert_eq!(chaos.required_passive, PassiveKind::Greed);
        assert_eq!(chaos.icon_spell, 50796);
    }

    #[test]
    fn test_evolution_availability_logic() {
        let mut run = Run::default();
        let fireball_def = data::ability("fireball");
        let might_def = &data::PASSIVES[0]; // might

        // 1. Neither ability nor passive -> None
        assert!(check_evolution_available(&run, "fireball").is_none());

        // 2. Ability rank 4 + passive rank 1 -> None (needs rank 5)
        run.abilities.push(Owned {
            def: fireball_def,
            level: 4,
            cd: 0.0,
        });
        run.passives.push((might_def, 1));
        assert!(check_evolution_available(&run, "fireball").is_none());

        // 3. Ability rank 5 + passive rank 0 (or no passive) -> None
        run.passives.clear();
        run.abilities[0].level = 5;
        assert!(check_evolution_available(&run, "fireball").is_none());

        // 4. Ability rank 5 + passive rank 1 -> Available!
        run.passives.push((might_def, 1));
        let evo = check_evolution_available(&run, "fireball").expect("Should be available");
        assert_eq!(evo.evolved_name, "Pyroblast");

        // 5. Once evolved -> None
        run.evolved.push("Pyroblast");
        assert!(check_evolution_available(&run, "fireball").is_none());
    }
}
