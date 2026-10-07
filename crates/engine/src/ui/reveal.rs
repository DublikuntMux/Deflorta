use std::time::Instant;

use num_traits::{AsPrimitive, ToPrimitive};

use super::desc::SpanDesc;
use crate::util::time::elapsed_secs;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stop {
    Wait(f32),
    Click,
}

pub struct RevealScript {
    pub stops: Vec<(usize, Stop)>,
    pub fast: usize,
    pub total: usize,
}

impl RevealScript {
    pub fn from_spans(spans: &[SpanDesc]) -> Self {
        let mut stops = Vec::new();
        let mut fast = 0;
        let mut pos = 0;
        for span in spans {
            if span.fast {
                fast = pos;
            }
            if let Some(wait) = span.wait {
                stops.push((pos, Stop::Wait(wait.max(0.0))));
            }
            if span.click {
                stops.push((pos, Stop::Click));
            }
            pos += span.text.chars().count();
        }
        stops.retain(|(p, _)| *p >= fast);
        Self {
            stops,
            fast,
            total: pos,
        }
    }
}

pub struct Reveal {
    pub spans: Vec<SpanDesc>,
    script: RevealScript,
    cps: f32,
    start: Instant,
    /// Characters visible when `start` was set.
    base: usize,
    next_stop: usize,
}

impl Reveal {
    pub fn new(spans: Vec<SpanDesc>, cps: f32, now: Instant, instant: bool) -> Self {
        let script = RevealScript::from_spans(&spans);
        let base = if instant { script.total } else { script.fast };
        let next_stop = if instant { script.stops.len() } else { 0 };
        Self {
            spans,
            script,
            cps,
            start: now,
            base,
            next_stop,
        }
    }

    /// Characters visible at `now`, and whether typing has stopped at a click-wait.
    fn progress(&self, now: Instant) -> (usize, bool) {
        let mut pos = self.base;
        let mut t = elapsed_secs(self.start, now);
        let advance = |pos: usize, t: f32, cps: f32| -> usize {
            if cps <= 0.0 {
                usize::MAX
            } else {
                pos.saturating_add((t * cps).max(0.0).to_usize().unwrap_or(usize::MAX))
            }
        };
        for &(stop_pos, stop) in &self.script.stops[self.next_stop..] {
            let chars = stop_pos.saturating_sub(pos);
            let need = if self.cps <= 0.0 {
                0.0
            } else {
                let chars: f32 = chars.as_();
                chars / self.cps
            };
            if t < need {
                return (advance(pos, t, self.cps).min(stop_pos), false);
            }
            t -= need;
            pos = stop_pos.max(pos);
            match stop {
                Stop::Wait(seconds) => {
                    if t < seconds {
                        return (pos, false);
                    }
                    t -= seconds;
                }
                Stop::Click => return (pos, true),
            }
        }
        (advance(pos, t, self.cps).min(self.script.total), false)
    }

    pub fn shown(&self, now: Instant) -> usize {
        self.progress(now).0
    }

    /// Still typing or in a timed pause (as opposed to stopped at a click-wait).
    pub fn is_typing(&self, now: Instant) -> bool {
        let (shown, at_click) = self.progress(now);
        !at_click && shown < self.script.total
    }

    pub fn is_revealing(&self, now: Instant) -> bool {
        self.shown(now) < self.script.total
    }

    /// Click behavior: when stopped at a click-wait, resume typing; otherwise
    /// show everything up to the next click-wait (or the end).
    pub fn skip(&mut self, now: Instant) {
        let (shown, at_click) = self.progress(now);
        let passed = self.script.stops[self.next_stop..]
            .iter()
            .take_while(|(p, _)| *p < shown || (*p == shown && at_click))
            .count();
        let mut index = self.next_stop + passed;
        if at_click {
            self.base = shown;
            self.next_stop = index;
            self.start = now;
            return;
        }
        while index < self.script.stops.len() && self.script.stops[index].1 != Stop::Click {
            index += 1;
        }
        if let Some(&(pos, _)) = self.script.stops.get(index) {
            self.base = pos;
            self.next_stop = index;
        } else {
            self.base = self.script.total;
            self.next_stop = self.script.stops.len();
        }
        self.start = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn span(text: &str) -> SpanDesc {
        SpanDesc {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn extreme_speed_finishes_from_fast_forward_without_overflow() {
        let spans = vec![
            span("a"),
            SpanDesc {
                fast: true,
                ..span("bc")
            },
        ];
        let now = Instant::now();
        let reveal = Reveal::new(spans, f32::MAX, now, false);
        assert_eq!(reveal.shown(now), 1);
        assert_eq!(reveal.shown(now + Duration::from_secs(2)), 3);
        assert!(!reveal.is_revealing(now + Duration::from_secs(2)));
    }

    #[test]
    fn stops_at_click_and_resumes() {
        let spans = vec![
            span("Hello"),
            SpanDesc {
                click: true,
                ..Default::default()
            },
            span(" world"),
        ];
        let t0 = Instant::now();
        let mut r = Reveal::new(spans, 10.0, t0, false);
        assert_eq!(r.shown(t0 + Duration::from_millis(300)), 3);
        assert_eq!(r.shown(t0 + Duration::from_secs(5)), 5);
        assert!(r.is_revealing(t0 + Duration::from_secs(5)));
        r.skip(t0 + Duration::from_secs(5));
        assert_eq!(r.shown(t0 + Duration::from_secs(5)), 5);
        assert_eq!(r.shown(t0 + Duration::from_millis(5300)), 8);
        r.skip(t0 + Duration::from_millis(5300));
        assert_eq!(r.shown(t0 + Duration::from_millis(5300)), 11);
    }

    #[test]
    fn skip_mid_typing_jumps_to_click_stop() {
        let spans = vec![
            span("abcdef"),
            SpanDesc {
                click: true,
                ..Default::default()
            },
            span("gh"),
        ];
        let t0 = Instant::now();
        let mut r = Reveal::new(spans, 2.0, t0, false);
        r.skip(t0);
        assert_eq!(r.shown(t0), 6);
        assert!(r.is_revealing(t0));
    }

    #[test]
    fn timed_wait_and_fast() {
        let spans = vec![
            span("ab"),
            SpanDesc {
                wait: Some(1.0),
                ..Default::default()
            },
            span("cd"),
            SpanDesc {
                fast: true,
                ..Default::default()
            },
            span("ef"),
        ];
        let t0 = Instant::now();
        let r = Reveal::new(spans, 0.0, t0, false);
        assert_eq!(r.shown(t0), 6);
        let r = Reveal::new(
            vec![
                span("ab"),
                SpanDesc {
                    wait: Some(1.0),
                    ..Default::default()
                },
                span("cd"),
            ],
            0.0,
            t0,
            false,
        );
        assert_eq!(r.shown(t0), 2);
        assert_eq!(r.shown(t0 + Duration::from_millis(1100)), 4);
    }
}
