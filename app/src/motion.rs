//! The small animations: covers pulse while they load and fade in when they
//! arrive. Pure functions of elapsed time, so the view stays a function of
//! state plus "now".

use std::time::Duration;

/// One breath of the loading pulse.
pub const PULSE_PERIOD: Duration = Duration::from_millis(1200);
/// How long a cover takes to fade in.
pub const FADE_IN: Duration = Duration::from_millis(250);

/// Where the pulse is, 0 → 1 → 0 over [`PULSE_PERIOD`], smoothly, repeating.
pub fn pulse(elapsed: Duration) -> f32 {
    let turn = elapsed.as_secs_f32() / PULSE_PERIOD.as_secs_f32();
    (1.0 - (turn * std::f32::consts::TAU).cos()) / 2.0
}

/// A cover's opacity `elapsed` after it arrived: 0 → 1 over [`FADE_IN`].
pub fn fade_in(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f32() / FADE_IN.as_secs_f32()).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn the_pulse_breathes_out_and_back_every_period() {
        assert!(close(pulse(ms(0)), 0.0));
        assert!(close(pulse(ms(300)), 0.5));
        assert!(close(pulse(ms(600)), 1.0));
        assert!(close(pulse(ms(1200)), 0.0));
        assert!(close(pulse(ms(1800)), 1.0), "and again");
    }

    #[test]
    fn a_cover_fades_in_then_stays() {
        assert!(close(fade_in(ms(0)), 0.0));
        assert!(close(fade_in(ms(125)), 0.5));
        assert!(close(fade_in(ms(250)), 1.0));
        assert!(close(fade_in(ms(10_000)), 1.0));
    }
}
