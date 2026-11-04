//! Combo Momentum System & Arcade "Killing Spree" Multipliers.
//!
//! Rapidly eliminating enemies builds a momentum kill streak:
//! - 25 kills: Killing Spree!
//! - 50 kills: Rampage!
//! - 100 kills: Dominating!
//! - 200 kills: Unstoppable!
//! - 350+ kills: GODLIKE!
//!
//! Maintaining the streak grants scaling movement speed and XP multipliers.
//! When the 3.5s decay timer runs out, streaks of 15+ award a gold cashout payout!

use bevy::prelude::*;

use crate::sound::GlueSound;
use super::units::Fx;
use super::Run;

pub const COMBO_MAX_TIMER: f32 = 3.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComboState {
    pub count: u32,
    pub timer: f32,
    pub max_combo: u32,
    pub last_announced_tier: u32,
}

pub fn tier_for_count(count: u32) -> Option<(u32, &'static str)> {
    match count {
        n if n >= 350 => Some((350, "★ GODLIKE ★")),
        n if n >= 200 => Some((200, "UNSTOPPABLE!")),
        n if n >= 100 => Some((100, "DOMINATING!")),
        n if n >= 50 => Some((50, "RAMPAGE!")),
        n if n >= 25 => Some((25, "KILLING SPREE!")),
        _ => None,
    }
}

/// Records an enemy defeat toward the active combo streak.
pub fn record_kill(run: &mut Run, sounds: &mut MessageWriter<GlueSound>) {
    run.combo.count += 1;
    run.combo.timer = COMBO_MAX_TIMER;
    if run.combo.count > run.combo.max_combo {
        run.combo.max_combo = run.combo.count;
    }

    if let Some((threshold, title)) = tier_for_count(run.combo.count) {
        if run.combo.last_announced_tier < threshold {
            run.combo.last_announced_tier = threshold;
            run.banner = Some((format!("{} ({} Streak!)", title, run.combo.count), 2.5));
            sounds.write(GlueSound("igPVPUpdate"));
        }
    }
}

/// Ticks active combo timer, handles expiration and cashout rewards.
pub(super) fn tick_combo(
    mut run: ResMut<Run>,
    time: Res<Time>,
    mut sounds: MessageWriter<GlueSound>,
    hero: Query<Entity, With<super::hero::Hero>>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || run.combo.count == 0 || run.dead_t.is_some() {
        return;
    }

    run.combo.timer -= dt;
    if run.combo.timer <= 0.0 {
        let count = run.combo.count;
        run.combo.count = 0;
        run.combo.timer = 0.0;
        run.combo.last_announced_tier = 0;

        // Cashout payout for combos of 15 or higher
        if count >= 15 {
            let cashout = (count as u64) * 2;
            run.gold += cashout;
            sounds.write(GlueSound("LOOTWINDOWCOINSOUND"));
            if let Ok(hero_e) = hero.single() {
                fx.number(hero_e, cashout as f32, false, 0xFFFF_D700);
            }
            run.banner = Some((format!("Streak Ended! +{}g Combo Cashout!", cashout), 2.5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combo_tiers() {
        assert_eq!(tier_for_count(10), None);
        assert_eq!(tier_for_count(25), Some((25, "KILLING SPREE!")));
        assert_eq!(tier_for_count(75), Some((50, "RAMPAGE!")));
        assert_eq!(tier_for_count(150), Some((100, "DOMINATING!")));
        assert_eq!(tier_for_count(250), Some((200, "UNSTOPPABLE!")));
        assert_eq!(tier_for_count(400), Some((350, "★ GODLIKE ★")));
    }

    #[test]
    fn test_combo_speed_and_xp_scaling() {
        let count = 50;
        let speed_bonus = (count as f32 * 0.0015).min(0.40);
        let xp_bonus = (count as f32 * 0.003).min(0.75);
        assert!((speed_bonus - 0.075).abs() < 1e-4);
        assert!((xp_bonus - 0.15).abs() < 1e-4);
    }
}
