//! The horde: spawning on a ring outside the view, the chase, contact damage, death, and the
//! experience gems the dead leave.

use std::collections::HashMap;

use benilla_world::rig_anim::GlobalSeqDrive;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::chests;
use super::data::{self, EnemyDef};
use super::hero::{self, Hero, HERO_RADIUS};
use super::telegraphs::{self, TelegraphCache};
use super::units::{self, flat, yaw_toward, Fx, Marks, UnitSpec, COLOR_HEAL, COLOR_HURT};
use super::{Phase, Rng, Run, Selection};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};
use crate::sound::GlueSound;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChampionModifier {
    Molten,
    Frozen,
    Vampiric,
    Avenger,
    Shielded,
}

impl ChampionModifier {
    pub const ALL: [ChampionModifier; 5] = [
        ChampionModifier::Molten,
        ChampionModifier::Frozen,
        ChampionModifier::Vampiric,
        ChampionModifier::Avenger,
        ChampionModifier::Shielded,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Molten => "Molten",
            Self::Frozen => "Frozen",
            Self::Vampiric => "Vampiric",
            Self::Avenger => "Avenger",
            Self::Shielded => "Shielded",
        }
    }

    #[allow(dead_code)]
    pub fn color(&self) -> Color {
        match self {
            Self::Molten => Color::srgb(1.0, 0.4, 0.1),
            Self::Frozen => Color::srgb(0.3, 0.75, 1.0),
            Self::Vampiric => Color::srgb(0.9, 0.15, 0.25),
            Self::Avenger => Color::srgb(1.0, 0.8, 0.1),
            Self::Shielded => Color::srgb(0.95, 0.85, 0.35),
        }
    }
}

#[derive(Component)]
pub struct MoltenFirePatch {
    pub left: f32,
    pub pos: Vec3,
}

#[derive(Component)]
pub(super) struct Enemy {
    pub def: &'static EnemyDef,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
    pub xp: u32,
    pub boss: bool,
    pub champion: Option<ChampionModifier>,
    pub champion_shield: f32,
    pub molten_timer: f32,
    pub attack_cd: f32,
    pub slow: f32,
    pub slow_t: f32,
    pub root_t: f32,
    pub stun_t: f32,
    pub fear_t: f32,
    pub weaken: f32,
    pub weaken_t: f32,
    pub dots: Vec<Dot>,
    pub dead: bool,
    pub ability_cd: f32,
    pub cast_t: f32,
    pub current_ability: Option<&'static data::BossAbilityDef>,
    pub ability_idx: usize,
}

pub(super) struct Dot {
    pub spell: u32,
    pub key: &'static str,
    pub dps: f32,
    pub left: f32,
    pub tick: f32,
}

/// A corpse playing its death before it fades.
#[derive(Component)]
pub(super) struct Dying(f32);

#[derive(Resource, Default)]
pub(super) struct Spawner {
    timer: f32,
}

/// An enemy died here, worth this much.
#[derive(Message, Clone, Copy)]
pub(super) struct Loot {
    pub pos: Vec3,
    pub xp: u32,
    pub boss: bool,
    pub elite: bool,
    pub double_gold: bool,
}

#[derive(Clone, Copy)]
pub(super) enum GemKind {
    Xp(u32),
    Heal(f32),
    Gold(u32),
}

#[derive(Component)]
pub(super) struct Gem {
    pub(super) kind: GemKind,
    pub(super) pulled: bool,
    pub(super) speed: f32,
}

/// The `GameObjectDisplayInfo` row and scale a pickup wears: a wisp per experience tier, the
/// bigger tiers rarer, a turkey leg for health, and a gold coin for gold.
fn look_for(kind: GemKind) -> (u32, f32) {
    match kind {
        // Gold coin: display ID 1201, scale 1.2.
        GemKind::Gold(_) => (1201, 1.2),
        // `TurkeyLeg.mdx`.
        GemKind::Heal(_) => (114, 2.5),
        // `BFD_WispSmallPurple.mdx`.
        GemKind::Xp(xp) if xp >= 20 => (1307, 1.0),
        // `BFD_WispSmallGreen.mdx`.
        GemKind::Xp(xp) if xp >= 5 => (1267, 1.0),
        // `BFD_WispSmall.mdx`.
        GemKind::Xp(_) => (426, 1.0),
    }
}

/// Body radius: the big beasts need more room.
fn radius_for(def: &EnemyDef) -> f32 {
    (if def.big { 1.3 } else { 0.6 }) * def.scale
}

pub(super) fn spawn_enemy(
    commands: &mut Commands,
    run: &mut Run,
    def: &'static EnemyDef,
    pos: Vec3,
    hero: Vec3,
    boss: bool,
    mult: (f32, f32, f32),
    champion: Option<ChampionModifier>,
) {
    let scale = data::hp_scale(run.elapsed);
    let serial = run.guid();
    let visual_scale = if champion.is_some() {
        def.scale * 1.35
    } else {
        def.scale
    };
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial,
            display: def.display,
            scale: visual_scale,
            level: def.level,
            faction: units::FACTION_MONSTER,
            bytes0: 1 << 8,
            weapons: &[],
            sheath: 0,
        },
        pos,
        yaw_toward(hero - pos),
    );
    let hp_mult = if champion.is_some() { 2.2 } else { 1.0 };
    let hp = def.hp * scale * mult.0 * hp_mult;
    let radius = if champion.is_some() {
        radius_for(def) * 1.35
    } else {
        radius_for(def)
    };
    let champion_shield = if champion == Some(ChampionModifier::Shielded) {
        400.0
    } else {
        0.0
    };
    if let Some(champ) = champion {
        run.banner = Some((format!("[{}] {} approaches!", champ.name(), def.name), 3.0));
    }
    commands.entity(e).insert((
        Enemy {
            def,
            hp,
            max_hp: hp,
            speed: def.speed,
            damage: def.damage * data::damage_scale(run.elapsed) * mult.1,
            radius,
            xp: if champion.is_some() { def.xp * 3 } else { def.xp },
            boss,
            champion,
            champion_shield,
            molten_timer: 0.0,
            attack_cd: 0.5,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: Vec::new(),
            dead: false,
            ability_cd: if boss { 3.0 } else { 0.0 },
            cast_t: 0.0,
            current_ability: None,
            ability_idx: 0,
        },
        Marks::debuffs(),
    ));
}

fn ground(world: &WorldPoint, mut p: Vec3, fallback: f32) -> Vec3 {
    p.y = world.terrain_height_under(p).unwrap_or(fallback);
    p
}

/// Keep the horde topped up on a ring outside the view, with the bosses and the encircling waves
/// on the clock.
pub(super) fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    selection: Res<Selection>,
    hero: Query<&Transform, With<Hero>>,
    enemies: Query<&Enemy>,
    world: WorldPoint,
    mut sounds: MessageWriter<GlueSound>,
) {
    let Ok(h) = hero.single() else { return };
    if run.dead_t.is_some() {
        return;
    }
    let hpos = h.translation;
    let elapsed = run.elapsed;
    let roster = data::roster(selection.map);
    let pool = data::wave_pool(elapsed, roster.enemies.len());
    let mut mult = selection.difficulty.multipliers();
    if run.overtime && elapsed > data::VICTORY_SECS {
        let ot_factor = 1.0 + ((elapsed - data::VICTORY_SECS) / 60.0) * 0.25;
        mult = (mult.0 * ot_factor, mult.1 * ot_factor, mult.2 * ot_factor);
    }

    let due_bosses = ((elapsed / data::BOSS_EVERY_SECS) as u32).min(roster.bosses.len() as u32);
    while run.bosses < due_bosses {
        let def = &roster.bosses[run.bosses as usize];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let pos = ground(
            &world,
            hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 30.0,
            hpos.y,
        );
        spawn_enemy(&mut commands, &mut run, def, pos, hpos, true, mult, None);
        run.bosses += 1;
        run.banner = Some((format!("{} approaches!", def.name), 4.0));
        sounds.write(GlueSound("gsCharacterSelectionEnterWorld"));
    }

    // Half way between bosses, a closing ring of the moment's weakest.
    let due_rings = ((elapsed + data::BOSS_EVERY_SECS / 2.0) / data::BOSS_EVERY_SECS) as u32;
    if run.rings < due_rings {
        run.rings = due_rings;
        let def = &roster.enemies[pool[0]];
        let mut n = 28 + (data::progress(elapsed) * 60.0) as usize;
        if run.affixes.horde_stampede {
            n = (n as f32 * 1.25).round() as usize;
        }
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let pos = ground(
                &world,
                hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 26.0,
                hpos.y,
            );
            spawn_enemy(&mut commands, &mut run, def, pos, hpos, false, mult, None);
        }
        run.banner = Some(("They're surrounding you!".into(), 3.0));
    }

    if run.overtime && elapsed > data::VICTORY_SECS {
        let ot_secs = elapsed - data::VICTORY_SECS;
        let due_ot_rings = (ot_secs / 45.0) as u32 + 1;
        if run.overtime_rings < due_ot_rings {
            run.overtime_rings = due_ot_rings;
            let def = &roster.enemies[pool[1]];
            let mut n = 32;
            if run.affixes.horde_stampede {
                n = (n as f32 * 1.25).round() as usize;
            }
            for i in 0..n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                let pos = ground(
                    &world,
                    hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 28.0,
                    hpos.y,
                );
                spawn_enemy(&mut commands, &mut run, def, pos, hpos, false, mult, None);
            }
            if due_ot_rings % 2 == 0 && !roster.bosses.is_empty() {
                let boss_def = &roster.bosses[rng.index(roster.bosses.len())];
                let a = rng.range(0.0, std::f32::consts::TAU);
                let bpos = ground(&world, hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 32.0, hpos.y);
                spawn_enemy(&mut commands, &mut run, boss_def, bpos, hpos, true, mult, None);
                run.banner = Some((format!("OVERTIME BOSS: {} has risen!", boss_def.name), 4.0));
                sounds.write(GlueSound("gsCharacterSelectionEnterWorld"));
            } else {
                run.banner = Some(("Overtime horde wave incoming!".into(), 3.0));
            }
        }
    }

    spawner.timer -= time.delta_secs();
    if spawner.timer > 0.0 {
        return;
    }
    spawner.timer = 0.12;
    let alive = enemies.iter().filter(|e| !e.dead).count();
    let mut target = data::target_alive(elapsed);
    if run.affixes.horde_stampede {
        target = (target as f32 * 1.25).round() as usize;
    }
    if alive >= target {
        return;
    }
    let n = (((target - alive) as f32) / 6.0).ceil().min(4.0) as usize;
    for _ in 0..n {
        let def = &roster.enemies[pool[rng.index(pool.len())]];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(34.0, 42.0);
        let pos = ground(&world, hpos + Vec3::new(a.cos(), 0.0, a.sin()) * r, hpos.y);
        let champion = if elapsed >= 90.0 && rng.f32() < 0.06 {
            Some(ChampionModifier::ALL[rng.index(ChampionModifier::ALL.len())])
        } else {
            None
        };
        spawn_enemy(&mut commands, &mut run, def, pos, hpos, false, mult, champion);
    }
}

/// The chase: straight at the hero, pushed apart from each other, stopped by crowd control, and
/// swinging when in reach.
#[allow(clippy::type_complexity)]
pub(super) fn move_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    world: WorldPoint,
    hero: Query<(Entity, &Transform), (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, &mut MovementState), Without<Hero>>,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cache: ResMut<TelegraphCache>,
    events_state: Option<Res<super::events::WorldEventState>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((hero_e, htf)) = hero.single() else {
        return;
    };
    let hpos = htf.translation;
    let hero_down = run.dead_t.is_some();
    let blood_moon = events_state.as_ref().is_some_and(|s| s.blood_moon_t > 0.0);

    // Separation over a coarse grid: only neighbours in the same or adjacent cells push.
    const CELL: f32 = 3.0;
    let bodies: Vec<(Entity, Vec3, f32)> = enemies
        .iter()
        .filter(|(_, _, e, _)| !e.dead)
        .map(|(ent, tf, e, _)| (ent, tf.translation, e.radius))
        .collect();
    let key = |p: Vec3| ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, b) in bodies.iter().enumerate() {
        grid.entry(key(b.1)).or_default().push(i);
    }
    let mut push: HashMap<Entity, Vec3> = HashMap::with_capacity(bodies.len());
    for (i, &(ent, pos, r)) in bodies.iter().enumerate() {
        let (cx, cz) = key(pos);
        let mut acc = Vec3::ZERO;
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(cell) = grid.get(&(cx + dx, cz + dz)) else {
                    continue;
                };
                for &j in cell {
                    if j == i {
                        continue;
                    }
                    let (_, other, or) = bodies[j];
                    let d = flat(pos - other);
                    let len = d.length();
                    let min = r + or;
                    if len < min && len > 1e-4 {
                        acc += d / len * (min - len);
                    } else if len <= 1e-4 {
                        acc += Vec3::new((i as f32).sin(), 0.0, (i as f32).cos()) * 0.1;
                    }
                }
            }
        }
        push.insert(ent, acc);
    }

    for (ent, mut tf, mut e, mut mv) in &mut enemies {
        let e = &mut *e;
        if e.dead {
            if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            continue;
        }
        e.attack_cd -= dt;
        for t in [
            &mut e.slow_t,
            &mut e.root_t,
            &mut e.stun_t,
            &mut e.fear_t,
            &mut e.weaken_t,
        ] {
            *t = (*t - dt).max(0.0);
        }
        if e.slow_t <= 0.0 {
            e.slow = 1.0;
        }
        if e.weaken_t <= 0.0 {
            e.weaken = 1.0;
        }
        let pos = tf.translation;
        let to_hero = flat(hpos - pos);
        let dist = to_hero.length();

        // Strays far behind are brought back in front, as the reference genre does.
        if dist > 75.0 {
            let back = hpos + to_hero.normalize_or_zero() * 38.0;
            tf.translation = ground(&world, back, hpos.y);
            continue;
        }

        // Under Fade the horde has lost the hero: it mills where it stands.
        let lost = run.mobile.vanish_t > 0.0 && !e.boss;
        let is_casting = e.cast_t > 0.0;

        if e.boss && !e.dead && !hero_down {
            if is_casting {
                e.cast_t = (e.cast_t - dt).max(0.0);
                if e.cast_t <= 0.0 {
                    e.current_ability = None;
                }
            } else {
                e.ability_cd -= dt;
                if e.ability_cd <= 0.0 && !e.def.abilities.is_empty() && dist < 35.0 {
                    let ability = &e.def.abilities[e.ability_idx % e.def.abilities.len()];
                    e.ability_idx += 1;
                    e.ability_cd = ability.cooldown;
                    e.cast_t = ability.windup;
                    e.current_ability = Some(ability);

                    run.banner =
                        Some((format!("{} prepares {}!", e.def.name, ability.name), 2.5));
                    fx.start_cast(ent, ability.spell_id);

                    let aim_dir = if to_hero.length_squared() > 1e-4 {
                        to_hero.normalize()
                    } else {
                        -Vec3::Z
                    };
                    let origin = match ability.shape {
                        data::TelegraphShape::Circle { .. } => {
                            if ability.name.contains("Pounce")
                                || ability.name.contains("Trap")
                                || ability.name.contains("Rain")
                                || ability.name.contains("Fissure")
                                || ability.name.contains("Shower")
                                || ability.name.contains("Eruption")
                                || ability.name.contains("Reflection")
                                || ability.name.contains("Spill")
                            {
                                hpos
                            } else {
                                pos
                            }
                        }
                        _ => pos,
                    };

                    telegraphs::spawn_telegraph(
                        &mut commands,
                        &mut meshes,
                        &mut materials,
                        &mut cache,
                        &world,
                        ability,
                        ent,
                        origin,
                        aim_dir,
                    );
                }
            }
        }

        let is_casting = e.cast_t > 0.0;
        let held = e.stun_t > 0.0 || e.root_t > 0.0 || lost || is_casting;
        let contact = dist <= HERO_RADIUS + e.radius + 0.35;
        let mut step = Vec3::ZERO;
        let mut speed = 0.0;
        if !held && !hero_down {
            let dir = if e.fear_t > 0.0 {
                -to_hero.normalize_or_zero()
            } else {
                to_hero.normalize_or_zero()
            };
            if !contact || e.fear_t > 0.0 {
                speed = e.speed * e.slow;
                if blood_moon {
                    speed *= 1.35;
                }
                if run.affixes.horde_stampede {
                    speed *= 1.30;
                }
                if e.champion == Some(ChampionModifier::Avenger) && e.hp <= e.max_hp * 0.35 {
                    speed *= 1.50;
                }
                step = dir * speed;
            }
            if e.champion == Some(ChampionModifier::Molten) && speed > 0.0 {
                e.molten_timer -= dt;
                if e.molten_timer <= 0.0 {
                    e.molten_timer = 0.5;
                    commands.spawn((
                        super::RunEntity,
                        MoltenFirePatch {
                            left: 3.0,
                            pos: tf.translation,
                        },
                    ));
                }
            }
            if dir != Vec3::ZERO {
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
            }
        } else if is_casting && to_hero.length_squared() > 1e-4 {
            let target = Quat::from_rotation_y(yaw_toward(to_hero.normalize()));
            tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
        }
        let shove = if is_casting {
            Vec3::ZERO
        } else {
            push.get(&ent).copied().unwrap_or_default()
        };
        let mut next = pos + step * dt + shove * (dt * 8.0).min(0.5);
        next = ground(&world, next, pos.y);
        tf.translation = next;
        if speed > 0.0 {
            mv.speed = speed;
            mv.flags = move_flags::FORWARD;
        } else if mv.speed != 0.0 || mv.flags != 0 {
            mv.speed = 0.0;
            mv.flags = 0;
        }

        if contact
            && !lost
            && !is_casting
            && e.stun_t <= 0.0
            && e.fear_t <= 0.0
            && e.attack_cd <= 0.0
            && !hero_down
        {
            e.attack_cd = if e.boss { 1.0 } else { 1.3 };
            let mut raw = e.damage * e.weaken;
            if blood_moon {
                raw *= 1.30;
            }
            let taken = hero::hurt_hero(&mut run, raw);
            fx.swing(ent, hero_e, taken);
            if taken > 0.0 {
                fx.number(hero_e, taken, false, COLOR_HURT);
                if e.champion == Some(ChampionModifier::Vampiric) {
                    let leech = taken * 0.25;
                    e.hp = (e.hp + leech).min(e.max_hp);
                    fx.number(ent, leech, false, COLOR_HEAL);
                }
            }
            if run.thorns_t > 0.0 {
                hits.write(Hit {
                    spell: run.thorns_spell,
                    key: "thorns",
                    ..Hit::direct(ent, run.thorns * run.stats().might, false)
                });
            }
        }
    }
}

/// Mark timers run down; the state kits reap with them.
pub(super) fn tick_marks(
    time: Res<Time>,
    mut units: Query<(Entity, &Guid, &mut ObjectStore, &mut Marks), With<Enemy>>,
    mut fields: MessageWriter<FieldChanged>,
) {
    let dt = time.delta_secs();
    for (entity, guid, mut store, mut marks) in &mut units {
        marks.tick(dt);
        marks.flush(entity, guid.0, &mut store, &mut fields);
    }
}

/// Corpses fade once their death has played.
pub(super) fn reap_dead(
    mut commands: Commands,
    time: Res<Time>,
    mut dying: Query<(Entity, &mut Dying)>,
) {
    for (e, mut d) in &mut dying {
        d.0 += time.delta_secs();
        if d.0 > 2.5 {
            commands
                .entity(e)
                .remove::<Dying>()
                .insert(benilla_world::model_fade::DespawnFade::default());
        }
    }
}

pub(super) fn mark_dying(commands: &mut Commands, e: Entity) {
    commands.entity(e).insert(Dying(0.0));
}

/// Most gems on the ground at once; past it, a kill's worth joins an existing gem.
const MAX_GEMS: usize = 350;

pub(super) fn drop_loot(
    mut commands: Commands,
    mut drops: MessageReader<Loot>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
    mut gems: Query<&mut Gem>,
    events_state: Option<Res<super::events::WorldEventState>>,
) {
    let is_blood_moon = events_state.as_ref().is_some_and(|s| s.blood_moon_t > 0.0);
    let mut count = gems.iter().count();
    for d in drops.read() {
        if is_blood_moon {
            super::breakables::spawn_powerup(&mut commands, &mut run, &mut rng, d.pos);
        }
        super::ultimates::charge_ultimate_on_kill(&mut run, d.elite, d.boss);
        if d.boss {
            let is_major = run.elapsed >= 600.0;
            chests::spawn_chest(&mut commands, d.pos, is_major, run.upgrades.fortune);
        }
        if d.boss || rng.f32() < 0.015 {
            if run.affixes.midas_curse {
                let bonus_gold = if d.boss { 150 } else { 25 };
                spawn_gem(&mut commands, run.guid(), d.pos + Vec3::X * 0.8, GemKind::Gold(bonus_gold));
            } else {
                let heal = GemKind::Heal(if d.boss { 1000.0 } else { 30.0 });
                spawn_gem(&mut commands, run.guid(), d.pos + Vec3::X * 0.8, heal);
            }
        }
        if d.double_gold {
            let gold = (rng.range(12.0, 30.0) as u32) * 2;
            spawn_gem(&mut commands, run.guid(), d.pos + Vec3::Z * 0.5, GemKind::Gold(gold));
        } else if d.boss {
            let gold = rng.range(150.0, 300.0) as u32;
            spawn_gem(&mut commands, run.guid(), d.pos - Vec3::X * 0.8, GemKind::Gold(gold));
        } else if rng.f32() < 0.10 {
            let gold = rng.range(5.0, 25.0) as u32;
            spawn_gem(&mut commands, run.guid(), d.pos + Vec3::Z * 0.5, GemKind::Gold(gold));
        }
        if is_blood_moon {
            // ALL killed enemies drop 3x XP gems
            for i in 0..3 {
                let angle = i as f32 * std::f32::consts::TAU / 3.0;
                let offset = Vec3::new(angle.cos() * 0.8, 0.0, angle.sin() * 0.8);
                spawn_gem(&mut commands, run.guid(), d.pos + offset, GemKind::Xp(d.xp));
            }
            count += 3;
        } else {
            if count >= MAX_GEMS && !d.boss {
                if let Some(mut g) = gems.iter_mut().find(|g| matches!(g.kind, GemKind::Xp(_))) {
                    if let GemKind::Xp(xp) = &mut g.kind {
                        *xp += d.xp;
                    }
                    continue;
                }
            }
            spawn_gem(&mut commands, run.guid(), d.pos, GemKind::Xp(d.xp));
            count += 1;
        }
    }
}

pub(super) fn spawn_gem(commands: &mut Commands, serial: u64, pos: Vec3, kind: GemKind) {
    let (display, scale) = look_for(kind);
    let e = units::spawn_object(commands, serial, display, scale, pos + Vec3::Y * 0.5);
    commands.entity(e).insert(Gem {
        kind,
        pulled: false,
        speed: 6.0,
    });
}

/// Gems in the magnet's reach fly to the hero; levels bank as they land.
pub(super) fn collect_gems(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    mut next: ResMut<NextState<Phase>>,
    hero: Query<(Entity, &Transform), (With<Hero>, Without<Gem>)>,
    mut gems: Query<(Entity, &mut Transform, &mut Gem)>,
    mut fx: Fx,
) {
    let Ok((hero_e, h)) = hero.single() else { return };
    if run.dead_t.is_some() {
        return;
    }
    let dt = time.delta_secs();
    let stats = run.stats();
    let mult = selection.difficulty.multipliers();
    let target = h.translation + Vec3::Y * 1.0;
    for (e, mut tf, mut g) in &mut gems {
        let d = target - tf.translation;
        let dist = d.length();
        if !g.pulled && flat(d).length() <= stats.magnet {
            g.pulled = true;
        }
        if !g.pulled {
            continue;
        }
        if dist < 0.9 {
            match g.kind {
                GemKind::Xp(xp) => {
                    run.xp += xp as f32 * data::XP_RATE * stats.growth * mult.2 * stats.xp_bonus
                }
                GemKind::Heal(hp) => run.hp = (run.hp + hp).min(stats.max_hp),
                GemKind::Gold(gold) => {
                    let scaled_gold =
                        (gold as f32 * stats.growth * stats.gold_bonus).round().max(1.0) as u64;
                    run.gold += scaled_gold;
                    fx.number(hero_e, scaled_gold as f32, false, 0xFFFF_D700);
                }
            }
            commands.entity(e).despawn();
            continue;
        }
        g.speed += 30.0 * dt;
        tf.translation += d / dist * (g.speed * dt).min(dist);
    }
    while run.xp >= data::xp_to_next(run.level) {
        run.xp -= data::xp_to_next(run.level);
        run.level += 1;
        run.pending_levels += 1;
        if run.level == super::mobility::LEARN_LEVEL {
            if let Some(m) = super::mobility::for_class(run.class.id) {
                run.banner = Some((format!("You have learned {}! Press F.", m.name), 4.0));
            }
        }
    }
    if run.pending_levels > 0 {
        next.set(Phase::LevelUp);
    }
}

/// Pickups hang still: a wisp's own loop (its circling motes and their trails) stops on its first
/// frame, clip and global sequences alike, as soon as the model attaches.
pub(super) fn still_gems(
    mut players: Query<&mut AnimationPlayer, (With<Gem>, Added<AnimationPlayer>)>,
    mut drives: Query<&mut GlobalSeqDrive, (With<Gem>, Added<GlobalSeqDrive>)>,
) {
    for mut player in &mut players {
        for (_, active) in player.playing_animations_mut() {
            active.set_speed(0.0);
        }
    }
    for mut drive in &mut drives {
        drive.set_paused(true);
    }
}

pub(super) fn tick_molten_fire(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    hero: Query<(Entity, &Transform), With<Hero>>,
    mut patches: Query<(Entity, &mut MoltenFirePatch)>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || run.dead_t.is_some() {
        return;
    }
    let Ok((hero_e, htf)) = hero.single() else { return };
    let hpos = htf.translation;

    for (e, mut patch) in &mut patches {
        patch.left -= dt;
        if patch.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let dist = flat(hpos - patch.pos).length();
        if dist <= 1.8 {
            let taken = hero::hurt_hero(&mut run, 14.0 * dt);
            if taken > 0.0 {
                fx.number(hero_e, taken, false, 0xFFFF_4500);
            }
        }
    }
}
