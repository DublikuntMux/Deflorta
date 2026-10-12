//! Single-finger gesture recognition, with native controls reserving input.

use std::time::{Duration, Instant};

const LONG_PRESS_DELAY: Duration = Duration::from_millis(500);
const TOUCH_TOLERANCE: f64 = 12.0;
const SWIPE_DISTANCE: f64 = 60.0;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TouchAction {
    None,
    Tap,
    Rollback,
}

pub(super) struct TouchState {
    pub id: u64,
    start: (f64, f64),
    last: (f64, f64),
    delta: (f64, f64),
    pub long_press: Option<Instant>,
    pub native: bool,
    pub scroll: bool,
    pub consumed: bool,
}

impl TouchState {
    pub fn new(id: u64, position: (f64, f64), native: bool, scroll: bool, now: Instant) -> Self {
        Self {
            id,
            start: position,
            last: position,
            delta: (0.0, 0.0),
            long_press: (!native && !scroll).then_some(now + LONG_PRESS_DELAY),
            native,
            scroll,
            consumed: false,
        }
    }

    /// Updates recognition and returns native scroll movement in wheel lines.
    pub fn move_to(&mut self, position: (f64, f64), scale: f64) -> f64 {
        self.delta = (
            (position.0 - self.start.0) / scale,
            (position.1 - self.start.1) / scale,
        );
        if self.delta.0.hypot(self.delta.1) > TOUCH_TOLERANCE {
            self.long_press = None;
            if self.scroll && !self.native {
                self.consumed = true;
            }
        }
        let scroll_delta = (self.last.1 - position.1) / scale / 40.0;
        self.last = position;
        scroll_delta
    }

    pub fn take_long_press(&mut self, now: Instant) -> bool {
        if self.long_press.is_some_and(|due| due <= now) {
            self.long_press = None;
            self.consumed = true;
            true
        } else {
            false
        }
    }

    pub fn finish(&self, cancelled: bool) -> TouchAction {
        if cancelled || self.native || self.consumed {
            return TouchAction::None;
        }
        let (dx, dy) = self.delta;
        if !self.scroll && dy <= -SWIPE_DISTANCE && dy.abs() > dx.abs() * 1.5 {
            TouchAction::Rollback
        } else if dx.hypot(dy) <= TOUCH_TOLERANCE {
            TouchAction::Tap
        } else {
            TouchAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_press_has_one_deadline_and_never_also_taps() {
        let now = Instant::now();
        let mut touch = TouchState::new(1, (100.0, 200.0), false, false, now);
        assert_eq!(touch.long_press, Some(now + LONG_PRESS_DELAY));
        let before_long_press = (now + LONG_PRESS_DELAY)
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert!(!touch.take_long_press(before_long_press));
        assert!(touch.take_long_press(now + LONG_PRESS_DELAY));
        assert!(!touch.take_long_press(now + Duration::from_secs(1)));
        assert_eq!(touch.long_press, None);
        assert_eq!(touch.finish(false), TouchAction::None);
    }

    #[test]
    fn movement_cancels_long_press_and_swipes_use_logical_pixels() {
        let now = Instant::now();
        let mut touch = TouchState::new(1, (100.0, 200.0), false, false, now);
        touch.move_to((100.0, 174.0), 2.0);
        assert_eq!(touch.long_press, None);
        assert!(!touch.take_long_press(now + Duration::from_secs(1)));
        assert_eq!(touch.finish(false), TouchAction::None);
        touch.move_to((100.0, 80.0), 2.0);
        assert_eq!(touch.finish(false), TouchAction::Rollback);
        assert_eq!(touch.finish(true), TouchAction::None);
        touch.move_to((190.0, 80.0), 2.0);
        assert_eq!(touch.finish(false), TouchAction::None);
    }

    #[test]
    fn native_controls_and_scroll_containers_reserve_gestures() {
        let now = Instant::now();
        for (native, scroll) in [(true, false), (true, true), (false, true)] {
            let mut touch = TouchState::new(1, (0.0, 200.0), native, scroll, now);
            assert_eq!(touch.long_press, None);
            touch.move_to((0.0, 100.0), 1.0);
            assert_eq!(touch.finish(false), TouchAction::None);
        }
        let mut scroll = TouchState::new(1, (0.0, 200.0), false, true, now);
        assert_eq!(scroll.finish(false), TouchAction::Tap);
        assert_eq!(scroll.move_to((0.0, 160.0), 1.0), 1.0);
        assert!(scroll.consumed);
        scroll.move_to((0.0, 200.0), 1.0);
        assert_eq!(scroll.finish(false), TouchAction::None);
    }

    #[test]
    fn taps_allow_small_movement_and_cancellation_never_clicks() {
        let now = Instant::now();
        let mut touch = TouchState::new(1, (0.0, 0.0), false, false, now);
        touch.move_to((6.0, 6.0), 1.0);
        assert_eq!(touch.finish(false), TouchAction::Tap);
        assert_eq!(touch.finish(true), TouchAction::None);
    }
}
