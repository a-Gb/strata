//! Exact, bounded screen-space cohort selection for 3D projection points.
#![allow(clippy::redundant_pub_crate)] // Parent-only helpers live in a separate binary module.

use std::{collections::BTreeSet, ops::Range};

/// Maximum selected members retained in one interaction result.
pub(crate) const MAX_COHORT_MEMBERS: usize = 4_096;

/// A finite, non-empty screen-space selection rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectionRect {
    /// Inclusive left screen coordinate.
    pub(crate) min_x: f32,
    /// Inclusive top screen coordinate.
    pub(crate) min_y: f32,
    /// Inclusive right screen coordinate.
    pub(crate) max_x: f32,
    /// Inclusive bottom screen coordinate.
    pub(crate) max_y: f32,
}

impl SelectionRect {
    /// Validates and constructs a rectangle from two arbitrary drag endpoints.
    pub(crate) fn from_endpoints(
        first_x: f32,
        first_y: f32,
        second_x: f32,
        second_y: f32,
    ) -> Result<Self, CohortError> {
        if !first_x.is_finite()
            || !first_y.is_finite()
            || !second_x.is_finite()
            || !second_y.is_finite()
        {
            return Err(CohortError::NonFiniteRectangle);
        }
        let min_x = first_x.min(second_x);
        let max_x = first_x.max(second_x);
        let min_y = first_y.min(second_y);
        let max_y = first_y.max(second_y);
        if min_x >= max_x || min_y >= max_y {
            return Err(CohortError::EmptyRectangle);
        }
        Ok(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    fn contains(self, screen_x: f32, screen_y: f32) -> bool {
        screen_x.is_finite()
            && screen_y.is_finite()
            && (self.min_x..=self.max_x).contains(&screen_x)
            && (self.min_y..=self.max_y).contains(&screen_y)
    }
}

/// A projection point adapted from the UI's screen-space representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ProjectedMember {
    /// Horizontal screen coordinate in logical pixels.
    pub(crate) screen_x: f32,
    /// Vertical screen coordinate in logical pixels.
    pub(crate) screen_y: f32,
    /// The exact contributing source-byte offsets for this projection point.
    pub(crate) source_offsets: [usize; 3],
    /// The exact half-open source span analyzed to produce this rendered datum.
    pub(crate) source_range: [usize; 2],
}

/// A compact byte-value concentration metric for selected offsets in supplied source bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceByteConcentration {
    /// Most frequent source byte, choosing the lower byte on a tie.
    pub(crate) byte: u8,
    /// Number of selected offsets that contained [`Self::byte`].
    pub(crate) occurrences: usize,
    /// Number of selected offsets readable from the supplied source bytes.
    pub(crate) observed_offsets: usize,
}

/// Explanation values retained alongside an exact cohort selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CohortMetrics {
    /// Number of retained projection members.
    pub(crate) member_count: usize,
    /// Smallest exact byte span that covers all selected contributors.
    pub(crate) source_span: Option<Range<usize>>,
    /// Number of distinct selected source-byte offsets.
    pub(crate) unique_byte_count: usize,
    /// Dominant selected byte when the caller supplied source bytes.
    pub(crate) source_byte_concentration: Option<SourceByteConcentration>,
}

/// An exact screen-space selection plus its compact source explanation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CohortSelection {
    /// Retained members in their stable input order.
    pub(crate) members: Vec<ProjectedMember>,
    /// Sorted, coalesced half-open source byte ranges for every retained member.
    pub(crate) source_ranges: Vec<Range<usize>>,
    /// Compact exact metrics derived from `members` and `source_ranges`.
    pub(crate) metrics: CohortMetrics,
    /// Whether matching members exceeded [`MAX_COHORT_MEMBERS`].
    pub(crate) truncated: bool,
}

/// A clean failure while constructing or explaining a cohort selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CohortError {
    /// A rectangle endpoint was `NaN` or infinite.
    NonFiniteRectangle,
    /// A rectangle had zero or negative width or height.
    EmptyRectangle,
    /// A source offset could not be represented as a half-open one-byte range.
    OffsetOverflow,
}

/// Selects stable projected members and returns exact, bounded source provenance.
///
/// Points with non-finite screen coordinates are ignored. Matching members retain
/// input order until the fixed bound is reached. Duplicate A/B or bit-plane instances
/// collapse to one stable source datum, and exact analyzed ranges are coalesced. When
/// `source_bytes` is present, concentration only considers bytes in those exact ranges.
pub(crate) fn select_cohort(
    rectangle: SelectionRect,
    projected_members: &[ProjectedMember],
    source_bytes: Option<&[u8]>,
) -> Result<CohortSelection, CohortError> {
    let mut members = Vec::with_capacity(projected_members.len().min(MAX_COHORT_MEMBERS));
    let mut identities = BTreeSet::new();
    let mut truncated = false;
    for &member in projected_members {
        if !rectangle.contains(member.screen_x, member.screen_y) {
            continue;
        }
        let identity = (member.source_offsets, member.source_range);
        if !identities.insert(identity) {
            continue;
        }
        if members.len() == MAX_COHORT_MEMBERS {
            truncated = true;
            break;
        }
        members.push(member);
    }

    let source_ranges = coalesce_ranges(&members)?;
    let source_span = match (source_ranges.first(), source_ranges.last()) {
        (Some(first), Some(last)) => Some(first.start..last.end),
        _ => None,
    };
    let unique_byte_count = source_ranges
        .iter()
        .fold(0_usize, |count, range| count.saturating_add(range.len()));
    let source_byte_concentration =
        source_bytes.and_then(|bytes| concentration(bytes, &source_ranges));

    Ok(CohortSelection {
        metrics: CohortMetrics {
            member_count: members.len(),
            source_span,
            unique_byte_count,
            source_byte_concentration,
        },
        members,
        source_ranges,
        truncated,
    })
}

fn coalesce_ranges(members: &[ProjectedMember]) -> Result<Vec<Range<usize>>, CohortError> {
    let mut candidates = Vec::with_capacity(members.len());
    for member in members {
        let [start, end] = member.source_range;
        if start >= end {
            return Err(CohortError::OffsetOverflow);
        }
        candidates.push(start..end);
    }
    candidates.sort_by_key(|range| (range.start, range.end));
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        match ranges.last_mut() {
            Some(last) if candidate.start <= last.end => last.end = last.end.max(candidate.end),
            _ => ranges.push(candidate),
        }
    }
    Ok(ranges)
}

fn concentration(bytes: &[u8], ranges: &[Range<usize>]) -> Option<SourceByteConcentration> {
    let mut counts = [0_usize; 256];
    let mut observed_offsets = 0_usize;
    for range in ranges {
        if let Some(window) = bytes.get(range.clone()) {
            for &byte in window {
                counts[usize::from(byte)] = counts[usize::from(byte)].saturating_add(1);
                observed_offsets = observed_offsets.saturating_add(1);
            }
        }
    }
    if observed_offsets == 0 {
        return None;
    }
    let mut byte = 0_u8;
    let mut occurrences = 0_usize;
    for (index, &count) in counts.iter().enumerate() {
        if count > occurrences {
            byte = u8::try_from(index).ok()?;
            occurrences = count;
        }
    }
    Some(SourceByteConcentration {
        byte,
        occurrences,
        observed_offsets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_rejects_non_finite_and_empty_inputs() {
        assert_eq!(
            SelectionRect::from_endpoints(f32::NAN, 0.0, 1.0, 1.0),
            Err(CohortError::NonFiniteRectangle)
        );
        assert_eq!(
            SelectionRect::from_endpoints(2.0, 1.0, 2.0, 3.0),
            Err(CohortError::EmptyRectangle)
        );
    }

    #[test]
    fn selection_is_stable_and_coalesces_exact_offsets() -> Result<(), CohortError> {
        let rectangle = SelectionRect::from_endpoints(0.0, 0.0, 10.0, 10.0)?;
        let members = [
            ProjectedMember {
                screen_x: 1.0,
                screen_y: 1.0,
                source_offsets: [5, 6, 8],
                source_range: [5, 9],
            },
            ProjectedMember {
                screen_x: 8.0,
                screen_y: 2.0,
                source_offsets: [7, 8, 9],
                source_range: [7, 10],
            },
            ProjectedMember {
                screen_x: 6.0,
                screen_y: 6.0,
                source_offsets: [25, 25, 26],
                source_range: [25, 27],
            },
            ProjectedMember {
                screen_x: 11.0,
                screen_y: 2.0,
                source_offsets: [50, 51, 52],
                source_range: [50, 53],
            },
        ];
        let selection = select_cohort(rectangle, &members, None)?;
        assert_eq!(selection.members, members[..3]);
        assert_eq!(selection.source_ranges, vec![5..10, 25..27]);
        assert_eq!(selection.metrics.member_count, 3);
        assert_eq!(selection.metrics.source_span, Some(5..27));
        assert_eq!(selection.metrics.unique_byte_count, 7);
        assert_eq!(selection.metrics.source_byte_concentration, None);
        assert!(!selection.truncated);
        Ok(())
    }

    #[test]
    fn selection_reports_source_byte_concentration_and_bound() -> Result<(), CohortError> {
        let rectangle = SelectionRect::from_endpoints(0.0, 0.0, 2.0, 2.0)?;
        let members: Vec<_> = (0..MAX_COHORT_MEMBERS.saturating_add(1))
            .map(|offset| ProjectedMember {
                screen_x: 1.0,
                screen_y: 1.0,
                source_offsets: [offset, offset, offset],
                source_range: [offset, offset.saturating_add(1)],
            })
            .collect();
        let mut source = vec![9_u8; MAX_COHORT_MEMBERS.saturating_add(1)];
        source[1..4].fill(4);
        let selection = select_cohort(rectangle, &members, Some(&source))?;
        assert_eq!(selection.members.len(), MAX_COHORT_MEMBERS);
        assert!(selection.truncated);
        assert_eq!(
            selection.metrics.source_byte_concentration,
            Some(SourceByteConcentration {
                byte: 9,
                occurrences: MAX_COHORT_MEMBERS.saturating_sub(3),
                observed_offsets: MAX_COHORT_MEMBERS,
            })
        );
        Ok(())
    }
}
