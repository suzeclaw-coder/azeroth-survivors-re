//! Auto-cast: every owned ability fires on its own cooldown at the targets its kind picks, with
//! the spell's own visuals; damage lands when its missile would.

use bevy::prelude::*;

use super::data::{AbilityDef, Effect, Kind};
use super::enemies::{self, Enemy, Loot};
use super::fct::{CombatTextKind, CombatTextMsg};
use super::hero::{self, Hero};
use super::traits;
use super::units::{self, flat, Fx, Marks, UnitSpec, COLOR_HEAL, COLOR_MELEE, COLOR_SPELL};
use super::{Rng, Run, RunEntity};
use crate::net::{FieldChanged, Guid, ObjectStore};

/// One blow landing on one enemy.
#[derive(Message, Clone)]
pub(super) struct Hit {
    pub spell: u32,
    pub key: &'static str,
    pub target: Entity,
    pub damage: f32,
    pub dot: f32,
    pub dot_dur: f32,
    pub effect: Effect,
    pub mark: u32,
    pub mark_dur: f32,
    /// Also hits everything this close to the target, for less.
    pub splash: f32,
    pub crit: bool,
    pub color: u32,
}

impl Hit {
    pub fn direct(target: Entity, damage: f32, crit: bool) -> Self {
        Self {
            spell: 6603,
            key: "attack",
            target,
            damage,
            dot: 0.0,
            dot_dur: 0.0,
            effect: Effect::None,
            mark: 0,
            mark_dur: 0.0,
            splash: 0.0,
            crit,
            color: COLOR_MELEE,
        }
    }
}

/// Hits in flight, by the run time they land.
#[derive(Resource, Default)]
pub(super) struct PendingHits(pub Vec<(f32, Hit)>);

/// A ground spell's area, ticking on whatever stands in it.
#[derive(Component)]
pub(super) struct GroundArea {
    spell: u32,
    key: &'static str,
    damage: f32,
    dot: f32,
    dot_dur: f32,
    radius: f32,
    left: f32,
    every: f32,
    next_tick: f32,
    effect: Effect,
    mark: u32,
    /// The spell whose impact plays on each enemy a tick hits, 0 for none.
    impact: u32,
}

#[derive(Component)]
pub(super) struct Totem {
    cast: u32,
    key: &'static str,
    damage: f32,
    range: f32,
    radius: f32,
    left: f32,
    tick: f32,
    pulse: bool,
}

#[derive(Clone, Copy)]
struct Foe {
    e: Entity,
    pos: Vec3,
    hp: f32,
    dist: f32,
    radius: f32,
}

/// The numbers one cast uses, from the ability's level and the hero's stats.
struct Params {
    dmg: f32,
    dot: f32,
    count: usize,
    radius: f32,
    duration: f32,
    cooldown: f32,
}

fn mark_secs(def: &AbilityDef, duration: f32) -> f32 {
    match def.effect {
        Effect::Slow(_, t) | Effect::Weaken(_, t) => t,
        Effect::Root(t) | Effect::Stun(t) | Effect::Fear(t) => t,
        _ if duration > 0.0 => duration,
        _ => 3.0,
    }
}

fn hit_for(def: &AbilityDef, p: &Params, target: Entity, damage: f32, crit: bool) -> Hit {
    Hit {
        spell: def.spell,
        key: def.key,
        target,
        damage,
        dot: p.dot,
        dot_dur: p.duration,
        effect: def.effect,
        mark: def.mark,
        mark_dur: mark_secs(def, p.duration),
        splash: if matches!(def.kind, Kind::Bolt | Kind::Strike) {
            p.radius
        } else {
            0.0
        },
        crit,
        color: if def.kind == Kind::Melee {
            COLOR_MELEE
        } else {
            COLOR_SPELL
        },
    }
}

fn schedule(
    pending: &mut PendingHits,
    hits: &mut MessageWriter<Hit>,
    now: f32,
    delay: f32,
    hit: Hit,
) {
    if delay <= 0.0 {
        hits.write(hit);
    } else {
        pending.0.push((now + delay, hit));
    }
}

fn roll(rng: &mut Rng, crit: f32, dmg: f32) -> (f32, bool) {
    if rng.f32() < crit {
        (dmg * 2.0, true)
    } else {
        (dmg, false)
    }
}

fn nearest(foes: &[Foe], range: f32, n: usize) -> Vec<Foe> {
    foes.iter()
        .filter(|f| f.dist <= range + f.radius)
        .take(n)
        .copied()
        .collect()
}

fn toughest(foes: &[Foe], range: f32, n: usize) -> Vec<Foe> {
    let mut v: Vec<Foe> = foes
        .iter()
        .filter(|f| f.dist <= range + f.radius)
        .copied()
        .collect();
    v.sort_by(|a, b| b.hp.total_cmp(&a.hp));
    v.truncate(n);
    v
}

fn around(foes: &[Foe], center: Vec3, r: f32) -> Vec<Foe> {
    foes.iter()
        .filter(|f| flat(f.pos - center).length() <= r + f.radius * 0.5)
        .copied()
        .collect()
}

/// The point among the nearest enemies in range with the most others within `r` of it.
fn densest(foes: &[Foe], range: f32, r: f32) -> Option<Vec3> {
    let near: Vec<&Foe> = foes.iter().filter(|f| f.dist <= range).take(40).collect();
    near.iter()
        .map(|f| {
            (
                f.pos,
                near.iter()
                    .filter(|o| flat(o.pos - f.pos).length() <= r)
                    .count(),
            )
        })
        .max_by_key(|&(_, n)| n)
        .map(|(p, _)| p)
}

fn entities(foes: &[Foe]) -> Vec<Entity> {
    foes.iter().map(|f| f.e).collect()
}

pub(super) fn cast_abilities(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
    mut combat_text: MessageWriter<CombatTextMsg>,
) {
    let dt = time.delta_secs();
    if run.dead_t.is_some() || dt <= 0.0 {
        return;
    }
    let Ok((me, htf)) = hero.single() else { return };
    let hpos = htf.translation;
    let stats = run.stats();
    let now = run.elapsed;
    let mut foes: Vec<Foe> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, tf, en)| Foe {
            e,
            pos: tf.translation,
            hp: en.hp,
            dist: hero::reach(hpos, tf.translation),
            radius: en.radius,
        })
        .collect();
    foes.sort_by(|a, b| a.dist.total_cmp(&b.dist));
    let speed_of = |spell: u32| {
        spells
            .as_deref()
            .and_then(|s| s.catalog.get(spell))
            .map_or(0.0, |d| d.speed)
    };

    let cdr_mult = if run.is_ultimate_active(super::ultimates::UltimateKind::MageTimeWarp) {
        3.5
    } else {
        1.0
    };

    for i in 0..run.abilities.len() {
        run.abilities[i].cd -= dt * cdr_mult;
        if run.abilities[i].cd > 0.0 {
            continue;
        }
        let o = &run.abilities[i];
        let def = o.def;

        if def.key == "whirlwind" && run.is_evolved("Bladestorm") {
            run.abilities[i].cd = 1.0;
            continue;
        }

        let is_pyro = def.key == "fireball" && run.is_evolved("Pyroblast");
        let is_chaos = def.key == "shadow_bolt" && run.is_evolved("Chaos Bolt");
        let is_stormbringer = def.key == "chain_lightning" && run.is_evolved("Stormbringer");
        let is_glacial_blink = def.key == "blink" && run.is_evolved("Glacial Blink");

        if is_chaos {
            let Some(first) = nearest(&foes, def.range, 1).first().copied() else {
                run.abilities[i].cd = 0.2;
                continue;
            };
            let dir = flat(first.pos - hpos).normalize_or(Vec3::NEG_Z);
            let range = def.range;
            let burst_pos = hpos + dir * range;
            fx.cast(me, 50796, &[], Some(burst_pos));

            let mult = o.damage_mult() * stats.might;
            let base_dmg = def.damage * mult;

            for f in &foes {
                let v = flat(f.pos - hpos);
                let proj = v.dot(dir);
                if proj >= 0.0 && proj <= range {
                    let perp = (v - dir * proj).length();
                    if perp <= 2.2 + f.radius {
                        let (dmg, crit) = roll(&mut rng, stats.crit, base_dmg * 1.5);
                        let s = speed_of(50796);
                        let delay = if s > 0.0 { proj / s } else { 0.0 };
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            delay,
                            Hit {
                                spell: 50796,
                                key: "chaos_bolt",
                                target: f.e,
                                damage: dmg,
                                dot: 0.0,
                                dot_dur: 0.0,
                                effect: Effect::None,
                                mark: 0,
                                mark_dur: 0.0,
                                splash: 0.0,
                                crit,
                                color: COLOR_SPELL,
                            },
                        );
                    }
                }
            }

            for f in around(&foes, burst_pos, 5.0) {
                let (dmg, crit) = roll(&mut rng, stats.crit, base_dmg * 2.0);
                schedule(
                    &mut pending,
                    &mut hits,
                    now,
                    range / 25.0,
                    Hit {
                        spell: 50796,
                        key: "chaos_bolt_burst",
                        target: f.e,
                        damage: dmg,
                        dot: 0.0,
                        dot_dur: 0.0,
                        effect: Effect::None,
                        mark: 0,
                        mark_dur: 0.0,
                        splash: 0.0,
                        crit,
                        color: COLOR_SPELL,
                    },
                );
            }
            run.abilities[i].cd = o.cooldown(&stats);
            continue;
        }

        if is_glacial_blink {
            let facing = flat(foes.first().map_or(Vec3::NEG_Z, |f| f.pos - hpos)).normalize_or(Vec3::NEG_Z);
            let to = hpos + facing * 12.0;
            fx.cast(me, 122, &[], Some(hpos));
            fx.cast(me, 122, &[], Some(to));
            let mult = o.damage_mult() * stats.might;
            let dmg = def.damage * mult * 1.5;
            for f in around(&foes, hpos, 9.0) {
                let (dmg_val, crit) = roll(&mut rng, stats.crit, dmg);
                hits.write(Hit {
                    spell: 122,
                    key: "glacial_blink",
                    target: f.e,
                    damage: dmg_val,
                    dot: 0.0,
                    dot_dur: 0.0,
                    effect: Effect::Root(3.0),
                    mark: 122,
                    mark_dur: 3.0,
                    splash: 0.0,
                    crit,
                    color: COLOR_SPELL,
                });
            }
            for f in around(&foes, to, 9.0) {
                let (dmg_val, crit) = roll(&mut rng, stats.crit, dmg);
                hits.write(Hit {
                    spell: 122,
                    key: "glacial_blink",
                    target: f.e,
                    damage: dmg_val,
                    dot: 0.0,
                    dot_dur: 0.0,
                    effect: Effect::Root(3.0),
                    mark: 122,
                    mark_dur: 3.0,
                    splash: 0.0,
                    crit,
                    color: COLOR_SPELL,
                });
            }
            run.abilities[i].cd = o.cooldown(&stats);
            continue;
        }

        let mult = o.damage_mult() * stats.might;
        let p = Params {
            dmg: def.damage * mult,
            dot: def.dot * mult,
            count: o.count() as usize,
            radius: o.radius(&stats),
            duration: o.duration(),
            cooldown: o.cooldown(&stats),
        };
        let speed = speed_of(def.spell);
        let fired = match def.kind {
            Kind::Bolt | Kind::Strike | Kind::Melee => {
                let reach = if def.kind == Kind::Melee {
                    def.range + 0.5
                } else {
                    def.range
                };
                let targets = if is_pyro || matches!(
                    def.key,
                    "pyroblast" | "aimed_shot" | "exorcism" | "starfire"
                ) {
                    toughest(&foes, reach, p.count)
                } else {
                    nearest(&foes, reach, p.count)
                };
                if targets.is_empty() {
                    false
                } else {
                    let cast_spell = if is_pyro { 11366 } else { def.spell };
                    if def.kind == Kind::Melee {
                        fx.swing(me, targets[0].e, p.dmg);
                        for f in &targets {
                            fx.impact(f.e, cast_spell);
                        }
                    } else {
                        fx.cast(me, cast_spell, &entities(&targets), None);
                    }
                    let hit_dmg = if is_pyro { p.dmg * 3.0 } else { p.dmg };
                    for f in &targets {
                        let (dmg, crit) = roll(&mut rng, stats.crit, hit_dmg);
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        let mut hit = hit_for(def, &p, f.e, dmg, crit);
                        if is_pyro {
                            hit.spell = 11366;
                            hit.key = "pyroblast";
                            hit.damage = dmg;
                            hit.splash = 6.0;
                        }
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            delay,
                            hit,
                        );
                    }
                    true
                }
            }
            Kind::Nova | Kind::Cone => {
                let mut targets = around(&foes, hpos, p.radius);
                if def.kind == Kind::Cone {
                    if let Some(first) = targets.first().copied() {
                        let aim = flat(first.pos - hpos).normalize_or_zero();
                        targets.retain(|f| {
                            let d = flat(f.pos - hpos);
                            d.length() < 2.0 || d.normalize_or_zero().dot(aim) >= 0.8
                        });
                    }
                }
                if targets.is_empty() {
                    false
                } else {
                    fx.cast(me, def.spell, &entities(&targets), None);
                    for f in &targets {
                        let (dmg, crit) = roll(&mut rng, stats.crit, p.dmg);
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            delay,
                            hit_for(def, &p, f.e, dmg, crit),
                        );
                    }
                    true
                }
            }
            Kind::Ground | Kind::GroundSelf => {
                let center = if def.kind == Kind::GroundSelf {
                    foes.first()
                        .filter(|f| f.dist <= p.radius + 4.0)
                        .map(|_| hpos)
                } else {
                    densest(&foes, def.range, p.radius)
                };
                match center {
                    None => false,
                    Some(center) => {
                        fx.cast(me, def.spell, &[], Some(center));
                        let area = if def.overhead {
                            commands
                                .spawn((Transform::from_translation(center), RunEntity))
                                .id()
                        } else {
                            let serial = run.guid();
                            units::spawn_area_object(
                                &mut commands,
                                serial,
                                def.spell,
                                p.radius,
                                center,
                            )
                        };
                        let burst = def.duration <= 0.0;
                        let mut area_cmd = commands.entity(area);
                        area_cmd.insert(GroundArea {
                            spell: def.spell,
                            key: def.key,
                            damage: p.dmg,
                            dot: p.dot,
                            dot_dur: if burst { 5.0 } else { 0.0 },
                            radius: p.radius,
                            left: if burst { 1.5 } else { p.duration },
                            every: if burst { f32::INFINITY } else { 1.0 },
                            next_tick: if burst { 0.0 } else { 0.4 },
                            effect: def.effect,
                            mark: def.mark,
                            impact: if def.overhead { def.spell } else { 0 },
                        });
                        if def.key == "blizzard" && run.is_evolved("Glacial Vortex") {
                            area_cmd.insert(super::evolutions::GlacialVortex {
                                radius: p.radius,
                            });
                        }
                        true
                    }
                }
            }
            Kind::Chain => match nearest(&foes, def.range, 1).first().copied() {
                None => false,
                Some(first) => {
                    let max_chain = if is_stormbringer { 10 } else { p.count + 1 };
                    let mut chain = vec![first];
                    while chain.len() < max_chain {
                        let last = chain[chain.len() - 1].pos;
                        let next = foes
                            .iter()
                            .filter(|f| !chain.iter().any(|c| c.e == f.e))
                            .filter(|f| flat(f.pos - last).length() <= p.radius)
                            .min_by(|a, b| {
                                flat(a.pos - last)
                                    .length()
                                    .total_cmp(&flat(b.pos - last).length())
                            });
                        match next {
                            Some(f) => chain.push(*f),
                            None => break,
                        }
                    }
                    fx.cast(me, def.spell, &entities(&chain), None);
                    for (k, f) in chain.iter().enumerate() {
                        let (dmg, crit) =
                            roll(&mut rng, stats.crit, p.dmg * 0.85f32.powi(k as i32));
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            0.08 * k as f32,
                            hit_for(def, &p, f.e, dmg, crit),
                        );
                    }
                    if is_stormbringer {
                        for f in &chain {
                            let static_targets: Vec<Foe> = foes
                                .iter()
                                .filter(|o| !chain.iter().any(|c| c.e == o.e) && flat(o.pos - f.pos).length() <= 6.0)
                                .take(2)
                                .copied()
                                .collect();
                            for s in static_targets {
                                fx.impact(s.e, 403);
                                let (dmg, crit) = roll(&mut rng, stats.crit, p.dmg * 0.4);
                                schedule(
                                    &mut pending,
                                    &mut hits,
                                    now,
                                    0.1,
                                    Hit {
                                        spell: 421,
                                        key: "stormbringer_static",
                                        target: s.e,
                                        damage: dmg,
                                        dot: 0.0,
                                        dot_dur: 0.0,
                                        effect: Effect::None,
                                        mark: 0,
                                        mark_dur: 0.0,
                                        splash: 0.0,
                                        crit,
                                        color: COLOR_SPELL,
                                    },
                                );
                            }
                        }
                    }
                    true
                }
            },
            Kind::Dot => {
                let mut pool: Vec<Foe> = foes
                    .iter()
                    .filter(|f| f.dist <= def.range)
                    .take(p.count * 3)
                    .copied()
                    .collect();
                if pool.is_empty() {
                    false
                } else {
                    let mut targets = Vec::with_capacity(p.count);
                    while targets.len() < p.count && !pool.is_empty() {
                        targets.push(pool.swap_remove(rng.index(pool.len())));
                    }
                    fx.cast(me, def.spell, &entities(&targets), None);
                    for f in &targets {
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            &mut pending,
                            &mut hits,
                            now,
                            delay,
                            hit_for(def, &p, f.e, 0.0, false),
                        );
                    }
                    true
                }
            }
            Kind::Aura => {
                let targets = around(&foes, hpos, p.radius);
                if targets.is_empty() {
                    false
                } else {
                    for f in targets.iter().take(6) {
                        fx.impact(f.e, def.spell);
                    }
                    for f in &targets {
                        let (dmg, crit) = roll(&mut rng, stats.crit, p.dmg);
                        hits.write(hit_for(def, &p, f.e, dmg, crit));
                    }
                    true
                }
            }
            Kind::Orbit => {
                let targets = nearest(&foes, p.radius, p.count);
                if targets.is_empty() {
                    false
                } else {
                    for f in &targets {
                        fx.impact(f.e, 403);
                        let (dmg, crit) = roll(&mut rng, stats.crit, p.dmg);
                        hits.write(hit_for(def, &p, f.e, dmg, crit));
                    }
                    true
                }
            }
            Kind::Shield => {
                run.shield = p.dmg;
                run.shield_t = p.duration;
                fx.cast(me, def.spell, &[me], None);
                true
            }
            Kind::Immune => {
                if foes.first().is_some_and(|f| f.dist <= 5.0) {
                    run.immune_t = p.duration;
                    fx.cast(me, def.spell, &[me], None);
                    true
                } else {
                    false
                }
            }
            Kind::Thorns => {
                if foes.first().is_some_and(|f| f.dist <= 8.0) {
                    run.thorns = p.dmg;
                    run.thorns_t = p.duration;
                    run.thorns_spell = def.spell;
                    fx.cast(me, def.spell, &[me], None);
                    true
                } else {
                    false
                }
            }
            Kind::Heal => {
                if run.hp < stats.max_hp * 0.85 {
                    run.heal_rate = p.dmg / p.duration.max(1.0);
                    run.heal_t = p.duration;
                    fx.cast(me, def.spell, &[me], None);
                    fx.number(me, p.dmg, false, COLOR_HEAL);
                    combat_text.write(CombatTextMsg {
                        pos: hpos,
                        amount: p.dmg,
                        is_crit: false,
                        kind: CombatTextKind::Heal,
                    });
                    true
                } else {
                    false
                }
            }
            Kind::Totem => {
                if foes
                    .first()
                    .is_some_and(|f| f.dist <= def.range.max(p.radius) + 6.0)
                {
                    fx.cast(me, def.spell, &[], None);
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let pos = hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 2.0;
                    let serial = run.guid();
                    let totem = units::spawn_unit(
                        &mut commands,
                        &UnitSpec {
                            serial,
                            display: def.totem.0,
                            scale: 1.0,
                            level: 60,
                            faction: units::FACTION_FRIENDLY,
                            bytes0: 0,
                            weapons: &[],
                            sheath: 0,
                        },
                        pos,
                        0.0,
                    );
                    commands.entity(totem).insert(Totem {
                        cast: def.totem.1,
                        key: def.key,
                        damage: p.dmg,
                        range: def.range,
                        radius: p.radius,
                        left: p.duration,
                        tick: 0.3,
                        pulse: def.key == "magma_totem",
                    });
                    true
                } else {
                    false
                }
            }
        };
        let actual_cd = if is_stormbringer { 0.6 } else { p.cooldown };
        run.abilities[i].cd = if fired { actual_cd } else { 0.2 };
    }
}

/// Ground areas tick, and totems fight, until they run out.
#[allow(clippy::type_complexity)]
pub(super) fn tick_areas(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut areas: Query<(Entity, &Transform, &mut GroundArea)>,
    mut totems: Query<(Entity, &Transform, &mut Totem), Without<GroundArea>>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<GroundArea>, Without<Totem>)>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let stats = run.stats();
    let now = run.elapsed;
    let live: Vec<Foe> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, tf, en)| Foe {
            e,
            pos: tf.translation,
            hp: en.hp,
            dist: 0.0,
            radius: en.radius,
        })
        .collect();

    for (e, tf, mut area) in &mut areas {
        area.left -= dt;
        area.next_tick -= dt;
        if area.next_tick <= 0.0 {
            area.next_tick += area.every;
            for (i, f) in around(&live, tf.translation, area.radius)
                .into_iter()
                .enumerate()
            {
                if area.impact != 0 && i < 12 {
                    fx.impact(f.e, area.impact);
                }
                let (dmg, crit) = roll(&mut rng, stats.crit, area.damage);
                hits.write(Hit {
                    spell: area.spell,
                    key: area.key,
                    target: f.e,
                    damage: dmg,
                    dot: area.dot,
                    dot_dur: area.dot_dur,
                    effect: area.effect,
                    mark: area.mark,
                    mark_dur: 1.5,
                    splash: 0.0,
                    crit,
                    color: COLOR_SPELL,
                });
            }
        }
        if area.left <= 0.0 {
            commands.entity(e).despawn();
        }
    }

    for (e, tf, mut totem) in &mut totems {
        totem.left -= dt;
        if totem.left <= 0.0 {
            commands
                .entity(e)
                .remove::<Totem>()
                .insert(benilla_world::model_fade::DespawnFade::default());
            continue;
        }
        totem.tick -= dt;
        if totem.tick > 0.0 {
            continue;
        }
        let pos = tf.translation;
        if totem.pulse {
            let targets = around(&live, pos, totem.radius);
            totem.tick = 2.0;
            if targets.is_empty() {
                continue;
            }
            fx.cast(e, totem.cast, &[], None);
            for f in &targets {
                let (dmg, crit) = roll(&mut rng, stats.crit, totem.damage);
                hits.write(Hit {
                    spell: totem.cast,
                    key: totem.key,
                    color: COLOR_SPELL,
                    ..Hit::direct(f.e, dmg, crit)
                });
            }
        } else {
            totem.tick = 1.0;
            let target = live
                .iter()
                .map(|f| (flat(f.pos - pos).length(), f))
                .filter(|(d, _)| *d <= totem.range)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((dist, f)) = target else { continue };
            fx.cast(e, totem.cast, &[f.e], None);
            let speed = spells
                .as_deref()
                .and_then(|s| s.catalog.get(totem.cast))
                .map_or(0.0, |d| d.speed);
            let delay = if speed > 0.0 { dist / speed } else { 0.0 };
            let (dmg, crit) = roll(&mut rng, stats.crit, totem.damage);
            schedule(
                &mut pending,
                &mut hits,
                now,
                delay,
                Hit {
                    spell: totem.cast,
                    key: totem.key,
                    color: COLOR_SPELL,
                    ..Hit::direct(f.e, dmg, crit)
                },
            );
        }
    }
}

/// Land every hit due: damage and its number, the over-time part, crowd control, the marks, and
/// death with its loot.
pub(super) fn apply_hits(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut pending: ResMut<PendingHits>,
    mut incoming: MessageReader<Hit>,
    hero: Query<&Transform, With<Hero>>,
    mut enemies: Query<(
        Entity,
        &Guid,
        &Transform,
        &mut Enemy,
        &mut ObjectStore,
        &mut Marks,
    )>,
    mut fx: Fx,
    mut loot: MessageWriter<Loot>,
    mut fields: MessageWriter<FieldChanged>,
    mut combat_text: MessageWriter<CombatTextMsg>,
    mut sounds: MessageWriter<crate::sound::GlueSound>,
    mut rng: ResMut<Rng>,
) {
    let dt = time.delta_secs();
    run.enchanter.crusader_timer = (run.enchanter.crusader_timer - dt).max(0.0);
    let now = run.elapsed;
    let mut due: Vec<Hit> = incoming.read().cloned().collect();
    pending.0.retain(|(t, h)| {
        if *t <= now {
            due.push(h.clone());
            false
        } else {
            true
        }
    });

    // Splash reaches the target's neighbours at seventy percent.
    let mut all = Vec::with_capacity(due.len());
    for h in due {
        if h.splash > 0.0 {
            if let Ok((_, _, center, ..)) = enemies.get(h.target) {
                let c = center.translation;
                for (e, _, tf, en, ..) in enemies.iter() {
                    if e != h.target && !en.dead && flat(tf.translation - c).length() <= h.splash {
                        all.push(Hit {
                            target: e,
                            damage: h.damage * 0.7,
                            splash: 0.0,
                            mark: 0,
                            ..h.clone()
                        });
                    }
                }
            }
        }
        all.push(Hit { splash: 0.0, ..h });
    }

    let mut bursts: Vec<(Entity, Vec3, u32, f32)> = Vec::new();

    let mut kill =
        |commands: &mut Commands,
         run: &mut Run,
         sounds: &mut MessageWriter<crate::sound::GlueSound>,
         rng: &mut ResMut<Rng>,
         bursts: &mut Vec<(Entity, Vec3, u32, f32)>,
         e: Entity,
         pos: Vec3,
         en: &mut Enemy,
         spell: u32,
         key: &'static str| {
            // Ailment death burst: compute unspent DoT damage before clearing
            let mut bleed_dmg = 0.0;
            let mut burn_dmg = 0.0;
            let mut poison_dmg = 0.0;
            for d in &en.dots {
                let mut rem = d.dps * d.left;
                if d.key == "rupture" && run.has_glyph(super::glyphs::GlyphKind::RuptureBurst) {
                    rem *= 1.5;
                }
                match d.ailment {
                    enemies::AilmentKind::Bleed => bleed_dmg += rem,
                    enemies::AilmentKind::Burn => burn_dmg += rem,
                    enemies::AilmentKind::Poison => poison_dmg += rem,
                    enemies::AilmentKind::Generic => {}
                }
            }
            let burst_dmg = bleed_dmg + burn_dmg + poison_dmg;
            if burst_dmg > 0.0 {
                let dominant_spell = if burn_dmg >= bleed_dmg && burn_dmg >= poison_dmg && burn_dmg > 0.0 {
                    20481 // Fire impact
                } else if bleed_dmg >= poison_dmg && bleed_dmg > 0.0 {
                    11579 // Blood splatter
                } else {
                    8092 // Nature burst
                };
                bursts.push((e, pos, dominant_spell, burst_dmg));
            }

            en.dead = true;
            en.hp = 0.0;
            en.dots.clear();
            enemies::mark_dying(commands, e);
            loot.write(Loot {
                pos,
                xp: en.xp,
                boss: en.boss,
                elite: en.def.big || en.xp >= 10 || en.champion.is_some(),
                double_gold: spell == 50796 || key == "chaos_bolt" || key == "chaos_bolt_burst",
            });
            if let Some(champ) = en.champion {
                enemies::spawn_gem(commands, run.guid(), pos + Vec3::X * 0.6, enemies::GemKind::Gold(75));
                run.banner = Some((format!("{} Champion Slain! +75 Gold!", champ.name()), 3.0));
            }
            run.kills += 1;
            run.meter.record_kill(spell, key);
            super::combo::record_kill(run, sounds);

            if en.boss {
                run.banner = Some((format!("{} is slain!", en.def.name), 3.0));
                if run.trinkets.len() < super::trinkets::MAX_TRINKETS {
                    if let Some(trinket) = super::trinkets::roll_random_unowned(run, rng) {
                        super::trinkets::award_trinket(run, trinket, sounds);
                    }
                }
                super::vault::roll_boss_gear_drop(
                    &mut run.vault,
                    run.level,
                    &mut run.banner,
                    rng,
                    true,
                );
            }
        };

    for h in all {
        let Ok((e, guid, tf, mut en, mut store, mut marks)) = enemies.get_mut(h.target) else {
            continue;
        };
        if en.dead {
            continue;
        }
        let mut dmg = h.damage;
        if h.key == "eviscerate" {
            let bleed_ticks: f32 = en.dots.iter()
                .filter(|d| matches!(d.ailment, enemies::AilmentKind::Bleed))
                .map(|d| d.left)
                .sum();
            dmg *= 1.0 + (0.05 * bleed_ticks).min(1.0);
        } else if h.key == "hurricane" {
            if en.dots.iter().any(|d| d.key == "moonfire" || d.key == "insect_swarm") {
                dmg *= 1.08;
            }
        } else if h.key == "shadow_bolt" {
            let dot_rem: f32 = en.dots.iter()
                .filter(|d| d.key == "corruption" || d.key == "curse_of_agony")
                .map(|d| d.dps * d.left)
                .fold(0.0, f32::max);
            if dot_rem > 0.0 {
                dmg *= 1.0 + (dot_rem * 0.01).min(0.5);
            }
        }
        if let Effect::Execute(threshold) = h.effect {
            if !en.boss && en.hp / en.max_hp <= threshold {
                dmg = dmg.max(en.hp);
            }
        }
        if en.champion_shield > 0.0 {
            let absorbed = dmg.min(en.champion_shield);
            en.champion_shield -= absorbed;
            dmg -= absorbed;
            fx.number(e, absorbed, false, 0xFFFF_D700);
        }
        if dmg > 0.0 {
            en.hp -= dmg;
            run.damage_done += f64::from(dmg);
            run.meter.record_hit(h.spell, h.key, dmg, h.crit);
            fx.number(e, dmg, h.crit, h.color);
            combat_text.write(CombatTextMsg {
                pos: tf.translation,
                amount: dmg,
                is_crit: h.crit,
                kind: CombatTextKind::from_key(h.key),
            });
            if (h.spell == 11366 || h.key == "pyroblast") && h.target == e {
                let serial = run.guid();
                let crater = units::spawn_area_object(&mut commands, serial, 11366, 6.0, tf.translation);
                commands.entity(crater).insert(GroundArea {
                    spell: 11366,
                    key: "burning_crater",
                    damage: 12.0 * run.stats().might,
                    dot: 0.0,
                    dot_dur: 0.0,
                    radius: 6.0,
                    left: 4.0,
                    every: 0.5,
                    next_tick: 0.5,
                    effect: Effect::None,
                    mark: 0,
                    impact: 11366,
                });
            }
            let (proc_fire, crusader_proc, leech_frac) = run.enchanter.roll_on_hit_procs(&mut rng);
            if proc_fire > 0.0 {
                en.hp -= proc_fire;
                run.damage_done += f64::from(proc_fire);
                fx.number(e, proc_fire, false, 0xFFFF_4500);
                fx.impact(e, 20481);
            }
            if crusader_proc {
                let max = run.stats().max_hp;
                let crusader_heal = max * 0.12;
                run.hp = (run.hp + crusader_heal).min(max);
                if let Ok(htf) = hero.single() {
                    combat_text.write(CombatTextMsg {
                        pos: htf.translation,
                        amount: crusader_heal,
                        is_crit: true,
                        kind: CombatTextKind::Heal,
                    });
                }
            }
            let mut drain_frac = match h.effect {
                Effect::Drain(frac) => frac,
                _ if run.race == 5 && traits::is_bleeding_or_burning(&en) => 0.10,
                _ => 0.0,
            };
            drain_frac += leech_frac;
            if run.trinkets.iter().any(|t| t.kind == super::trinkets::TrinketKind::ShardOfTheScale) {
                drain_frac += 0.10;
            }
            if drain_frac > 0.0 {
                let max = run.stats().max_hp;
                let heal = (dmg * drain_frac).min(max - run.hp);
                run.hp = (run.hp + heal).min(max);
                if heal > 0.0 {
                    if let Ok(htf) = hero.single() {
                        combat_text.write(CombatTextMsg {
                            pos: htf.translation,
                            amount: heal,
                            is_crit: false,
                            kind: CombatTextKind::Heal,
                        });
                    }
                }
            }
        }
        if h.dot > 0.0 && h.dot_dur > 0.0 {
            if en.dots.len() >= 6 {
                en.dots.remove(0);
            }
            let ailment = enemies::AilmentKind::from_key(h.key);
            en.dots.push(enemies::Dot {
                spell: h.spell,
                key: h.key,
                dps: h.dot,
                left: h.dot_dur,
                tick: 1.0,
                ailment,
            });
        }
        let boss = en.boss;
        match h.effect {
            Effect::Slow(m, t) => {
                let m = if boss { m.max(0.75) } else { m };
                en.slow = if en.slow_t > 0.0 { en.slow.min(m) } else { m };
                en.slow_t = en.slow_t.max(t);
            }
            Effect::Root(t) => en.root_t = en.root_t.max(if boss { t * 0.3 } else { t }),
            Effect::Stun(t) => en.stun_t = en.stun_t.max(if boss { t * 0.3 } else { t }),
            Effect::Fear(t) if !boss => en.fear_t = en.fear_t.max(t),
            Effect::Weaken(m, t) => {
                en.weaken = m;
                en.weaken_t = t;
            }
            _ => {}
        }
        if h.mark != 0 {
            marks.add(h.mark, h.mark_dur);
        }
        if en.hp <= 0.0 {
            kill(&mut commands, &mut run, &mut sounds, &mut rng, &mut bursts, e, tf.translation, &mut en, h.spell, h.key);
            units::kill_unit(&mut store);
            marks.clear();
        }
        marks.flush(e, guid.0, &mut store, &mut fields);
    }

    // The over-time ticks, once a second each.
    for (e, guid, tf, mut en, mut store, mut marks) in &mut enemies {
        if en.dead || en.dots.is_empty() {
            continue;
        }
        let mut dealt = 0.0;
        let mut last_dot = (0u32, "");
        for d in &mut en.dots {
            d.left -= dt;
            d.tick -= dt;
            if d.tick <= 0.0 {
                d.tick += 1.0;
                dealt += d.dps;
                run.meter.record_hit(d.spell, d.key, d.dps, false);
                last_dot = (d.spell, d.key);
            }
        }
        en.dots.retain(|d| d.left > 0.0);
        if dealt > 0.0 {
            en.hp -= dealt;
            run.damage_done += f64::from(dealt);
            fx.number(e, dealt, false, COLOR_SPELL);
            combat_text.write(CombatTextMsg {
                pos: tf.translation,
                amount: dealt,
                is_crit: false,
                kind: CombatTextKind::from_key(last_dot.1),
            });
            if en.hp <= 0.0 {
                kill(&mut commands, &mut run, &mut sounds, &mut rng, &mut bursts, e, tf.translation, &mut en, last_dot.0, last_dot.1);
                units::kill_unit(&mut store);
                marks.clear();
                marks.flush(e, guid.0, &mut store, &mut fields);
            }
        }
    }

    // Dispatch ailment death bursts to nearby enemies within 8.0 yards (up to 4 targets)
    for (dead_e, burst_pos, burst_spell, total_burst_dmg) in bursts {
        let mut candidates: Vec<(Entity, f32)> = Vec::new();
        for (cand_e, _, cand_tf, cand_en, ..) in enemies.iter() {
            if cand_e != dead_e && !cand_en.dead {
                let dist = flat(cand_tf.translation - burst_pos).length();
                if dist <= 8.0 {
                    candidates.push((cand_e, dist));
                }
            }
        }
        if !candidates.is_empty() {
            candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
            let targets: Vec<Entity> = candidates.into_iter().take(4).map(|c| c.0).collect();
            let per_target_dmg = total_burst_dmg / (targets.len() as f32);
            for target_e in targets {
                fx.impact(target_e, burst_spell);
                pending.0.push((
                    now,
                    Hit {
                        target: target_e,
                        damage: per_target_dmg,
                        spell: burst_spell,
                        key: "ailment_burst",
                        color: match burst_spell {
                            20481 => 0xFFFF_661A, // Fire
                            11579 => 0xFFE6_1A1A, // Blood
                            _ => 0xFF33_E64D,     // Nature
                        },
                        ..Hit::direct(target_e, per_target_dmg, false)
                    },
                ));
            }
        }
    }
}
