//! One `Grid` axis: validate the definitions, resolve the star shares and set
//! the final sizes.
//!
//! Every helper names the function it ports from
//! `dxaml/xcp/core/core/elements/Grid.cpp`: `ValidateDefinitions`,
//! `GetPreferredSize` (`GridDefinitions.h`), `ResolveStar`,
//! `DistributeStarSpace`, `EnsureMinSizeInDefinitionRange`,
//! `SortDefinitionsForSpanMaxSizeDistribution`, `SortDefinitionsForStarSizeDistribution`,
//! `SortDefinitionsForOverflowSizeDistribution` and `SetFinalSize`.

use super::super::layout_round;
use crate::element::{GridLength, GridTrack};

/// `REAL_EPSILON` in `dxaml/xcp/core/core/elements/xcpmath.h`.
const REAL_EPSILON: f32 = 1.192_092_9e-7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unit {
    Pixels,
    Auto,
    Star,
}

/// `CDefinitionBase`'s state after `ValidateDefinitions`; `measure` is
/// `m_measureArrangeSize` and `effective_min` is `m_effectiveMinSize`.
#[derive(Clone, Copy, Debug)]
struct Track {
    unit: Unit,
    weight: f32,
    user_max: f32,
    effective_min: f32,
    measure: f32,
}

impl Track {
    /// `CGrid::ValidateDefinitions`, with `treatStarAsAuto` from an infinite
    /// available size. The user min is clamped at zero; XAML validates that
    /// anyway and a negative min has no meaning here.
    fn new(definition: &GridTrack, treat_star_as_auto: bool) -> Self {
        let user_min = layout_round(definition.min.max(0.0));
        let user_max = layout_round(definition.max.max(0.0));
        match definition.length {
            GridLength::Pixels(pixels) => {
                let size = layout_round(pixels.max(0.0));
                let effective_min = user_min.max(size.min(user_max));
                Self {
                    unit: Unit::Pixels,
                    weight: 0.0,
                    user_max,
                    effective_min,
                    measure: effective_min,
                }
            }
            GridLength::Auto => Self {
                unit: Unit::Auto,
                weight: 0.0,
                user_max,
                effective_min: user_min,
                measure: user_min.max(user_max),
            },
            GridLength::Star(weight) => {
                let unit = if treat_star_as_auto {
                    Unit::Auto
                } else {
                    Unit::Star
                };
                Self {
                    unit,
                    weight: weight.max(0.0),
                    user_max,
                    effective_min: user_min,
                    measure: user_min.max(user_max),
                }
            }
        }
    }

    /// `max(GetUserMaxSize(), GetEffectiveMinSize())`, the `sizeCache` value
    /// `EnsureMinSizeInDefinitionRange` stores.
    fn max_size(&self) -> f32 {
        self.user_max.max(self.effective_min)
    }

    /// `maxSize / starValue`, the key `SortDefinitionsForStarSizeDistribution`
    /// sorts on. A zero-weight star gets zero, so it sorts first and resolves
    /// to its effective min.
    fn star_ratio(&self) -> f32 {
        if self.weight < REAL_EPSILON {
            0.0
        } else {
            self.max_size() / self.weight
        }
    }
}

/// `m_pRows` or `m_pColumns` for one measure or arrange pass.
pub(super) struct Tracks {
    tracks: Vec<Track>,
    spacing: f32,
    /// The size handed to the tracks, already minus the spacing between them.
    available: f32,
}

impl Tracks {
    /// Build the definitions. An empty collection is the engine's default
    /// single `1*` track (`CGrid::InitializeDefinitionStructure`).
    pub(super) fn new(definitions: &[GridTrack], spacing: f32, available: f32) -> Self {
        let treat_star_as_auto = !available.is_finite();
        let mut tracks: Vec<Track> = definitions
            .iter()
            .map(|definition| Track::new(definition, treat_star_as_auto))
            .collect();
        if tracks.is_empty() {
            tracks.push(Track::new(&GridTrack::star(1.0), treat_star_as_auto));
        }
        Self {
            tracks,
            spacing,
            available,
        }
    }

    /// `GetDesiredInnerSize`: the sum of `effectiveMinSize`, stars included.
    pub(super) fn desired(&self) -> f32 {
        self.tracks.iter().map(|track| track.effective_min).sum()
    }

    /// `CGrid::GetRowSpan`/`GetColumnSpan`: clamp the first index and the span
    /// to the tracks that remain.
    pub(super) fn range(&self, index: usize, span: usize) -> (usize, usize) {
        let start = index.min(self.tracks.len() - 1);
        let span = span.max(1).min(self.tracks.len() - start);
        (start, span)
    }

    fn has(&self, start: usize, span: usize, unit: Unit) -> bool {
        self.tracks[start..start + span]
            .iter()
            .any(|track| track.unit == unit)
    }

    pub(super) fn range_has_star(&self, start: usize, span: usize) -> bool {
        self.has(start, span, Unit::Star)
    }

    /// `CGrid::GetAvailableSizeForRange`: an auto count uses its effective min,
    /// every other track its measure size, plus the spacing inside the range.
    fn range_available(&self, start: usize, span: usize) -> f32 {
        let sum: f32 = self.tracks[start..start + span]
            .iter()
            .map(|track| match track.unit {
                Unit::Auto => track.effective_min,
                Unit::Pixels | Unit::Star => track.measure,
            })
            .sum();
        sum + self.spacing * span.saturating_sub(1) as f32
    }

    /// `CGrid::MeasureCell`: a range with an auto track and no star track is
    /// measured freely; otherwise it gets the sum of the tracks it covers.
    pub(super) fn cell_constraint(&self, start: usize, span: usize) -> f32 {
        if self.has(start, span, Unit::Auto) && !self.has(start, span, Unit::Star) {
            f32::INFINITY
        } else {
            self.range_available(start, span)
        }
    }

    /// `MeasureCellsGroup`: a one-track cell raises that track's effective min;
    /// a span hands its desired size to `EnsureMinSizeInDefinitionRange`.
    pub(super) fn contribute(&mut self, start: usize, span: usize, desired: f32) {
        if span == 1 {
            let track = &mut self.tracks[start];
            track.effective_min = track.effective_min.max(desired);
        } else {
            self.ensure_min_in_range(start, span, desired);
        }
    }

    /// `CGrid::ResolveStar`, used during measure to give star tracks the size a
    /// star cell is measured against.
    pub(super) fn resolve_stars(&mut self) {
        let mut taken = 0.0;
        let mut stars = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            match track.unit {
                Unit::Star => stars.push(index),
                Unit::Pixels => taken += track.measure,
                Unit::Auto => taken += track.effective_min,
            }
        }
        // ResolveStar rounds the taken and the available size together.
        let available = layout_round(self.available) - layout_round(taken);
        self.distribute(&stars, available);
    }

    /// `CGrid::DistributeStarSpace`. Stars are visited once, in ascending
    /// `maxSize / weight` order; each resolved size is clamped to
    /// `[effectiveMin, userMax]` and rounded, and the running total is what the
    /// next star sees. A clamp therefore moves the remainder to the later stars.
    fn distribute(&mut self, stars: &[usize], available: f32) {
        if stars.is_empty() {
            return;
        }
        // Stable sort: equal ratios keep definition order.
        let mut order: Vec<usize> = stars.to_vec();
        order.sort_by(|&left, &right| {
            self.tracks[left]
                .star_ratio()
                .total_cmp(&self.tracks[right].star_ratio())
        });

        // Partial sums of the weights, suffix inclusive, for the denominator.
        let mut suffix = vec![0.0_f32; order.len()];
        let mut sum = 0.0;
        for (slot, &index) in order.iter().enumerate().rev() {
            sum += self.tracks[index].weight;
            suffix[slot] = sum;
        }

        let mut total = 0.0;
        for (slot, &index) in order.iter().enumerate() {
            let track = &mut self.tracks[index];
            let resolved = if track.weight < REAL_EPSILON {
                track.effective_min
            } else {
                let share = (available - total).max(0.0) * track.weight / suffix[slot];
                // MAX(effectiveMin, MIN(resolved, userMax)): the min wins over
                // the max when a definition was given min > max.
                track.effective_min.max(share.min(track.user_max))
            };
            let resolved = layout_round(resolved);
            track.measure = resolved;
            total += resolved;
        }
    }

    /// `CGrid::SetFinalSize`: non-star tracks keep their measure size, the
    /// stars divide what is left, then an overflowing total is taken back.
    pub(super) fn set_final_size(&mut self) {
        let mut preferred = 0.0;
        let mut stars = Vec::new();
        for (index, track) in self.tracks.iter_mut().enumerate() {
            match track.unit {
                Unit::Star => stars.push(index),
                Unit::Pixels => {
                    track.measure = track.measure.min(track.user_max).max(track.effective_min);
                    preferred += track.measure;
                }
                Unit::Auto => {
                    track.measure = track.effective_min;
                    preferred += track.measure;
                }
            }
        }
        self.distribute(&stars, self.available - preferred);

        let total: f32 = self.tracks.iter().map(|track| track.measure).sum();
        if total > self.available && total - self.available > REAL_EPSILON {
            self.remove_overflow(total);
        }
    }

    /// The overflow half of `CGrid::SetFinalSize`: stable-sort by
    /// `size - effectiveMin`, give every track one share of the excess, clamp
    /// each to its own remaining slack. The engine's loop visits each track once.
    fn remove_overflow(&mut self, total: f32) {
        let count = self.tracks.len();
        let mut order: Vec<usize> = (0..count).collect();
        order.sort_by(|&left, &right| {
            let left = self.tracks[left].measure - self.tracks[left].effective_min;
            let right = self.tracks[right].measure - self.tracks[right].effective_min;
            left.total_cmp(&right)
        });

        let mut to_distribute = self.available - total;
        for (slot, &index) in order.iter().enumerate() {
            let track = &mut self.tracks[index];
            let current = track.measure;
            let next = current + to_distribute / (count - slot) as f32;
            let next = next.max(track.effective_min).min(current);
            to_distribute -= next - current;
            track.measure = next;
        }
    }

    /// `CGrid::EnsureMinSizeInDefinitionRange` for a span whose tracks are all
    /// pixel or auto. For such a range `GetPreferredSize` is the effective min
    /// on every track, so the engine's `rangePreferred` branch is unreachable;
    /// the max-size branch and the beyond-max branch are ported.
    fn ensure_min_in_range(&mut self, start: usize, span: usize, child_desired: f32) {
        let requested = (child_desired - self.spacing * span.saturating_sub(1) as f32).max(0.0);
        if requested <= REAL_EPSILON {
            return;
        }

        let mut auto_count = 0;
        let mut range_min = 0.0;
        let mut range_max = 0.0;
        let mut max_max = 0.0_f32;
        let mut max_sizes = Vec::with_capacity(span);
        for track in &self.tracks[start..start + span] {
            let max_size = track.max_size();
            range_min += track.effective_min;
            range_max += max_size;
            max_max = max_max.max(max_size);
            if track.unit == Unit::Auto {
                auto_count += 1;
            }
            max_sizes.push(max_size);
        }

        if requested <= range_min {
            return;
        }

        if requested <= range_max {
            // SortDefinitionsForSpanMaxSizeDistribution: non-auto first, then
            // auto, each ascending by max size, stable.
            let mut order: Vec<usize> = (0..span).collect();
            let tracks = &self.tracks[start..start + span];
            order.sort_by(|&left, &right| {
                let left_auto = tracks[left].unit == Unit::Auto;
                let right_auto = tracks[right].unit == Unit::Auto;
                left_auto
                    .cmp(&right_auto)
                    .then(max_sizes[left].total_cmp(&max_sizes[right]))
            });

            let non_auto = span - auto_count;
            let mut to_distribute = requested - range_min;
            for (slot, &offset) in order.iter().enumerate() {
                let track = &mut self.tracks[start + offset];
                let old = track.effective_min;
                let mut new_min = old;
                if slot < non_auto {
                    new_min += to_distribute / (non_auto - slot) as f32;
                } else {
                    new_min += to_distribute / (span - slot) as f32;
                }
                new_min = new_min.min(max_sizes[offset]);
                track.effective_min = old.max(new_min);
                to_distribute -= track.effective_min - old;
                if to_distribute < REAL_EPSILON {
                    break;
                }
            }
            return;
        }

        // Beyond the sum of the max sizes.
        let equally = requested / span as f32;
        if equally < max_max && max_max - equally > REAL_EPSILON {
            let total_remaining = max_max * span as f32 - range_max;
            let to_distribute = requested - range_max;
            for (offset, max_size) in max_sizes.iter().enumerate() {
                let delta = (max_max - max_size) * to_distribute / total_remaining;
                let track = &mut self.tracks[start + offset];
                track.effective_min = track.effective_min.max(max_size + delta);
            }
        } else {
            for track in &mut self.tracks[start..start + span] {
                track.effective_min = track.effective_min.max(equally);
            }
        }
    }

    /// `CGrid::GetFinalSizeForRange`: sum of the final track sizes, without the
    /// spacing (the caller adds `spacing * (span - 1)`).
    pub(super) fn range_size(&self, start: usize, span: usize) -> f32 {
        self.tracks[start..start + span]
            .iter()
            .map(|track| track.measure)
            .sum()
    }

    /// `SetFinalSize`'s final loop: track offsets in definition order, spacing
    /// excluded (the caller adds `spacing * index`).
    pub(super) fn offsets(&self) -> Vec<f32> {
        let mut offsets = Vec::with_capacity(self.tracks.len());
        let mut offset = 0.0;
        for track in &self.tracks {
            offsets.push(offset);
            offset += track.measure;
        }
        offsets
    }
}
