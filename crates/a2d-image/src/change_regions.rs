use std::collections::VecDeque;

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

use crate::fingerprint::{
    PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT, PERCEPTUAL_FINGERPRINT_V1_HEIGHT,
    PERCEPTUAL_FINGERPRINT_V1_WIDTH, PerceptualFingerprintDifference,
};

/// Explicit segmentation policy for the canonical aligned fingerprint grid.
///
/// The threshold is inclusive and must be nonzero. No default is provided because a production
/// threshold must be selected from photographed-fixture evidence rather than embedded in the image
/// layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlignedChangeRegionConfig {
    minimum_cell_absolute_difference: u8,
}

impl AlignedChangeRegionConfig {
    pub fn new(minimum_cell_absolute_difference: u8) -> Result<Self, A2dError> {
        if minimum_cell_absolute_difference == 0 {
            return Err(change_region_error(
                "IMAGE_FINGERPRINT_CHANGE_THRESHOLD_INVALID",
                "aligned change-region threshold must be greater than zero",
            ));
        }
        Ok(Self {
            minimum_cell_absolute_difference,
        })
    }

    pub fn minimum_cell_absolute_difference(self) -> u8 {
        self.minimum_cell_absolute_difference
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlignedChangeCell {
    column: usize,
    row: usize,
    absolute_difference: u8,
}

impl AlignedChangeCell {
    pub fn column(&self) -> usize {
        self.column
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn absolute_difference(&self) -> u8 {
        self.absolute_difference
    }
}

/// One four-neighbor connected component in the canonical upright 16x24 grid.
///
/// Bounds are half-open grid coordinates. `cells` preserves the exact changed shape so consumers do
/// not have to treat unchanged cells inside a bounding rectangle as changed.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignedChangeRegion {
    left_column: usize,
    top_row: usize,
    right_column_exclusive: usize,
    bottom_row_exclusive: usize,
    changed_cell_count: usize,
    mean_absolute_difference: f32,
    maximum_absolute_difference: u8,
    cells: Vec<AlignedChangeCell>,
}

impl AlignedChangeRegion {
    pub fn left_column(&self) -> usize {
        self.left_column
    }

    pub fn top_row(&self) -> usize {
        self.top_row
    }

    pub fn right_column_exclusive(&self) -> usize {
        self.right_column_exclusive
    }

    pub fn bottom_row_exclusive(&self) -> usize {
        self.bottom_row_exclusive
    }

    pub fn width_cells(&self) -> usize {
        self.right_column_exclusive - self.left_column
    }

    pub fn height_cells(&self) -> usize {
        self.bottom_row_exclusive - self.top_row
    }

    pub fn changed_cell_count(&self) -> usize {
        self.changed_cell_count
    }

    pub fn mean_absolute_difference(&self) -> f32 {
        self.mean_absolute_difference
    }

    pub fn maximum_absolute_difference(&self) -> u8 {
        self.maximum_absolute_difference
    }

    pub fn cells(&self) -> &[AlignedChangeCell] {
        &self.cells
    }
}

/// Threshold provenance and deterministic regions for one aligned fingerprint comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignedChangeRegionComparison {
    minimum_cell_absolute_difference: u8,
    changed_cell_count: usize,
    regions: Vec<AlignedChangeRegion>,
}

impl AlignedChangeRegionComparison {
    pub fn minimum_cell_absolute_difference(&self) -> u8 {
        self.minimum_cell_absolute_difference
    }

    pub fn changed_cell_count(&self) -> usize {
        self.changed_cell_count
    }

    pub fn regions(&self) -> &[AlignedChangeRegion] {
        &self.regions
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

impl PerceptualFingerprintDifference {
    /// Groups threshold-matching cells using four-neighbor connectivity.
    ///
    /// Regions and their member cells are returned in deterministic row-major order. Diagonal-only
    /// cells remain separate regions, and no minimum region-size filter is applied silently.
    pub fn aligned_change_regions(
        &self,
        config: AlignedChangeRegionConfig,
    ) -> Result<AlignedChangeRegionComparison, A2dError> {
        if self.cell_absolute_differences.len() != PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT {
            return Err(change_region_error(
                "IMAGE_FINGERPRINT_DIFFERENCE_CELL_COUNT_INVALID",
                format!(
                    "aligned difference grid requires {} cells, got {}",
                    PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT,
                    self.cell_absolute_differences.len()
                ),
            ));
        }

        let threshold = config.minimum_cell_absolute_difference();
        let mut visited = vec![false; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT];
        let mut regions = Vec::new();

        for start_index in 0..PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT {
            if visited[start_index] || self.cell_absolute_differences[start_index] < threshold {
                continue;
            }

            visited[start_index] = true;
            let mut pending = VecDeque::new();
            pending.push_back(start_index);
            let mut cells = Vec::new();
            let mut left_column = PERCEPTUAL_FINGERPRINT_V1_WIDTH;
            let mut top_row = PERCEPTUAL_FINGERPRINT_V1_HEIGHT;
            let mut right_column_exclusive = 0;
            let mut bottom_row_exclusive = 0;
            let mut total_difference = 0_u64;
            let mut maximum_absolute_difference = 0_u8;

            while let Some(index) = pending.pop_front() {
                let row = index / PERCEPTUAL_FINGERPRINT_V1_WIDTH;
                let column = index % PERCEPTUAL_FINGERPRINT_V1_WIDTH;
                let absolute_difference = self.cell_absolute_differences[index];

                left_column = left_column.min(column);
                top_row = top_row.min(row);
                right_column_exclusive = right_column_exclusive.max(column + 1);
                bottom_row_exclusive = bottom_row_exclusive.max(row + 1);
                total_difference += u64::from(absolute_difference);
                maximum_absolute_difference = maximum_absolute_difference.max(absolute_difference);
                cells.push(AlignedChangeCell {
                    column,
                    row,
                    absolute_difference,
                });

                let mut enqueue_if_changed = |neighbor: usize| {
                    if !visited[neighbor]
                        && self.cell_absolute_differences[neighbor] >= threshold
                    {
                        visited[neighbor] = true;
                        pending.push_back(neighbor);
                    }
                };
                if column > 0 {
                    enqueue_if_changed(index - 1);
                }
                if column + 1 < PERCEPTUAL_FINGERPRINT_V1_WIDTH {
                    enqueue_if_changed(index + 1);
                }
                if row > 0 {
                    enqueue_if_changed(index - PERCEPTUAL_FINGERPRINT_V1_WIDTH);
                }
                if row + 1 < PERCEPTUAL_FINGERPRINT_V1_HEIGHT {
                    enqueue_if_changed(index + PERCEPTUAL_FINGERPRINT_V1_WIDTH);
                }
            }

            cells.sort_unstable_by_key(|cell| (cell.row, cell.column));
            let changed_cell_count = cells.len();
            regions.push(AlignedChangeRegion {
                left_column,
                top_row,
                right_column_exclusive,
                bottom_row_exclusive,
                changed_cell_count,
                mean_absolute_difference: total_difference as f32 / changed_cell_count as f32,
                maximum_absolute_difference,
                cells,
            });
        }

        let changed_cell_count = regions
            .iter()
            .map(AlignedChangeRegion::changed_cell_count)
            .sum();
        Ok(AlignedChangeRegionComparison {
            minimum_cell_absolute_difference: threshold,
            changed_cell_count,
            regions,
        })
    }
}

fn change_region_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.image.fingerprint",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn difference_with_changes(changes: &[(usize, usize, u8)]) -> PerceptualFingerprintDifference {
        let mut cell_absolute_differences = vec![0_u8; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT];
        for &(column, row, difference) in changes {
            cell_absolute_differences[row * PERCEPTUAL_FINGERPRINT_V1_WIDTH + column] = difference;
        }
        let total = cell_absolute_differences
            .iter()
            .map(|difference| u64::from(*difference))
            .sum::<u64>();
        let maximum_absolute_difference =
            cell_absolute_differences.iter().copied().max().unwrap_or(0);
        PerceptualFingerprintDifference {
            mean_absolute_difference: total as f32 / PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT as f32,
            maximum_absolute_difference,
            cell_absolute_differences,
        }
    }

    #[test]
    fn zero_change_threshold_is_rejected_instead_of_matching_unchanged_cells() {
        let error = AlignedChangeRegionConfig::new(0).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "IMAGE_FINGERPRINT_CHANGE_THRESHOLD_INVALID"
        );
    }

    #[test]
    fn separated_changes_become_deterministically_ordered_regions() {
        let comparison = difference_with_changes(&[(2, 3, 40), (12, 20, 120)])
            .aligned_change_regions(AlignedChangeRegionConfig::new(30).unwrap())
            .unwrap();
        let regions = comparison.regions();

        assert_eq!(comparison.minimum_cell_absolute_difference(), 30);
        assert_eq!(comparison.changed_cell_count(), 2);
        assert_eq!(regions.len(), 2);
        assert_eq!((regions[0].left_column(), regions[0].top_row()), (2, 3));
        assert_eq!(regions[0].maximum_absolute_difference(), 40);
        assert_eq!((regions[1].left_column(), regions[1].top_row()), (12, 20));
        assert_eq!(regions[1].maximum_absolute_difference(), 120);
    }

    #[test]
    fn four_connected_l_shape_preserves_exact_cells_and_statistics() {
        let comparison = difference_with_changes(&[(4, 5, 50), (5, 5, 100), (4, 6, 10)])
            .aligned_change_regions(AlignedChangeRegionConfig::new(10).unwrap())
            .unwrap();
        let region = &comparison.regions()[0];

        assert_eq!(comparison.changed_cell_count(), 3);
        assert_eq!((region.left_column(), region.top_row()), (4, 5));
        assert_eq!(
            (region.right_column_exclusive(), region.bottom_row_exclusive()),
            (6, 7)
        );
        assert_eq!(region.width_cells(), 2);
        assert_eq!(region.height_cells(), 2);
        assert_eq!(region.changed_cell_count(), 3);
        assert_eq!(region.maximum_absolute_difference(), 100);
        assert!((region.mean_absolute_difference() - 160.0 / 3.0).abs() < f32::EPSILON);
        assert_eq!(
            region.cells(),
            &[
                AlignedChangeCell {
                    column: 4,
                    row: 5,
                    absolute_difference: 50,
                },
                AlignedChangeCell {
                    column: 5,
                    row: 5,
                    absolute_difference: 100,
                },
                AlignedChangeCell {
                    column: 4,
                    row: 6,
                    absolute_difference: 10,
                },
            ]
        );
    }

    #[test]
    fn threshold_is_inclusive_and_diagonal_or_wrapped_cells_remain_separate() {
        let comparison = difference_with_changes(&[
            (7, 8, 10),
            (8, 9, 10),
            (10, 10, 9),
            (15, 12, 20),
            (0, 13, 20),
        ])
        .aligned_change_regions(AlignedChangeRegionConfig::new(10).unwrap())
        .unwrap();
        let coordinates = comparison
            .regions()
            .iter()
            .map(|region| (region.left_column(), region.top_row()))
            .collect::<Vec<_>>();

        assert_eq!(comparison.changed_cell_count(), 4);
        assert_eq!(coordinates, vec![(7, 8), (8, 9), (15, 12), (0, 13)]);
    }

    #[test]
    fn identical_and_malformed_difference_grids_are_explicit() {
        let identical = difference_with_changes(&[])
            .aligned_change_regions(AlignedChangeRegionConfig::new(1).unwrap())
            .unwrap();
        assert!(identical.is_empty());
        assert_eq!(identical.changed_cell_count(), 0);

        let malformed = PerceptualFingerprintDifference {
            mean_absolute_difference: 1.0,
            maximum_absolute_difference: 1,
            cell_absolute_differences: vec![1; PERCEPTUAL_FINGERPRINT_V1_CELL_COUNT - 1],
        };
        let error = malformed
            .aligned_change_regions(AlignedChangeRegionConfig::new(1).unwrap())
            .unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "IMAGE_FINGERPRINT_DIFFERENCE_CELL_COUNT_INVALID"
        );
    }
}
