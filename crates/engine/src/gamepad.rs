//! Desktop gamepad events and direction repeat, independent of window redraws.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType, Gilrs};
use log::{info, warn};
use winit::event_loop::EventLoopProxy;

use super::AppEvent;

const STICK_THRESHOLD: f32 = 0.6;
const REPEAT_DELAY: Duration = Duration::from_millis(400);
const REPEAT_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Default)]
struct HeldKeys {
    keys: HashMap<(usize, &'static str), Option<Instant>>,
}

impl HeldKeys {
    fn set(&mut self, id: usize, key: &'static str, down: bool, now: Instant) -> Option<AppEvent> {
        let was_down = self.keys.contains_key(&(id, key));
        if was_down == down {
            return None;
        }
        let was_held = self.keys.keys().any(|(_, held)| *held == key);
        if down {
            let repeat = (key.starts_with("GamepadDPad") || key.starts_with("GamepadLeftStick"))
                .then_some(now + REPEAT_DELAY);
            self.keys.insert((id, key), repeat);
        } else {
            self.keys.remove(&(id, key));
        }
        let is_held = self.keys.keys().any(|(_, held)| *held == key);
        (was_held != is_held).then_some(AppEvent::Gamepad {
            key,
            down: is_held,
            repeat: false,
        })
    }

    fn deadline(&self) -> Option<Instant> {
        self.keys.values().copied().flatten().min()
    }

    fn repeats(&mut self, now: Instant) -> Vec<AppEvent> {
        let mut keys = Vec::new();
        for ((_, key), due) in &mut self.keys {
            if let Some(deadline) = due
                && *deadline <= now
            {
                *deadline = now + REPEAT_INTERVAL;
                if !keys.contains(key) {
                    keys.push(*key);
                }
            }
        }
        keys.into_iter()
            .map(|key| AppEvent::Gamepad {
                key,
                down: true,
                repeat: true,
            })
            .collect()
    }
}

pub(super) fn start(proxy: EventLoopProxy<AppEvent>) {
    // Gilrs has no Android backend. Android's keys arrive through winit instead.
    // The worker lives for the event loop/process lifetime; no idle polling is
    // necessary, including when no controllers are connected.
    if let Err(error) = std::thread::Builder::new()
        .name("deflorta-gamepad".into())
        .spawn(move || run(&proxy))
    {
        warn!("Cannot start gamepad worker: {error}");
    }
}

fn run(proxy: &EventLoopProxy<AppEvent>) {
    let mut gamepads = match Gilrs::new() {
        Ok(gamepads) => gamepads,
        Err(error) => {
            warn!("Gamepads unavailable: {error}");
            return;
        }
    };
    info!(
        "Gamepad support ready, {} connected",
        gamepads.gamepads().count()
    );
    let mut held = HeldKeys::default();
    loop {
        let timeout = held
            .deadline()
            .map(|due| due.saturating_duration_since(Instant::now()));
        if let Some(event) = gamepads.next_event_blocking(timeout) {
            let now = Instant::now();
            let id = usize::from(event.id);
            let mut events = Vec::new();
            match event.event {
                EventType::Connected => {
                    info!("Gamepad connected: {}", gamepads.gamepad(event.id).name());
                }
                EventType::Disconnected => {
                    info!(
                        "Gamepad disconnected: {}",
                        gamepads.gamepad(event.id).name()
                    );
                    let keys: Vec<_> = held
                        .keys
                        .keys()
                        .filter_map(|&(held_id, key)| (held_id == id).then_some(key))
                        .collect();
                    for key in keys {
                        events.extend(held.set(id, key, false, now));
                    }
                }
                EventType::ButtonPressed(button, _) | EventType::ButtonReleased(button, _) => {
                    if let Some(key) = button_name(button) {
                        let down = matches!(event.event, EventType::ButtonPressed(..));
                        events.extend(held.set(id, key, down, now));
                    }
                }
                EventType::AxisChanged(axis, value, _) => {
                    if let Some((negative, positive)) = stick_names(axis) {
                        events.extend(held.set(id, negative, value < -STICK_THRESHOLD, now));
                        events.extend(held.set(id, positive, value > STICK_THRESHOLD, now));
                    }
                }
                _ => {}
            }
            for event in events {
                if proxy.send_event(event).is_err() {
                    return;
                }
            }
        }
        for event in held.repeats(Instant::now()) {
            if proxy.send_event(event).is_err() {
                return;
            }
        }
    }
}

const fn stick_names(axis: Axis) -> Option<(&'static str, &'static str)> {
    match axis {
        Axis::LeftStickX => Some(("GamepadLeftStickLeft", "GamepadLeftStickRight")),
        // Gilrs reports up as positive.
        Axis::LeftStickY => Some(("GamepadLeftStickDown", "GamepadLeftStickUp")),
        _ => None,
    }
}

const fn button_name(button: Button) -> Option<&'static str> {
    Some(match button {
        Button::South => "GamepadSouth",
        Button::East => "GamepadEast",
        Button::North => "GamepadNorth",
        Button::West => "GamepadWest",
        Button::C => "GamepadC",
        Button::Z => "GamepadZ",
        Button::LeftTrigger => "GamepadLeftTrigger",
        Button::LeftTrigger2 => "GamepadLeftTrigger2",
        Button::RightTrigger => "GamepadRightTrigger",
        Button::RightTrigger2 => "GamepadRightTrigger2",
        Button::Select => "GamepadSelect",
        Button::Start => "GamepadStart",
        Button::Mode => "GamepadMode",
        Button::LeftThumb => "GamepadLeftThumb",
        Button::RightThumb => "GamepadRightThumb",
        Button::DPadUp => "GamepadDPadUp",
        Button::DPadDown => "GamepadDPadDown",
        Button::DPadLeft => "GamepadDPadLeft",
        Button::DPadRight => "GamepadDPadRight",
        Button::Unknown => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_directions_repeat_and_releases_remove_deadlines() {
        let mut held = HeldKeys::default();
        let now = Instant::now();
        assert!(held.set(0, "GamepadDPadDown", true, now).is_some());
        assert_eq!(held.deadline(), Some(now + REPEAT_DELAY));
        let before_repeat = (now + REPEAT_DELAY)
            .checked_sub(Duration::from_millis(1))
            .unwrap();
        assert!(held.repeats(before_repeat).is_empty());
        assert!(matches!(
            held.repeats(now + REPEAT_DELAY).as_slice(),
            [AppEvent::Gamepad { repeat: true, .. }]
        ));
        assert_eq!(held.deadline(), Some(now + REPEAT_DELAY + REPEAT_INTERVAL));
        assert!(held.set(0, "GamepadDPadDown", false, now).is_some());
        assert!(held.deadline().is_none());
        assert!(held.repeats(now + Duration::from_secs(2)).is_empty());
    }

    #[test]
    fn buttons_do_not_repeat_and_one_controller_cannot_release_another() {
        let mut held = HeldKeys::default();
        let now = Instant::now();
        assert!(held.set(0, "GamepadRightTrigger2", true, now).is_some());
        assert!(held.set(1, "GamepadRightTrigger2", true, now).is_none());
        assert!(held.deadline().is_none());
        assert!(held.set(0, "GamepadRightTrigger2", false, now).is_none());
        assert!(matches!(
            held.set(1, "GamepadRightTrigger2", false, now),
            Some(AppEvent::Gamepad { down: false, .. })
        ));
        assert!(held.keys.is_empty());
    }
}
