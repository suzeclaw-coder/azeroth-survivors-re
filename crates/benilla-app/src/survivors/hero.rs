//! The hero: its body, WASD movement over the terrain, the top-down camera, and its health.

use avian3d::character_controller::move_and_slide::{
    MoveAndSlideConfig, MoveAndSlideHitResponse,
};
use avian3d::prelude::Collider;
use benilla_world::collision::WorldCollision;
use benilla_world::view::WorldCamera;
use benilla_world::world_point::WorldPoint;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::units::{self, flat, wow_to_bevy, yaw_toward, Fx, Marks, UnitSpec};
use super::{Phase, Run, Selection};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

#[derive(Component)]
pub(super) struct Hero {
    /// Vertical velocity in yards/second while airborne; 0 on the ground.
    pub(super) vy: f32,
    /// Height above the ground while airborne. Kept here, not in the transform: the creature
    /// ground clamp (`net::motion::ground_clamp_creatures`) re-seats every unit on the floor at
    /// the top of the frame, so the arc is re-applied over the ground each frame.
    pub(super) air: f32,
}

/// The body radius enemies stop at.
pub(super) const HERO_RADIUS: f32 = 0.7;

/// The hop's initial upward speed and the gravity that brings it back down — the reference
/// `CMovement` jump seed and gravity (`player/state.rs`'s `JUMP_SPEED` / `GRAVITY`).
const JUMP_SPEED: f32 = 7.955_547;
const GRAVITY: f32 = 19.291_105;

/// Radians of orbit per raw mouse unit while right-dragging.
const ORBIT_RATE: f32 = 0.008;
/// Elevation clamp (radians): ~10°..~83° above the horizon, so the combat camera never dives underground.
const PITCH_MIN: f32 = 0.18;
const PITCH_MAX: f32 = 1.45;
/// Orbit and Town Hub elevation clamp: allows negative pitch (~ -66° below horizon) so the camera
/// can tilt upward to frame the staircase, second-floor loft, chandeliers, and high structures.
const ORBIT_PITCH_MIN: f32 = -1.15;
/// The locked view's tilt.
pub(super) const FIXED_PITCH: f32 = 61.0 * std::f32::consts::PI / 180.0;
/// Town Hub 3rd-person eye-level RPG view tilt (~18° above horizon).
pub(super) const HUB_PITCH: f32 = 18.0 * std::f32::consts::PI / 180.0;
/// Town Hub default camera distance from hero (yards).
pub(super) const HUB_DIST: f32 = 6.0;
/// Wheel zoom range, yards from the hero's shoulders.
const ZOOM_MIN: f32 = 1.5;
const ZOOM_MAX: f32 = 70.0;

/// Whether the camera is locked; `C` toggles it.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum CamMode {
    /// Locked top-down (58°), laptop-friendly — the original feel.
    #[default]
    Fixed,
    /// Unlocked: a mouse drag swings and tilts it around the hero.
    Orbit,
}

#[derive(Resource)]
pub(super) struct CamRig {
    pub dist: f32,
    pub orbit: f32,
    /// Orbit azimuth (radians) around the hero; right-drag swings it during play.
    pub yaw: f32,
    /// Elevation above the horizon (radians), clamped so the camera never dives underground.
    pub pitch: f32,
    pub mode: CamMode,
    pub shake_t: f32,
    pub shake_strength: f32,
}

impl CamRig {
    /// Triggers an impulse camera shake for high-impact combat moments.
    pub fn shake(&mut self, strength: f32, duration: f32) {
        self.shake_strength = self.shake_strength.max(strength);
        self.shake_t = self.shake_t.max(duration);
    }
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            dist: 30.0,
            orbit: 0.6,
            yaw: 0.0,
            pitch: FIXED_PITCH,
            mode: CamMode::Fixed,
            shake_t: 0.0,
            shake_strength: 0.0,
        }
    }
}

/// Where a battleground's runs start.
pub(super) fn arena(map: &data::MapDef) -> Vec3 {
    wow_to_bevy(map.center)
}

/// The world is the battleground being stood in ([`Selection::stage`]): entering one on another
/// continent worldports, which raises the loading screen until the new ground is in.
pub(super) fn follow_map(
    selection: Res<Selection>,
    phase: Res<State<Phase>>,
    current: Option<ResMut<benilla_world::world_map::CurrentMap>>,
) {
    let Some(mut current) = current else {
        return;
    };
    let want = if *phase.get() == Phase::Hub {
        super::hub::HUB_MAP_ID
    } else {
        selection.stage_def().map
    };
    if current.0 != want {
        current.0 = want;
    }
}

/// The hero in the class trainer's look, weapons drawn.
pub(super) fn spawn_hero(
    commands: &mut Commands,
    class: &'static ClassDef,
    display: u32,
    race: u8,
    sex: u8,
    serial: u64,
    pos: Vec3,
) -> Entity {
    let bytes0 = u32::from(race)
        | u32::from(class.id) << 8
        | u32::from(sex) << 16
        | u32::from(class.power) << 24;
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial,
            display,
            scale: 1.0,
            level: 60,
            faction: units::FACTION_FRIENDLY,
            bytes0,
            weapons: class.weapons,
            sheath: class.sheath,
        },
        pos,
        0.0,
    );
    commands
        .entity(e)
        .insert((Hero { vy: 0.0, air: 0.0 }, Marks::buffs()));
    e
}

/// Midday over the battleground, whatever the clock says.
pub(super) const MINUTE_OF_DAY: u32 = 780;

pub(super) fn daylight(
    debug: Option<ResMut<benilla_world::dev_state::DebugState>>,
    event_state: Option<Res<super::events::WorldEventState>>,
) {
    let Some(mut debug) = debug else { return };
    let is_blood_moon = event_state.as_ref().is_some_and(|s| s.blood_moon_t > 0.0);
    // Midnight darkness during Blood Moon eclipse, bright midday sun otherwise
    let target_minute = if is_blood_moon { 0 } else { MINUTE_OF_DAY };
    if debug.lighting.follow_server_time || debug.lighting.manual_minute != target_minute {
        debug.lighting.follow_server_time = false;
        debug.lighting.manual_minute = target_minute;
    }
}

/// The ground under the mouse cursor, `None` with the cursor off the window. The locked camera's
/// hero faces it, and the aimed mobility spells (Blink, Charge) go toward it.
#[derive(Resource, Default)]
pub(super) struct AimPoint(pub(super) Option<Vec3>);

/// Cast the cursor onto the terrain: the camera ray meets the plane at the hero's height, then the
/// terrain height there, a few times over, which settles on hills within a frame.
pub(super) fn aim_cursor(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    world: WorldPoint,
    gamepads: Query<&Gamepad>,
    mut aim: ResMut<AimPoint>,
) {
    let mut gamepad_aim = None;
    for gp in &gamepads {
        let rx = gp.get(GamepadAxis::RightStickX).unwrap_or(0.0);
        let ry = gp.get(GamepadAxis::RightStickY).unwrap_or(0.0);
        if rx * rx + ry * ry > 0.04 {
            if let Ok(htf) = hero.single() {
                let hpos = htf.translation;
                let aim_dir = Vec3::new(rx, 0.0, -ry).normalize_or_zero();
                let p = hpos + aim_dir * 14.0;
                let ground_y = world.terrain_height_under(p).unwrap_or(hpos.y);
                gamepad_aim = Some(Vec3::new(p.x, ground_y, p.z));
                break;
            }
        }
    }

    let point = gamepad_aim.or_else(|| {
        let cursor = windows.single().ok()?.cursor_position()?;
        let (cam, tf) = cams.single().ok()?;
        let ray = cam
            .viewport_to_world(&GlobalTransform::from(*tf), cursor)
            .ok()?;
        let mut y = hero.single().ok()?.translation.y;
        let mut at = None;
        for _ in 0..4 {
            if ray.direction.y.abs() < 1e-4 {
                break;
            }
            let t = (y - ray.origin.y) / ray.direction.y;
            if t <= 0.0 {
                break;
            }
            let p = ray.origin + *ray.direction * t;
            at = Some(p);
            match world.terrain_height_under(p) {
                Some(h) if (h - y).abs() > 0.05 => y = h,
                _ => break,
            }
        }
        at
    });
    if aim.0 != point {
        aim.0 = point;
    }
}

/// WASD over the terrain within the battleground's leash, and the streaming focus on the hero.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_hero(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    rig: Res<CamRig>,
    aim: Res<AimPoint>,
    world: WorldPoint,
    collide: WorldCollision,
    auto: Option<Res<super::auto::Auto>>,
    mut player: ResMut<crate::player::Player>,
    mut hero: Query<(Entity, &mut Transform, &mut MovementState, &mut Hero)>,
    enemies: Query<(&Transform, &super::enemies::Enemy), Without<Hero>>,
    mut fx: Fx,
) {
    let focus = match hero.single_mut() {
        Ok((me, mut tf, mut mv, mut hm)) => {
            let dt = time.delta_secs();
            let mut dir = Vec3::ZERO;
            let active = (*phase.get() == Phase::Playing || *phase.get() == Phase::Hub) && run.dead_t.is_none();
            if let Some(auto) = auto
                .as_deref()
                .filter(|_| active)
            {
                dir = super::auto::steer(auto);
            } else if active {
                if keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
                    dir.z -= 1.0;
                }
                if keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
                    dir.z += 1.0;
                }
                if keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
                    dir.x -= 1.0;
                }
                if keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
                    dir.x += 1.0;
                }
                for gp in &gamepads {
                    let lx = gp.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
                    let ly = gp.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
                    if lx.abs() > 0.18 {
                        dir.x += lx;
                    }
                    if ly.abs() > 0.18 {
                        dir.z -= ly;
                    }
                    if gp.pressed(GamepadButton::DPadUp) {
                        dir.z -= 1.0;
                    }
                    if gp.pressed(GamepadButton::DPadDown) {
                        dir.z += 1.0;
                    }
                    if gp.pressed(GamepadButton::DPadLeft) {
                        dir.x -= 1.0;
                    }
                    if gp.pressed(GamepadButton::DPadRight) {
                        dir.x += 1.0;
                    }
                }
                // In orbit mode WASD is camera-relative; in fixed mode it stays world-relative.
                if rig.mode == CamMode::Orbit {
                    let (sy, cy) = rig.yaw.sin_cos();
                    dir = Vec3::new(dir.x * cy + dir.z * sy, 0.0, dir.z * cy - dir.x * sy);
                }
            }
            let mut pos = tf.translation;
            // Locked, the hero faces the cursor and WASD moves him under it, so he backpedals and
            // strafes as a player does; unlocked (or driven), he faces where he runs.
            let face = (rig.mode == CamMode::Fixed
                && auto.is_none()
                && active)
            .then(|| aim.0.map(|a| flat(a - pos)))
            .flatten()
            .filter(|d| d.length() > 0.5)
            .map(|d| d.normalize());
            if let Some(look) = face {
                let target = Quat::from_rotation_y(yaw_toward(look));
                tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
            }

            // Player collision capsule for horizontal obstacles (walls, doorframes, railings, counters):
            // Radius 0.30 yd, cylinder height 0.9 yd (total height 1.5 yd).
            // Bottom of the capsule is raised 0.45 yd above pos.y to glide over door sills and low curbs,
            // while stopping solidly against full-height walls and railings.
            const STEP_CLEARANCE: f32 = 0.45;
            const CAP_RADIUS: f32 = 0.30;
            const CAP_CYLINDER: f32 = 0.90;
            let capsule = Collider::capsule(CAP_RADIUS, CAP_CYLINDER);
            let shape_center = |p: Vec3| {
                Vec3::new(
                    p.x,
                    p.y + STEP_CLEARANCE + CAP_RADIUS + CAP_CYLINDER * 0.5,
                    p.z,
                )
            };

            if dir != Vec3::ZERO && dt > 0.0 {
                let dir = dir.normalize();
                let mut speed = run.stats().speed * run.haste_mult();
                let near_frozen = enemies.iter().any(|(etf, en)| {
                    en.champion == Some(super::enemies::ChampionModifier::Frozen)
                        && !en.dead
                        && (etf.translation - pos).length_squared() <= 36.0
                });
                if near_frozen {
                    speed *= 0.75;
                }

                // Check ahead for stairs or elevated walkable surfaces.
                // Probing ahead lets the collision capsule step up over stair risers, thresholds, and porches
                // instead of catching on the vertical riser as a horizontal wall.
                let advance = (speed * dt).max(0.35);
                let probe_ahead = Vec3::new(pos.x + dir.x * advance, (pos.y - hm.air) + 1.8, pos.z + dir.z * advance);
                let step_lift = collide
                    .ray_body(probe_ahead, Dir3::NEG_Y, 8.0)
                    .filter(|h| h.normal.y >= 0.45)
                    .map(|h| (probe_ahead.y - h.distance) - (pos.y - hm.air))
                    .filter(|&rise| rise > 0.0 && rise <= 1.25)
                    .unwrap_or(0.0);

                // Slide body horizontally along walls and obstacles, lifted if stepping onto stairs
                let horiz_vel = dir * speed;
                let center_now = shape_center(pos + Vec3::Y * step_lift);
                let out = collide.slide_body(
                    &capsule,
                    center_now,
                    horiz_vel,
                    time.delta(),
                    &MoveAndSlideConfig::default(),
                    |hit| {
                        // Walkable floors, stairs, and slopes are ignored for horizontal collisions
                        if hit.normal.y >= 0.45 {
                            MoveAndSlideHitResponse::Ignore
                        } else {
                            // Flatten obstacle normal horizontally so player slides smoothly along walls
                            let n_h = Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize_or_zero();
                            if n_h != Vec3::ZERO {
                                *hit.normal = Dir3::new_unchecked(n_h);
                            }
                            MoveAndSlideHitResponse::Accept
                        }
                    },
                );
                pos.x = out.position.x;
                pos.z = out.position.z;

                mv.speed = speed;
                mv.flags = match face {
                    Some(look) => gait(look, dir),
                    None => {
                        let target = Quat::from_rotation_y(yaw_toward(dir));
                        tf.rotation = tf.rotation.slerp(target, (dt * 20.0).min(1.0));
                        move_flags::FORWARD
                    }
                };
            } else if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            let is_hub = *phase.get() == Phase::Hub;
            let playing = (*phase.get() == Phase::Playing || is_hub) && run.dead_t.is_none();
            let gp_dash = gamepads.iter().any(|gp| {
                gp.just_pressed(GamepadButton::RightTrigger2)
                    || gp.just_pressed(GamepadButton::East)
            });
            if run.hearth_channel.is_some()
                && (dir != Vec3::ZERO
                    || keys.any_just_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
                    || gp_dash
                    || keys.just_pressed(KeyCode::Space))
            {
                run.hearth_channel = None;
                run.banner = Some(("Hearthstone Interrupted!".to_string(), 1.5));
            }
            if *phase.get() == Phase::Playing && keys.just_pressed(KeyCode::KeyH) && run.dead_t.is_none() {
                if run.hearth_channel.is_some() {
                    run.hearth_channel = None;
                    run.banner = Some(("Hearthstone Cancelled".to_string(), 1.2));
                } else {
                    run.hearth_channel = Some(8.0);
                    fx.cast(me, 8690, &[me], None);
                    run.banner = Some(("Channeling Hearthstone... Stand Still!".to_string(), 2.0));
                }
            }
            if playing
                && (keys.any_just_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) || gp_dash)
                && run.dash_cd <= 0.0
            {
                let dash_dir = if dir != Vec3::ZERO {
                    dir.normalize()
                } else {
                    flat(tf.rotation * Vec3::NEG_Z).normalize_or(Vec3::NEG_Z)
                };
                tf.rotation = Quat::from_rotation_y(yaw_toward(dash_dir));
                let dash_disp = dash_dir * 12.0;
                let probe_dash = Vec3::new(pos.x + dash_dir.x * 0.5, (pos.y - hm.air) + 1.8, pos.z + dash_dir.z * 0.5);
                let dash_lift = collide
                    .ray_body(probe_dash, Dir3::NEG_Y, 8.0)
                    .filter(|h| h.normal.y >= 0.45)
                    .map(|h| (probe_dash.y - h.distance) - (pos.y - hm.air))
                    .filter(|&rise| rise > 0.0 && rise <= 1.25)
                    .unwrap_or(0.0);
                let center_now = shape_center(pos + Vec3::Y * dash_lift);
                let actual_disp = match collide.cast_body(&capsule, center_now, dash_disp, 0.05) {
                    Some(hit) => dash_dir * (hit.distance - 0.15).max(0.0),
                    None => dash_disp,
                };
                pos += actual_disp;
                run.dash_cd = run.dash_max_cd;
                run.iframe_t = 0.25;
                fx.cast(me, 1604, &[me], None);
                hm.air = 0.0;
                hm.vy = 0.0;
            }
            let (center, leash) = if is_hub {
                (super::hub::hub_center(), super::hub::HUB_LEASH)
            } else {
                let map = selection.stage_def();
                (arena(map), map.leash)
            };
            let off = flat(pos - center);
            if off.length() > leash * 2.0 {
                // Just entered another battleground or hub: start at its centre.
                pos = center;
            } else if off.length() > leash {
                let held = center + off.normalize() * leash;
                pos.x = held.x;
                pos.z = held.z;
            }

            // 3D vertical floor and stairs resolution via WorldCollision raycast.
            // Probing downward from chest height (pos.y - hm.air + 1.8) catches WMO stairs,
            // tavern porches, second-floor boards, bridges, and outdoor terrain alike.
            let base_y = pos.y - hm.air;
            let origin = Vec3::new(pos.x, base_y + 1.8, pos.z);
            let probed_wmo = collide
                .ray_body(origin, Dir3::NEG_Y, 8.0)
                .filter(|h| h.normal.y >= 0.45)
                .map(|h| origin.y - h.distance);
            let terrain_ground = world.terrain_height_under(pos);
            let hero_indoors = world.indoors_at(pos);

            let target_ground = if hero_indoors {
                probed_wmo.or(terrain_ground).unwrap_or(base_y)
            } else {
                match (probed_wmo, terrain_ground) {
                    (Some(w), Some(t)) => {
                        // Outdoors, elevated structures (porch, bridge, stairs, dock) take precedence over terrain
                        if w >= t - 0.4 {
                            w
                        } else {
                            t
                        }
                    }
                    (Some(w), None) => w,
                    (None, Some(t)) => t,
                    (None, None) => base_y,
                }
            };

            let playing = (*phase.get() == Phase::Playing || is_hub) && run.dead_t.is_none();
            let gp_jump = gamepads
                .iter()
                .any(|gp| gp.just_pressed(GamepadButton::South));
            // Space or Gamepad A hops: lift off the terrain/floor, then gravity brings the hero back down.
            if playing
                && (keys.just_pressed(KeyCode::Space) || gp_jump)
                && hm.vy == 0.0
                && hm.air == 0.0
            {
                hm.vy = JUMP_SPEED;
            }
            if playing && (hm.vy != 0.0 || hm.air > 0.0) {
                hm.vy -= GRAVITY * dt;
                hm.air += hm.vy * dt;
                if hm.air <= 0.0 || (base_y + hm.air <= target_ground && hm.vy <= 0.0) {
                    hm.air = 0.0;
                    hm.vy = 0.0;
                    pos.y = target_ground;
                } else {
                    pos.y = base_y + hm.air;
                }
            } else {
                // Grounded movement: smooth stepping over stairs, curbs, and ledges
                let step_up = target_ground - base_y;
                if step_up > 0.0 && step_up <= 1.25 {
                    // Smoothly step up stairs, tavern porch, or threshold
                    pos.y = target_ground;
                    hm.air = 0.0;
                    hm.vy = 0.0;
                } else if step_up > 1.25 {
                    // Solid barrier/high ledge: do not pop upward
                    pos.y = base_y;
                } else {
                    let drop = base_y - target_ground;
                    if drop > 0.8 && playing {
                        // Walked off an elevated ledge / balcony: initiate gravity fall
                        hm.air = drop;
                        hm.vy = 0.0;
                        pos.y = target_ground + hm.air;
                    } else {
                        // Smoothly descend stairs or slope
                        pos.y = target_ground;
                        hm.air = 0.0;
                        hm.vy = 0.0;
                    }
                }
            }
            // Feed the animation driver: `vertical_speed`'s sign and the `FALLING` flag are what
            // select the Jump (37/38) / Fall (40) animation (`creature_anim::select`).
            mv.vertical_speed = hm.vy;
            if hm.vy != 0.0 {
                mv.flags |= move_flags::FALLING;
            }
            if tf.translation != pos {
                tf.translation = pos;
            }
            pos
        }
        Err(_) => {
            let mut p = arena(selection.stage_def());
            if let Some(h) = world.terrain_height_under(p) {
                p.y = h;
            }
            p
        }
    };
    // The terrain streams and the loading screen waits on the ground under this.
    player.active = true;
    player.detached = false;
    player.settling = false;
    player.world_stale = false;
    player.pos = focus;
}

/// Let go of the mouse buttons when the page loses focus or the pointer leaves it. A release that
/// lands elsewhere (alt-tab mid-drag, a browser or OS popup) never reaches the game, so the button
/// would read as held and the unlocked camera would swing with every move until both buttons were
/// clicked again. Survivors only: the engine's mouse-look keeps its buttons across a deactivate,
/// as 1.12.1 does.
pub(super) fn drop_stray_buttons(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    mut left: MessageReader<bevy::window::CursorLeft>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) {
    let lost = focus.read().any(|f| !f.focused);
    if lost | (left.read().count() > 0) {
        mouse.release_all();
    }
}

/// A slow orbit at the menu; in play, top-down until `C` unlocks it to swing and tilt.
pub(super) fn place_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut rig: ResMut<CamRig>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    player: Res<crate::player::Player>,
    world: WorldPoint,
    collide: WorldCollision,
    hero: Query<&Transform, (With<Hero>, Without<WorldCamera>)>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
    mut last_phase: Local<Phase>,
) {
    let p = *phase.get();
    let is_hub = p == Phase::Hub;

    // Detect phase transitions for camera presets
    if *last_phase == Phase::Hub && p == Phase::Playing {
        rig.mode = CamMode::Fixed;
        rig.pitch = FIXED_PITCH;
        rig.dist = 18.0;
        rig.yaw = 0.0;
    } else if *last_phase != Phase::Hub && is_hub {
        rig.mode = CamMode::Orbit;
        rig.pitch = HUB_PITCH;
        rig.dist = HUB_DIST;
        rig.yaw = 0.0;
    }
    *last_phase = p;

    // In play, `V` unlocks the camera to swing and tilt; locking it again snaps back to top-down.
    // In the Town Hub, orbit mode is active by default. `C` is reserved for Armory / Character Sheet.
    if keys.just_pressed(KeyCode::KeyV) && !is_hub {
        rig.mode = match rig.mode {
            CamMode::Fixed => CamMode::Orbit,
            CamMode::Orbit => {
                rig.yaw = 0.0;
                rig.pitch = FIXED_PITCH;
                CamMode::Fixed
            }
        };
    }
    // With no hero yet, the focus `drive_hero` grounded.
    let focus = hero.single().map_or(player.pos, |t| t.translation);
    // Multiplicative, so a notch moves as far by eye close in as far out; all the way down to an
    // over-the-shoulder view.
    let (min_zoom, max_zoom) = if is_hub { (2.5, 20.0) } else { (ZOOM_MIN, ZOOM_MAX) };
    for w in wheel.read() {
        let notches = match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y * 0.02,
        };
        rig.dist = (rig.dist * (1.0 - 0.12 * notches).clamp(0.5, 1.5)).clamp(min_zoom, max_zoom);
    }
    let dt = time.delta_secs();
    if rig.shake_t > 0.0 {
        rig.shake_t = (rig.shake_t - dt).max(0.0);
    }
    let (mut eye, mut look) = if p == Phase::Menu {
        rig.orbit += dt * 0.2;
        let d = 7.0;
        (
            focus + Vec3::new(rig.orbit.sin() * d, 2.4, rig.orbit.cos() * d),
            focus + Vec3::Y * 1.1,
        )
    } else {
        let (min_pitch, max_pitch) = if is_hub || rig.mode == CamMode::Orbit {
            (ORBIT_PITCH_MIN, PITCH_MAX)
        } else {
            (PITCH_MIN, PITCH_MAX)
        };

        // Unlocked or in Hub: mouse drag swings and tilts camera around the hero, as in WoW.
        if (rig.mode == CamMode::Orbit || is_hub)
            && mouse.any_pressed([MouseButton::Left, MouseButton::Right])
        {
            rig.yaw -= motion.delta.x * ORBIT_RATE;
            rig.pitch = (rig.pitch + motion.delta.y * ORBIT_RATE).clamp(min_pitch, max_pitch);
        }
        // Classic WoW keyboard tilt controls: PageUp looks up, PageDown looks down
        if keys.pressed(KeyCode::PageUp) {
            rig.pitch = (rig.pitch - 1.5 * dt).clamp(min_pitch, max_pitch);
        }
        if keys.pressed(KeyCode::PageDown) {
            rig.pitch = (rig.pitch + 1.5 * dt).clamp(min_pitch, max_pitch);
        }

        let (sy, cy) = rig.yaw.sin_cos();
        let (sp, cp) = rig.pitch.sin_cos();
        let look = if is_hub {
            focus + Vec3::Y * 1.6
        } else {
            focus + Vec3::Y * 1.4
        };

        let boom_dir = Vec3::new(cp * sy, sp, cp * cy).normalize();
        let desired_eye = look + boom_dir * rig.dist;
        let boom = desired_eye - look;
        let boom_len = boom.length();

        // Camera obstruction trace: sweep sphere along boom from look to desired_eye.
        // Prevents camera boom from penetrating tavern ceilings, roofs, or interior/exterior walls.
        let mut eye = if let Some(hit_dist) = collide.cast_camera(look, boom, false) {
            let safe_dist = (hit_dist - 0.15).max(0.1).min(boom_len);
            look + boom_dir * safe_dist
        } else {
            desired_eye
        };

        // Floor and ground clearance: ensure camera never dips beneath indoor floorboards or outdoor terrain
        if let Some(floor) = collide.ray_body(Vec3::new(eye.x, eye.y + 1.0, eye.z), Dir3::NEG_Y, 2.5).filter(|h| h.normal.y >= 0.45) {
            let floor_y = eye.y + 1.0 - floor.distance;
            eye.y = eye.y.max(floor_y + 0.35);
        } else if let Some(h) = world.terrain_height_under(eye) {
            eye.y = eye.y.max(h + 0.5);
        }

        (eye, look)
    };
    if rig.shake_t > 0.0 && rig.shake_strength > 0.0 {
        let f = (rig.shake_t / 0.4).min(1.0);
        let freq = time.elapsed_secs() * 45.0;
        let shake = Vec3::new(
            freq.sin() * rig.shake_strength * f * 0.35,
            (freq * 1.3).cos() * rig.shake_strength * f * 0.25,
            (freq * 0.7).sin() * rig.shake_strength * f * 0.35,
        );
        eye += shake;
        look += shake;
    }
    let placed = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    for mut cam in &mut cams {
        *cam = placed;
    }
}

/// Damage the hero takes from one blow, after immunity, armour and the shield; returns what got
/// through.
pub(super) fn hurt_hero(run: &mut Run, raw: f32) -> f32 {
    if run.immune_t > 0.0 || run.iframe_t > 0.0 || run.dead_t.is_some() {
        return 0.0;
    }
    let stats = run.stats();
    if stats.evasion > 0.0 {
        let roll = ((run.guid().wrapping_mul(1103515245).wrapping_add(12345) >> 16) & 0x7fff) as f32
            / 32768.0;
        if roll < stats.evasion {
            return 0.0;
        }
    }
    let mut dmg = (raw - stats.armor).max(1.0);
    if run.shield_t > 0.0 && run.shield > 0.0 {
        let absorbed = dmg.min(run.shield);
        run.shield -= absorbed;
        dmg -= absorbed;
    }
    run.hp -= dmg;
    dmg
}

/// The clock, regeneration, the timed buffs and their visuals, death and victory.
pub(super) fn hero_vitals(
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    mut hero: Query<(Entity, &Guid, &mut ObjectStore, &mut Marks), With<Hero>>,
    mut fields: MessageWriter<FieldChanged>,
) {
    let dt = time.delta_secs();
    let stats = run.stats();
    let Ok((entity, guid, mut store, mut marks)) = hero.single_mut() else {
        return;
    };
    if let Some(t) = run.dead_t.as_mut() {
        *t += dt;
        if *t > 3.0 {
            next.set(Phase::GameOver);
        }
        return;
    }
    let clock_dt = if run.affixes.turbo_blitz { dt * 1.5 } else { dt };
    run.elapsed += clock_dt;
    run.dash_cd = (run.dash_cd - clock_dt).max(0.0);
    run.iframe_t = (run.iframe_t - dt).max(0.0);
    let r = &mut *run;
    for t in [&mut r.shield_t, &mut r.immune_t, &mut r.thorns_t] {
        *t = (*t - dt).max(0.0);
    }
    let mut heal = stats.regen * dt;
    if run.heal_t > 0.0 {
        heal += run.heal_rate * dt.min(run.heal_t);
        run.heal_t = (run.heal_t - dt).max(0.0);
    }
    run.hp = (run.hp + heal).min(stats.max_hp);

    // The buffs show as their own state kits: the shield bubble, the immunity glow, the orbs.
    let mut shown: Vec<u32> = Vec::new();
    if run.shield_t > 0.0 && run.shield > 0.0 {
        shown.push(17);
    }
    if run.immune_t > 0.0 {
        shown.push(642);
    }
    if run.thorns_t > 0.0 {
        shown.push(run.thorns_spell);
    }
    if run.heal_t > 0.0 {
        shown.push(774);
    }
    for owned in &run.abilities {
        if owned.def.key == "lightning_shield" {
            shown.push(owned.def.spell);
        }
    }
    if run.passives.iter().any(|(p, _)| p.key == "renew") {
        shown.push(139);
    }
    marks.set(&shown);
    marks.flush(entity, guid.0, &mut store, &mut fields);

    if run.hp <= 0.0 {
        run.hearth_channel = None;
        run.hp = 0.0;
        run.dead_t = Some(0.0);
        units::kill_unit(&mut store);
        marks.clear();
        marks.flush(entity, guid.0, &mut store, &mut fields);
    } else if let Some(h) = run.hearth_channel.as_mut() {
        *h -= dt;
        if *h <= 0.0 {
            run.hearth_channel = None;
            run.won = true;
            next.set(Phase::GameOver);
        }
    } else if run.elapsed >= data::VICTORY_SECS && !run.overtime && !run.won {
        next.set(Phase::PortalChoice);
    }
}

/// The ground-plane distance from the hero, the measure every range in the mode uses.
/// The movement flags for running `dir` while facing `look`: forward, backpedal or strafe, by
/// which quarter the run falls in, so the walk, backpedal and strafe animations play.
fn gait(look: Vec3, dir: Vec3) -> u32 {
    let ahead = look.dot(dir);
    // Positive when `dir` is to the hero's left: WoW faces −Z with +X on the right.
    let left = look.cross(dir).y;
    let quarter = std::f32::consts::FRAC_1_SQRT_2;
    if ahead >= quarter {
        move_flags::FORWARD
    } else if ahead <= -quarter {
        move_flags::BACKWARD
    } else if left > 0.0 {
        move_flags::STRAFE_LEFT
    } else {
        move_flags::STRAFE_RIGHT
    }
}

pub(super) fn reach(hero: Vec3, other: Vec3) -> f32 {
    flat(other - hero).length()
}

/// Tactical range rings configuration.
#[derive(Resource)]
pub struct RangeRingSettings {
    pub enabled: bool,
    pub opacity: f32,
    pub ring_thickness: f32,
}

impl Default for RangeRingSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            opacity: 0.45,
            ring_thickness: 0.18,
        }
    }
}

/// Tag component for spawned range ring visual entities.
#[derive(Component)]
pub(super) struct RangeRingMarker;

/// Calculates dynamic inner melee reach, AoE radii, and outer range from owned abilities.
pub(super) fn calculate_hero_ranges(run: &Run) -> (f32, Vec<f32>, f32) {
    let mut min_melee = 5.5;
    let mut aoe_radii = Vec::new();
    let mut max_range = 5.5;
    let stats = run.stats();

    for owned in &run.abilities {
        let def = owned.def;
        if def.range > 0.0 {
            if def.range <= 8.0 && (def.range < min_melee || min_melee == 5.5) {
                min_melee = def.range;
            }
            if def.range > max_range {
                max_range = def.range;
            }
        }
        if def.radius > 0.0 {
            let r = owned.radius(&stats);
            if !aoe_radii.iter().any(|&existing: &f32| (existing - r).abs() < 0.1) {
                aoe_radii.push(r);
            }
        }
    }
    (min_melee, aoe_radii, max_range)
}

/// Builds an annulus (flat ground ring) mesh with the given radius and radial thickness.
pub(super) fn ring_mesh(radius: f32, thickness: f32) -> Mesh {
    const SEGMENTS: usize = 48;
    let r_inner = (radius - thickness * 0.5).max(0.1);
    let r_outer = radius + thickness * 0.5;
    let mut positions = Vec::with_capacity(SEGMENTS * 2);
    let mut normals = Vec::with_capacity(SEGMENTS * 2);
    let mut uvs = Vec::with_capacity(SEGMENTS * 2);
    let mut indices = Vec::with_capacity(SEGMENTS * 6);

    for i in 0..SEGMENTS {
        let theta = (i as f32) / (SEGMENTS as f32) * std::f32::consts::TAU;
        let sin = theta.sin();
        let cos = theta.cos();
        positions.push([r_inner * sin, 0.0, -r_inner * cos]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([0.0, (i as f32) / (SEGMENTS as f32)]);

        positions.push([r_outer * sin, 0.0, -r_outer * cos]);
        normals.push([0.0, 1.0, 0.0]);
        uvs.push([1.0, (i as f32) / (SEGMENTS as f32)]);
    }

    for i in 0..SEGMENTS {
        let next = (i + 1) % SEGMENTS;
        let i0 = (i * 2) as u32;
        let i1 = (i * 2 + 1) as u32;
        let n0 = (next * 2) as u32;
        let n1 = (next * 2 + 1) as u32;
        indices.extend_from_slice(&[i0, n0, i1]);
        indices.extend_from_slice(&[i1, n0, n1]);
    }

    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    mesh
}

/// Spawns or repositions ground range ring decals around the active hero.
pub(super) fn update_range_rings(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut settings: ResMut<RangeRingSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    hero: Query<&Transform, With<Hero>>,
    mut rings: Query<(Entity, &mut Transform), (With<RangeRingMarker>, Without<Hero>)>,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    mut last_version: Local<Option<u32>>,
) {
    if keys.just_pressed(KeyCode::KeyO) {
        settings.enabled = !settings.enabled;
    }

    if *phase.get() != Phase::Playing || !settings.enabled || run.dead_t.is_some() {
        for (e, _) in &rings {
            commands.entity(e).despawn();
        }
        *last_version = None;
        return;
    }

    let Ok(hero_tf) = hero.single() else {
        return;
    };

    if *last_version != Some(run.version) || rings.iter().count() == 0 {
        *last_version = Some(run.version);
        for (e, _) in &rings {
            commands.entity(e).despawn();
        }

        let (melee_range, aoe_ranges, outer_range) = calculate_hero_ranges(&run);

        let melee_color = Color::srgba(1.0, 0.8, 0.2, settings.opacity);
        let aoe_color = Color::srgba(0.2, 0.8, 1.0, settings.opacity);
        let outer_color = Color::srgba(0.8, 0.3, 0.9, settings.opacity * 0.7);

        let primary_rings = [
            (melee_range, melee_color),
            (outer_range, outer_color),
        ];

        for (r, col) in primary_rings {
            let mesh = meshes.add(ring_mesh(r, settings.ring_thickness));
            let mat = materials.add(StandardMaterial {
                base_color: col,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                depth_bias: 40.0,
                ..default()
            });
            commands.spawn((
                RangeRingMarker,
                super::RunEntity,
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_translation(hero_tf.translation + Vec3::Y * 0.05),
            ));
        }

        for &aoe_r in &aoe_ranges {
            if (aoe_r - melee_range).abs() > 0.5 && (aoe_r - outer_range).abs() > 0.5 {
                let mesh = meshes.add(ring_mesh(aoe_r, settings.ring_thickness));
                let mat = materials.add(StandardMaterial {
                    base_color: aoe_color,
                    alpha_mode: AlphaMode::Blend,
                    unlit: true,
                    cull_mode: None,
                    depth_bias: 40.0,
                    ..default()
                });
                commands.spawn((
                    RangeRingMarker,
                    super::RunEntity,
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_translation(hero_tf.translation + Vec3::Y * 0.05),
                ));
            }
        }
    } else {
        let ring_pos = hero_tf.translation + Vec3::Y * 0.05;
        for (_, mut rtf) in &mut rings {
            rtf.translation = ring_pos;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::window::{CursorLeft, WindowFocused};

    fn held_left() -> World {
        let mut world = World::new();
        world.init_resource::<Messages<WindowFocused>>();
        world.init_resource::<Messages<CursorLeft>>();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        world.insert_resource(mouse);
        world
    }

    fn left_held(world: &World) -> bool {
        world.resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left)
    }

    #[test]
    fn a_held_button_is_dropped_when_the_page_loses_focus() {
        let mut world = held_left();
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(left_held(&world), "nothing happened: the drag goes on");
        world.write_message(WindowFocused { window: Entity::PLACEHOLDER, focused: true });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(left_held(&world), "gaining focus is no release");
        world.write_message(WindowFocused { window: Entity::PLACEHOLDER, focused: false });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(!left_held(&world));
    }

    #[test]
    fn a_held_button_is_dropped_when_the_pointer_leaves_the_page() {
        let mut world = held_left();
        world.write_message(CursorLeft { window: Entity::PLACEHOLDER });
        world.run_system_once(drop_stray_buttons).unwrap();
        assert!(!left_held(&world));
    }

    #[test]
    fn test_step_up_and_descent_thresholds() {
        // Step up <= 1.25 yd steps onto the stair/porch
        let base_y = 16.5;
        let step_stair = 16.8;
        let step_up = step_stair - base_y;
        assert!(step_up > 0.0 && step_up <= 1.25);

        // Porch edge ~1.0 yd steps up cleanly
        let porch_y = 17.5;
        let step_porch = porch_y - base_y;
        assert!(step_porch > 0.0 && step_porch <= 1.25);

        // High wall / roof (> 1.25 yd) does not pop up
        let wall_y = 18.5;
        let step_wall = wall_y - base_y;
        assert!(step_wall > 1.25);

        // Moderate step down (drop <= 0.8 yd) smoothly descends
        let lower_step = 16.2;
        let drop_step = base_y - lower_step;
        assert!(drop_step <= 0.8);

        // Balcony / ledge fall (drop > 0.8 yd) triggers gravity fall
        let ground_below = 12.0;
        let drop_balcony = base_y - ground_below;
        assert!(drop_balcony > 0.8);
    }

    #[test]
    fn test_wall_normal_horizontal_flattening() {
        // Walkable surface has normal.y >= 0.45 and should be ignored for horizontal slide
        let floor_normal = Vec3::new(0.1, 0.9, 0.0).normalize();
        assert!(floor_normal.y >= 0.45);

        // Vertical wall has normal.y < 0.45 and should be flattened to XZ
        let wall_normal = Vec3::new(0.8, 0.2, 0.5).normalize();
        assert!(wall_normal.y < 0.45);
        let n_h = Vec3::new(wall_normal.x, 0.0, wall_normal.z).normalize_or_zero();
        assert_eq!(n_h.y, 0.0);
        assert!((n_h.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_camera_obstruction_safe_distance() {
        let boom_len: f32 = 12.0;
        let hit_dist_indoor: f32 = 2.2; // ceiling or wall 2.2 yd away
        let safe_dist = (hit_dist_indoor - 0.15).max(0.1).min(boom_len);
        assert!((safe_dist - 2.05).abs() < 1e-5);
        assert!(safe_dist < hit_dist_indoor);

        // Near-obstacle distance does not punch past geometry:
        let hit_dist_close: f32 = 0.25;
        let safe_close = (hit_dist_close - 0.15).max(0.1).min(boom_len);
        assert!(safe_close < hit_dist_close);
        assert!(safe_close >= 0.1);
    }

    #[test]
    fn test_orbit_pitch_range_allows_looking_up() {
        // Orbit pitch minimum is negative so player can tilt upward to view stairs, loft, and ceilings
        assert!(ORBIT_PITCH_MIN < 0.0);
        // At negative pitch, boom_dir has negative Y, so eye is below look,
        // which makes the camera aim upward toward look and beyond
        let pitch = -0.5f32;
        let (sp, cp) = pitch.sin_cos();
        assert!(sp < 0.0);
        assert!(cp > 0.0);
        let look = Vec3::new(0.0, 18.0, 0.0);
        let boom_dir = Vec3::new(0.0, sp, cp).normalize();
        let eye = look + boom_dir * 4.0;
        assert!(eye.y < look.y); // Eye is lower than look, aiming UP
    }

    #[test]
    fn test_stair_step_lift_ahead() {
        let base_y = 16.5;
        let stair_ground = 16.75;
        let rise = stair_ground - base_y;
        let step_lift = if rise > 0.0 && rise <= 1.25 { rise } else { 0.0 };
        assert_eq!(step_lift, 0.25);

        // Flat ground gives 0 lift
        let flat_ground = 16.5;
        let flat_rise = flat_ground - base_y;
        let flat_lift = if flat_rise > 0.0 && flat_rise <= 1.25 { flat_rise } else { 0.0 };
        assert_eq!(flat_lift, 0.0);

        // High wall (> 1.25 yd) gives 0 lift
        let wall_ground = 18.5;
        let wall_rise = wall_ground - base_y;
        let wall_lift = if wall_rise > 0.0 && wall_rise <= 1.25 { wall_rise } else { 0.0 };
        assert_eq!(wall_lift, 0.0);
    }
}
