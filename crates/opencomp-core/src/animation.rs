//! Key-frame animations: what an animation is once the API has been stripped off.
//!
//! The composition API models an animation as an object with key frames inserted at
//! times, an easing function per key frame, a duration, a delay and an iteration
//! count. Sampling it for a given time is the only thing the renderer needs, and it
//! is the only part that can be tested without a window.

/// A curve that maps 0..=1 to 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    /// The four control points of a cubic bezier, `(x1, y1, x2, y2)`.
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    pub fn apply(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Easing::Linear => progress,
            Easing::CubicBezier(x1, y1, x2, y2) => {
                // Solve x(u) = progress for u, then read y(u). Sixteen bisections put the
                // result within a pixel of the curve, which is all a 60 Hz frame can show.
                let x = |u: f32| {
                    let v = 1.0 - u;
                    3.0 * v * v * u * x1 + 3.0 * v * u * u * x2 + u * u * u
                };
                let y = |u: f32| {
                    let v = 1.0 - u;
                    3.0 * v * v * u * y1 + 3.0 * v * u * u * y2 + u * u * u
                };
                let (mut low, mut high) = (0.0f32, 1.0f32);
                for _ in 0..16 {
                    let middle = (low + high) / 2.0;
                    if x(middle) < progress {
                        low = middle;
                    } else {
                        high = middle;
                    }
                }
                y((low + high) / 2.0)
            }
        }
    }
}

/// One key frame: a time in seconds, four channels and the curve leaving it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub time: f32,
    pub value: [f32; 4],
    pub easing: Easing,
}

/// A whole animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframes {
    /// Seconds; zero means "as long as the last key frame".
    pub duration: f32,
    pub delay: f32,
    /// Zero iterations means "forever".
    pub iterations: i32,
    pub frames: Vec<Frame>,
}

impl Default for Keyframes {
    fn default() -> Self {
        Self {
            duration: 1.0,
            delay: -1.0,
            iterations: 1,
            frames: Vec::new(),
        }
    }
}

impl Keyframes {
    /// The value at `time` seconds after the animation started, if it has one.
    pub fn sample(&self, time: f32) -> Option<[f32; 4]> {
        let (first, last) = (self.frames.first()?, self.frames.last()?);
        let span = if self.duration > 0.0 {
            self.duration
        } else {
            last.time
        };
        let mut local = time - self.delay.max(0.0);
        if local < 0.0 {
            return Some(first.value);
        }
        if span > 0.0 {
            if self.iterations == 0 {
                local %= span;
            } else if self.iterations > 0 && local >= span * self.iterations as f32 {
                local = span * self.iterations as f32;
            } else if local >= span {
                local %= span;
            }
        }
        if local <= first.time {
            return Some(first.value);
        }
        if local >= last.time {
            return Some(last.value);
        }
        let index = self.frames.partition_point(|frame| frame.time <= local) - 1;
        let from = self.frames[index];
        let to = self.frames[index + 1];
        let span = to.time - from.time;
        let progress = if span > 0.0 {
            (local - from.time) / span
        } else {
            1.0
        };
        let progress = from.easing.apply(progress);
        Some([
            from.value[0] + (to.value[0] - from.value[0]) * progress,
            from.value[1] + (to.value[1] - from.value[1]) * progress,
            from.value[2] + (to.value[2] - from.value[2]) * progress,
            from.value[3] + (to.value[3] - from.value[3]) * progress,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyframes(frames: Vec<Frame>) -> Keyframes {
        Keyframes {
            frames,
            ..Keyframes::default()
        }
    }

    fn frame(time: f32, value: [f32; 4]) -> Frame {
        Frame {
            time,
            value,
            easing: Easing::Linear,
        }
    }

    #[test]
    fn an_empty_animation_has_no_value() {
        assert!(Keyframes::default().sample(0.0).is_none());
    }

    #[test]
    fn a_linear_ramp_is_halfway_at_half_the_time() {
        let animation = keyframes(vec![frame(0.0, [0.0; 4]), frame(1.0, [1.0; 4])]);
        assert_eq!(animation.sample(0.5).unwrap()[0], 0.5);
        assert_eq!(animation.sample(0.25).unwrap()[0], 0.25);
    }

    #[test]
    fn the_value_holds_before_the_first_and_after_the_last_frame() {
        let animation = Keyframes {
            frames: vec![frame(0.2, [0.3; 4]), frame(0.8, [0.9; 4])],
            ..Keyframes::default()
        };
        assert_eq!(animation.sample(0.0).unwrap()[0], 0.3);
        assert_eq!(animation.sample(5.0).unwrap()[0], 0.9);
    }

    #[test]
    fn a_zero_iteration_animation_never_ends() {
        let animation = Keyframes {
            frames: vec![frame(0.0, [0.0; 4]), frame(1.0, [1.0; 4])],
            iterations: 0,
            ..Keyframes::default()
        };
        // 2.5 s into an endless one-second animation is the same as 0.5 s.
        assert_eq!(animation.sample(1.5).unwrap()[0], 0.5);
    }

    #[test]
    fn a_finished_animation_holds_its_last_value() {
        let animation = Keyframes {
            frames: vec![frame(0.0, [0.0; 4]), frame(1.0, [1.0; 4])],
            iterations: 2,
            ..Keyframes::default()
        };
        assert_eq!(animation.sample(3.0).unwrap()[0], 1.0);
    }

    #[test]
    fn a_delay_holds_the_first_value_until_it_is_over() {
        let animation = Keyframes {
            delay: 0.5,
            frames: vec![frame(0.0, [0.0; 4]), frame(1.0, [1.0; 4])],
            ..Keyframes::default()
        };
        assert_eq!(animation.sample(0.25).unwrap()[0], 0.0);
        assert_eq!(animation.sample(1.0).unwrap()[0], 0.5);
    }

    #[test]
    fn the_easing_function_bends_the_middle_of_a_segment() {
        let eased = Easing::CubicBezier(0.42, 0.0, 0.58, 1.0);
        let middle = eased.apply(0.5);
        assert!((middle - 0.5).abs() < 0.01, "symmetric curve: {middle}");

        let slow_start = Easing::CubicBezier(0.9, 0.0, 1.0, 0.5);
        assert!(slow_start.apply(0.5) < 0.3);
        assert!((slow_start.apply(0.0) - 0.0).abs() < 1e-3);
        assert!((slow_start.apply(1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn a_key_frame_can_have_its_own_curve() {
        let animation = keyframes(vec![
            Frame {
                time: 0.0,
                value: [0.0; 4],
                easing: Easing::CubicBezier(2.0 / 3.0, 0.0, 1.0, 1.0),
            },
            frame(1.0, [1.0; 4]),
        ]);
        // That curve is close to "stay put, then jump": at half the time it is under a fifth.
        assert!(animation.sample(0.5).unwrap()[0] < 0.25);
    }

    #[test]
    fn each_channel_interpolates_on_its_own() {
        let animation = keyframes(vec![
            frame(0.0, [0.0, 0.0, 0.0, 1.0]),
            frame(1.0, [1.0, 0.0, 0.5, 0.0]),
        ]);
        let value = animation.sample(0.5).unwrap();
        assert_eq!(value[0], 0.5);
        assert_eq!(value[1], 0.0);
        assert_eq!(value[2], 0.25);
        assert_eq!(value[3], 0.5);
    }
}
