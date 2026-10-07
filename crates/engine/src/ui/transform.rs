use num_traits::AsPrimitive;

use crate::util::math::lerp;

use super::Rect;
use super::desc::{RepeatCount, TransformProps, TransformStep};

/// A similarity transform: p' = R(angle) * (p * scale) + t, stored as
/// p' = (a*x - b*y + tx, b*x + a*y + ty).
#[derive(Clone, Copy, Debug)]
pub struct Similarity {
    pub a: f32,
    pub b: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Similarity {
    pub const fn new(scale: f32, tx: f32, ty: f32) -> Self {
        Self {
            a: scale,
            b: 0.0,
            tx,
            ty,
        }
    }

    pub fn scale(&self) -> f32 {
        self.a.hypot(self.b)
    }

    pub fn angle(&self) -> f32 {
        self.b.atan2(self.a)
    }

    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.b.mul_add(-y, self.a * x) + self.tx,
            self.a.mul_add(y, self.b * x) + self.ty,
        )
    }

    /// `self ∘ other`: applies `other` first.
    pub fn then(&self, other: &Self) -> Self {
        Self {
            a: self.b.mul_add(-other.b, self.a * other.a),
            b: self.a.mul_add(other.b, self.b * other.a),
            tx: self.b.mul_add(-other.ty, self.a * other.tx) + self.tx,
            ty: self.a.mul_add(other.ty, self.b * other.tx) + self.ty,
        }
    }

    /// Scale `k` and rotation `angle` about pivot (px, py), then translate by (dx, dy).
    pub fn local(k: f32, angle: f32, px: f32, py: f32, dx: f32, dy: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let (a, b) = (k * cos, k * sin);
        Self {
            a,
            b,
            tx: px - f32::mul_add(b, -py, a * px) + dx,
            ty: py - f32::mul_add(a, py, b * px) + dy,
        }
    }

    /// The rectangle after transformation, as an unrotated rectangle around
    /// the transformed center plus a rotation angle.
    pub fn rect(&self, r: Rect) -> (Rect, f32) {
        let (cx, cy) = self.apply(r.x + r.w / 2.0, r.y + r.h / 2.0);
        let s = self.scale();
        let (w, h) = (r.w * s, r.h * s);
        (
            Rect {
                x: cx - w / 2.0,
                y: cy - h / 2.0,
                w,
                h,
            },
            self.angle(),
        )
    }

    pub fn bounds(&self, r: Rect) -> Rect {
        let corners = [
            (r.x, r.y),
            (r.x + r.w, r.y),
            (r.x, r.y + r.h),
            (r.x + r.w, r.y + r.h),
        ];
        let points = corners.map(|(x, y)| self.apply(x, y));
        let min_x = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
        let max_x = points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max);
        let min_y = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
        let max_y = points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max);
        Rect {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Values {
    pub x: f32,
    pub y: f32,
    pub opacity: f32,
    pub scale: f32,
    /// Degrees.
    pub rotate: f32,
    pub crop: [f32; 4],
}

impl Default for Values {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            opacity: 1.0,
            scale: 1.0,
            rotate: 0.0,
            crop: [0.0, 0.0, 1.0, 1.0],
        }
    }
}

impl Values {
    /// Moves each property set in `props` from its current value toward the target by `t`.
    fn tween(&mut self, props: &TransformProps, t: f32) {
        if let Some(v) = props.x {
            self.x = lerp(self.x, v, t);
        }
        if let Some(v) = props.y {
            self.y = lerp(self.y, v, t);
        }
        if let Some(v) = props.opacity {
            self.opacity = lerp(self.opacity, v, t);
        }
        if let Some(v) = props.scale {
            self.scale = lerp(self.scale, v, t);
        }
        if let Some(v) = props.rotate {
            self.rotate = lerp(self.rotate, v, t);
        }
        if let Some(c) = props.crop {
            for (cur, target) in self.crop.iter_mut().zip(c) {
                *cur = lerp(*cur, target, t);
            }
        }
    }
}

fn walk(steps: &[TransformStep], add: &mut dyn FnMut(&TransformProps)) {
    for step in steps {
        match step {
            TransformStep::Set { set } => add(set),
            TransformStep::Tween { to, .. } => add(to),
            TransformStep::Pause { .. } => {}
            TransformStep::Parallel { parallel } => parallel.iter().for_each(|b| walk(b, add)),
            TransformStep::Repeat { steps, .. } => walk(steps, add),
        }
    }
}

/// Bit set of properties touched by a program, used to merge parallel branches.
fn touched(steps: &[TransformStep]) -> [bool; 6] {
    let mut mask = [false; 6];
    let mut add = |p: &TransformProps| {
        mask[0] |= p.x.is_some();
        mask[1] |= p.y.is_some();
        mask[2] |= p.opacity.is_some();
        mask[3] |= p.scale.is_some();
        mask[4] |= p.rotate.is_some();
        mask[5] |= p.crop.is_some();
    };
    walk(steps, &mut add);
    mask
}

const fn merge(into: &mut Values, from: &Values, mask: [bool; 6]) {
    if mask[0] {
        into.x = from.x;
    }
    if mask[1] {
        into.y = from.y;
    }
    if mask[2] {
        into.opacity = from.opacity;
    }
    if mask[3] {
        into.scale = from.scale;
    }
    if mask[4] {
        into.rotate = from.rotate;
    }
    if mask[5] {
        into.crop = from.crop;
    }
}

/// Total duration of a program; `None` when it repeats forever.
pub fn duration(steps: &[TransformStep]) -> Option<f32> {
    let mut total = 0.0;
    for step in steps {
        total += match step {
            TransformStep::Set { .. } => 0.0,
            TransformStep::Tween { dur, .. } => dur.max(0.0),
            TransformStep::Pause { pause } => pause.max(0.0),
            TransformStep::Parallel { parallel } => {
                let mut longest: f32 = 0.0;
                for branch in parallel {
                    longest = longest.max(duration(branch)?);
                }
                longest
            }
            TransformStep::Repeat { repeat, steps } => match repeat {
                RepeatCount::Forever(false) => duration(steps)?,
                RepeatCount::Forever(true) => {
                    if duration(steps)? <= 0.0 {
                        0.0
                    } else {
                        return None;
                    }
                }
                RepeatCount::Times(n) => {
                    let count: f32 = n.as_();
                    duration(steps)? * count
                }
            },
        };
    }
    Some(total)
}

pub fn evaluate(steps: &[TransformStep], time: f32, values: &mut Values) -> bool {
    let mut time = time;
    for step in steps {
        match step {
            TransformStep::Set { set } => values.tween(set, 1.0),
            TransformStep::Pause { pause } => {
                if time < *pause {
                    return false;
                }
                time -= pause;
            }
            TransformStep::Tween { dur, ease, to } => {
                if time < *dur {
                    values.tween(to, ease.apply(time / dur));
                    return false;
                }
                values.tween(to, 1.0);
                time -= dur.max(0.0);
            }
            TransformStep::Parallel { parallel } => {
                let start = *values;
                let mut done = true;
                let mut longest: f32 = 0.0;
                for branch in parallel {
                    let mut branch_values = start;
                    done &= evaluate(branch, time, &mut branch_values);
                    longest = longest.max(duration(branch).unwrap_or(f32::INFINITY));
                    merge(values, &branch_values, touched(branch));
                }
                if !done {
                    return false;
                }
                time -= longest;
            }
            TransformStep::Repeat { repeat, steps } => {
                let count = match repeat {
                    RepeatCount::Forever(true) => None,
                    RepeatCount::Forever(false) => Some(1),
                    RepeatCount::Times(n) => Some(*n),
                };
                let Some(period) = duration(steps).filter(|d| *d > 0.0) else {
                    evaluate(steps, time, values);
                    continue;
                };
                let iteration = (time / period).floor();
                if count.is_none_or(|n| iteration < n.as_()) {
                    if iteration >= 1.0 {
                        let mut end = *values;
                        evaluate(steps, period, &mut end);
                        *values = end;
                    }
                    evaluate(steps, iteration.mul_add(-period, time), values);
                    return false;
                }
                let n: f32 = count.unwrap_or(1).as_();
                if n >= 2.0 {
                    let mut end = *values;
                    evaluate(steps, period, &mut end);
                    *values = end;
                }
                evaluate(steps, period, values);
                time = f32::mul_add(period, -n, time);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::desc::{Ease, TransformDesc};

    fn program(json: &str) -> Vec<TransformStep> {
        serde_json::from_str::<TransformDesc>(json).unwrap().steps
    }

    #[test]
    fn tweens_and_pauses() {
        let steps = program(
            r#"{"steps":[{"set":{"x":0}},{"dur":1,"to":{"x":100}},{"pause":1},{"dur":1,"ease":"linear","to":{"x":0}}]}"#,
        );
        let mut v = Values::default();
        assert!(!evaluate(&steps, 0.5, &mut v));
        assert_eq!(v.x, 50.0);
        let mut v = Values::default();
        evaluate(&steps, 1.5, &mut v);
        assert_eq!(v.x, 100.0);
        let mut v = Values::default();
        assert!(evaluate(&steps, 3.5, &mut v));
        assert_eq!(v.x, 0.0);
        assert_eq!(duration(&steps), Some(3.0));
    }

    #[test]
    fn repeat_forever_loops() {
        let steps = program(
            r#"{"steps":[{"repeat":true,"steps":[{"dur":1,"to":{"y":10}},{"dur":1,"to":{"y":0}}]}]}"#,
        );
        assert_eq!(duration(&steps), None);
        let mut v = Values::default();
        assert!(!evaluate(&steps, 4.5, &mut v));
        assert!((v.y - 5.0).abs() < 1e-4);
    }

    #[test]
    fn parallel_merges_branches() {
        let steps = program(
            r#"{"steps":[{"parallel":[[{"dur":2,"to":{"x":20}}],[{"dur":1,"to":{"opacity":0}}]]}]}"#,
        );
        let mut v = Values::default();
        evaluate(&steps, 1.0, &mut v);
        assert_eq!((v.x, v.opacity), (10.0, 0.0));
        assert_eq!(Ease::Linear.apply(0.25), 0.25);
    }

    #[test]
    fn similarity_composes() {
        let rot = Similarity::local(2.0, std::f32::consts::FRAC_PI_2, 0.0, 0.0, 0.0, 0.0);
        let shift = Similarity::new(1.0, 10.0, 0.0);
        let (x, y) = shift.then(&rot).apply(1.0, 0.0);
        assert!((x - 10.0).abs() < 1e-4 && (y - 2.0).abs() < 1e-4);
    }
}
