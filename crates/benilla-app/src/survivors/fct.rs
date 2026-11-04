//! Classic WoW Floating Combat Text (FCT) for Warcraft Survivors.
//!
//! Spawns scrolling numbers for physical and elemental spell damage, crits, and heals,
//! floating upward in world space and projected to billboarded screen space.

use std::sync::atomic::{AtomicU32, Ordering};

use benilla_world::view::WorldCamera;
use bevy::prelude::*;

use super::hud::HudFont;
use super::Phase;
use super::RunEntity;

/// Combat text categories matching Classic WoW schools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CombatTextKind {
    #[default]
    Physical,
    Fire,
    Frost,
    Nature,
    Holy,
    Shadow,
    Heal,
}

impl CombatTextKind {
    /// The canonical Classic WoW floating text color for this school.
    pub fn color(&self) -> Color {
        match self {
            Self::Physical => Color::WHITE,
            Self::Fire => Color::srgb(1.0, 0.45, 0.05),     // Orange
            Self::Frost => Color::srgb(0.25, 0.85, 1.0),    // Cyan
            Self::Nature => Color::srgb(0.25, 0.95, 0.25),  // Green
            Self::Holy => Color::srgb(1.0, 0.84, 0.0),      // Gold
            Self::Shadow => Color::srgb(0.75, 0.35, 0.95),  // Violet
            Self::Heal => Color::srgb(0.2, 1.0, 0.2),       // Bright Green
        }
    }

    /// Classify an ability key or spell name into its combat text school.
    pub fn from_key(key: &str) -> Self {
        let lower = key.to_ascii_lowercase();
        if lower.contains("fire")
            || lower.contains("flame")
            || lower.contains("pyro")
            || lower.contains("hellfire")
            || lower.contains("magma")
            || lower.contains("immolate")
            || lower.contains("searing")
            || lower.contains("blast_wave")
        {
            Self::Fire
        } else if lower.contains("frost")
            || lower.contains("blizzard")
            || lower.contains("cold")
            || lower.contains("ice")
        {
            Self::Frost
        } else if lower.contains("holy")
            || lower.contains("smite")
            || lower.contains("exorcism")
            || lower.contains("judgement")
            || lower.contains("consecrat")
            || lower.contains("wrath")
            || lower.contains("arcane")
            || lower.contains("starfire")
            || lower.contains("moonfire")
        {
            Self::Holy
        } else if lower.contains("shadow")
            || lower.contains("corruption")
            || lower.contains("agony")
            || lower.contains("death_coil")
            || lower.contains("mind_blast")
            || lower.contains("scream")
        {
            Self::Shadow
        } else if lower.contains("heal") || lower.contains("rejuvenat") {
            Self::Heal
        } else if lower.contains("nature")
            || lower.contains("lightning")
            || lower.contains("thunder")
            || lower.contains("earth")
            || lower.contains("thorns")
            || lower.contains("roots")
            || lower.contains("swarm")
            || lower.contains("hurricane")
            || lower.contains("serpent")
            || lower.contains("sting")
        {
            Self::Nature
        } else {
            Self::Physical
        }
    }
}

/// Message requesting a scrolling combat text number at a world position.
#[derive(Message, Event, Clone, Debug)]
pub struct CombatTextMsg {
    pub pos: Vec3,
    pub amount: f32,
    pub is_crit: bool,
    pub kind: CombatTextKind,
}

/// Component tracking an active floating combat text number.
#[derive(Component, Debug, Clone)]
pub struct FloatingText {
    pub pos: Vec3,
    pub lifetime: f32,
    pub base_color: Color,
    pub is_crit: bool,
    pub elapsed: f32,
}

static JITTER_SEED: AtomicU32 = AtomicU32::new(1234567);
fn jitter() -> f32 {
    let s = JITTER_SEED.fetch_add(1103515245, Ordering::Relaxed);
    let r = ((s >> 16) & 0x7fff) as f32 / 32768.0;
    (r - 0.5) * 0.4
}

/// Spawns 3D/billboard text entities for incoming `CombatTextMsg`s.
pub(super) fn spawn_combat_text(
    mut commands: Commands,
    mut reader: MessageReader<CombatTextMsg>,
    font_res: Option<Res<HudFont>>,
    asset_server: Res<AssetServer>,
) {
    let font = font_res
        .map(|f| f.0.clone())
        .unwrap_or_else(|| crate::char_select::wow_font(&asset_server));

    for msg in reader.read() {
        if msg.amount <= 0.0 {
            continue;
        }

        let (text_str, font_size, color) = if msg.is_crit {
            (
                format!("{:.0}!", msg.amount.round().max(1.0)),
                18.0,
                Color::srgb(1.0, 0.88, 0.0), // bright Yellow/Gold with exclamation mark
            )
        } else if msg.kind == CombatTextKind::Heal {
            (
                format!("+{:.0}", msg.amount.round().max(1.0)),
                13.0,
                Color::srgb(0.2, 1.0, 0.2), // bright Green with plus prefix
            )
        } else {
            (
                format!("{:.0}", msg.amount.round().max(1.0)),
                12.0,
                msg.kind.color(), // elemental color
            )
        };

        let jx = jitter();
        let jz = jitter();
        let start_pos = msg.pos + Vec3::new(jx, 1.3, jz);

        commands.spawn((
            RunEntity,
            FloatingText {
                pos: start_pos,
                lifetime: 0.8,
                base_color: color,
                is_crit: msg.is_crit,
                elapsed: 0.0,
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
            Text::new(text_str),
            TextFont {
                font: font.clone(),
                font_size,
                ..default()
            },
            TextColor(color),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::srgba(0.0, 0.0, 0.0, 0.9),
            },
            GlobalZIndex(350),
            Visibility::Hidden,
        ));
    }
}

/// Ticks lifetime, floats upward, fades alpha, and projects world pos to screen billboard.
pub(super) fn update_combat_text(
    mut commands: Commands,
    time: Res<Time>,
    ui_scale: Option<Res<UiScale>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    mut query: Query<(
        Entity,
        &mut FloatingText,
        &mut Node,
        &mut TextColor,
        Option<&mut TextShadow>,
        &mut Visibility,
    )>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    let cam_opt = cams.iter().next();
    let s = ui_scale.map(|s| s.0).unwrap_or(1.0);

    for (entity, mut item, mut node, mut text_color, shadow, mut vis) in &mut query {
        item.lifetime -= dt;
        item.elapsed += dt;

        if item.lifetime <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // Float upward: translation.y += dt * 1.8;
        // Crits get an initial fast upward bounce decaying over 0.3s.
        let vy = if item.is_crit {
            let bounce = (1.0 - (item.elapsed / 0.3).min(1.0)) * 3.5;
            1.8 + bounce
        } else {
            1.8
        };

        item.pos.y += dt * vy;

        // Fades out alpha towards 0.0 over the last 0.3s.
        let alpha = if item.lifetime < 0.3 {
            (item.lifetime / 0.3).clamp(0.0, 1.0)
        } else {
            1.0
        };

        text_color.0 = item.base_color.with_alpha(alpha);
        if let Some(mut sh) = shadow {
            sh.color = Color::srgba(0.0, 0.0, 0.0, alpha * 0.9);
        }

        // Project world pos to screen viewport coordinates.
        if let Some((cam, cam_tf)) = cam_opt {
            if let Ok(screen) = cam.world_to_viewport(&GlobalTransform::from(*cam_tf), item.pos) {
                node.left = Val::Px(screen.x / s - 20.0);
                node.top = Val::Px(screen.y / s - 10.0);
                *vis = Visibility::Inherited;
            } else {
                *vis = Visibility::Hidden;
            }
        }
    }
}

pub struct FctPlugin;

impl Plugin for FctPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<CombatTextMsg>()
            .add_systems(
                Update,
                (spawn_combat_text, update_combat_text)
                    .chain()
                    .after(super::abilities::apply_hits)
                    .run_if(in_state(Phase::Playing)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn combat_text_kind_colors() {
        assert_eq!(CombatTextKind::Physical.color(), Color::WHITE);
        assert_eq!(CombatTextKind::Heal.color(), Color::srgb(0.2, 1.0, 0.2));
        assert_eq!(CombatTextKind::Fire.color(), Color::srgb(1.0, 0.45, 0.05));
        assert_eq!(CombatTextKind::Frost.color(), Color::srgb(0.25, 0.85, 1.0));
        assert_eq!(CombatTextKind::Nature.color(), Color::srgb(0.25, 0.95, 0.25));
        assert_eq!(CombatTextKind::Holy.color(), Color::srgb(1.0, 0.84, 0.0));
        assert_eq!(CombatTextKind::Shadow.color(), Color::srgb(0.75, 0.35, 0.95));
    }

    #[test]
    fn combat_text_school_detection() {
        assert_eq!(CombatTextKind::from_key("fireball"), CombatTextKind::Fire);
        assert_eq!(CombatTextKind::from_key("pyroblast"), CombatTextKind::Fire);
        assert_eq!(CombatTextKind::from_key("frostbolt"), CombatTextKind::Frost);
        assert_eq!(CombatTextKind::from_key("lightning_bolt"), CombatTextKind::Nature);
        assert_eq!(CombatTextKind::from_key("smite"), CombatTextKind::Holy);
        assert_eq!(CombatTextKind::from_key("shadow_bolt"), CombatTextKind::Shadow);
        assert_eq!(CombatTextKind::from_key("heal"), CombatTextKind::Heal);
        assert_eq!(CombatTextKind::from_key("cleave"), CombatTextKind::Physical);
    }

    #[test]
    fn combat_text_float_and_fade() {
        let mut app = App::new();
        app.init_resource::<Time>();

        let entity = app
            .world_mut()
            .spawn((
                FloatingText {
                    pos: Vec3::new(10.0, 20.0, 30.0),
                    lifetime: 0.8,
                    base_color: Color::WHITE,
                    is_crit: false,
                    elapsed: 0.0,
                },
                Node::default(),
                TextColor(Color::WHITE),
                Visibility::Hidden,
            ))
            .id();

        // Advance 0.5s -> lifetime becomes 0.3, alpha still 1.0
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_millis(500));
        app.world_mut().run_system_once(update_combat_text).unwrap();

        let ft = app.world().get::<FloatingText>(entity).unwrap();
        assert!((ft.lifetime - 0.3).abs() < 1e-4);
        assert!((ft.pos.y - (20.0 + 0.5 * 1.8)).abs() < 1e-4);
        let tc = app.world().get::<TextColor>(entity).unwrap();
        assert!((tc.0.alpha() - 1.0).abs() < 1e-4);

        // Advance another 0.15s -> lifetime becomes 0.15, alpha becomes 0.5
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_millis(150));
        app.world_mut().run_system_once(update_combat_text).unwrap();

        let tc = app.world().get::<TextColor>(entity).unwrap();
        assert!((tc.0.alpha() - 0.5).abs() < 1e-2);

        // Advance another 0.2s -> lifetime <= 0.0, entity despawns
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_millis(200));
        app.world_mut().run_system_once(update_combat_text).unwrap();

        assert!(app.world().get_entity(entity).is_err());
    }
}
