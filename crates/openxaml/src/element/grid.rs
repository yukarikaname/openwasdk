//! The `Grid` attached properties that are not a row or column index: a child's
//! span and the Grid's inter-track spacing.
//!
//! `CGrid::GetRowSpan`/`GetColumnSpan` clamp the user value to the tracks that
//! remain from the child's first index (`dxaml/xcp/core/core/elements/Grid.cpp`).
//! The accessors return at least one so a zero left in the store cannot make a
//! child disappear.

use super::Element;
use crate::property;
use crate::value::Value;

impl Element {
    pub fn grid_row_span(&self) -> usize {
        super::index_value(self.get(&property::GRID_ROW_SPAN)).max(1)
    }

    pub fn set_grid_row_span(&mut self, span: usize) {
        self.store(&property::GRID_ROW_SPAN, Value::Number(span.max(1) as f32));
    }

    pub fn with_grid_row_span(self, span: usize) -> Self {
        let mut element = self;
        element.set_grid_row_span(span);
        element
    }

    pub fn grid_column_span(&self) -> usize {
        super::index_value(self.get(&property::GRID_COLUMN_SPAN)).max(1)
    }

    pub fn set_grid_column_span(&mut self, span: usize) {
        self.store(
            &property::GRID_COLUMN_SPAN,
            Value::Number(span.max(1) as f32),
        );
    }

    pub fn with_grid_column_span(self, span: usize) -> Self {
        let mut element = self;
        element.set_grid_column_span(span);
        element
    }

    /// `CGrid::GetRowSpacing`: space between rows, never outside them.
    pub fn row_spacing(&self) -> f32 {
        match self.get(&property::GRID_ROW_SPACING) {
            Value::Number(spacing) => spacing.max(0.0),
            _ => 0.0,
        }
    }

    pub fn set_row_spacing(&mut self, spacing: f32) {
        self.store(&property::GRID_ROW_SPACING, Value::Number(spacing.max(0.0)));
    }

    pub fn with_row_spacing(self, spacing: f32) -> Self {
        let mut element = self;
        element.set_row_spacing(spacing);
        element
    }

    /// `CGrid::GetColumnSpacing`: space between columns, never outside them.
    pub fn column_spacing(&self) -> f32 {
        match self.get(&property::GRID_COLUMN_SPACING) {
            Value::Number(spacing) => spacing.max(0.0),
            _ => 0.0,
        }
    }

    pub fn set_column_spacing(&mut self, spacing: f32) {
        self.store(
            &property::GRID_COLUMN_SPACING,
            Value::Number(spacing.max(0.0)),
        );
    }

    pub fn with_column_spacing(self, spacing: f32) -> Self {
        let mut element = self;
        element.set_column_spacing(spacing);
        element
    }
}
