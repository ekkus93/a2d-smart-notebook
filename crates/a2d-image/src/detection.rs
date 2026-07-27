use a2d_domain::A2dError;
use a2d_layout::MarkerRole;
use std::collections::{BTreeMap, BTreeSet};

use crate::error::{processing_error, validation_error};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkerFamily {
    TagStandard41h12,
}

impl MarkerFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TagStandard41h12 => "tagStandard41h12",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePoint {
    pub x: f64,
    pub y: f64,
}

impl ImagePoint {
    pub(crate) fn from_array(value: [f64; 2]) -> Result<Self, A2dError> {
        if !value[0].is_finite() || !value[1].is_finite() {
            return Err(processing_error(
                "MARKER_DETECTION_NON_FINITE_GEOMETRY",
                "native detector returned a non-finite image point",
                false,
            ));
        }
        Ok(Self {
            x: value[0],
            y: value[1],
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkerDetection {
    pub family: MarkerFamily,
    pub id: u32,
    pub hamming_errors: u8,
    pub decision_margin: f32,
    pub center: ImagePoint,
    /// Counter-clockwise detector corners, preserving the native result.
    pub corners: [ImagePoint; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageOrientation {
    Degrees0,
    Degrees90,
    Degrees180,
    Degrees270,
}

impl PageOrientation {
    pub const fn degrees(self) -> u16 {
        match self {
            Self::Degrees0 => 0,
            Self::Degrees90 => 90,
            Self::Degrees180 => 180,
            Self::Degrees270 => 270,
        }
    }
}

/// Mapping from printed tag IDs to page-corner semantics for one layout
/// version. The constructor rejects duplicate IDs and duplicate roles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarkerIdLayout {
    assignments: BTreeMap<u32, MarkerRole>,
}

impl MarkerIdLayout {
    pub fn new(assignments: [(u32, MarkerRole); 4]) -> Result<Self, A2dError> {
        let mut by_id = BTreeMap::new();
        let mut roles = BTreeSet::new();
        for (id, role) in assignments {
            if by_id.insert(id, role).is_some() {
                return Err(validation_error(
                    "MARKER_LAYOUT_DUPLICATE_TAG_ID",
                    format!("marker layout assigns tag ID {id} more than once"),
                ));
            }
            if !roles.insert(role.as_id_str()) {
                return Err(validation_error(
                    "MARKER_LAYOUT_DUPLICATE_ROLE",
                    format!(
                        "marker layout assigns role {} more than once",
                        role.as_id_str()
                    ),
                ));
            }
        }
        for expected in MarkerRole::ALL {
            if !roles.contains(expected.as_id_str()) {
                return Err(validation_error(
                    "MARKER_LAYOUT_MISSING_ROLE",
                    format!(
                        "marker layout is missing role {}",
                        expected.as_id_str()
                    ),
                ));
            }
        }
        Ok(Self { assignments: by_id })
    }

    pub fn role_for(&self, id: u32) -> Option<MarkerRole> {
        self.assignments.get(&id).copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedMarker {
    pub role: MarkerRole,
    pub detection: MarkerDetection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPageMarkers {
    pub markers: [ResolvedMarker; 4],
    pub orientation: PageOrientation,
    /// Detected tags that are not part of the selected page layout. They are
    /// preserved for diagnostics/review rather than silently discarded.
    pub unexpected_tag_ids: Vec<u32>,
}

impl ResolvedPageMarkers {
    pub fn marker(&self, role: MarkerRole) -> &MarkerDetection {
        &self
            .markers
            .iter()
            .find(|marker| marker.role == role)
            .expect("constructor guarantees every role")
            .detection
    }
}

pub fn resolve_page_markers(
    detections: &[MarkerDetection],
    layout: &MarkerIdLayout,
) -> Result<ResolvedPageMarkers, A2dError> {
    let mut seen_ids = BTreeSet::new();
    let mut by_role = BTreeMap::new();
    let mut unexpected_tag_ids = Vec::new();

    for detection in detections {
        if detection.family != MarkerFamily::TagStandard41h12 {
            return Err(processing_error(
                "MARKER_FAMILY_UNEXPECTED",
                format!(
                    "expected tagStandard41h12 but detector returned {}",
                    detection.family.as_str()
                ),
                false,
            ));
        }
        if !seen_ids.insert(detection.id) {
            return Err(processing_error(
                "MARKER_DETECTION_DUPLICATE_TAG_ID",
                format!("tag ID {} was detected more than once", detection.id),
                true,
            ));
        }

        let Some(role) = layout.role_for(detection.id) else {
            unexpected_tag_ids.push(detection.id);
            continue;
        };
        if by_role
            .insert(role.as_id_str(), (role, detection.clone()))
            .is_some()
        {
            return Err(processing_error(
                "MARKER_DETECTION_DUPLICATE_ROLE",
                format!(
                    "more than one detection resolved to role {}",
                    role.as_id_str()
                ),
                true,
            ));
        }
    }

    let get = |role: MarkerRole| -> Result<ResolvedMarker, A2dError> {
        by_role
            .get(role.as_id_str())
            .cloned()
            .map(|(role, detection)| ResolvedMarker { role, detection })
            .ok_or_else(|| {
                processing_error(
                    "MARKER_DETECTION_MISSING_ROLE",
                    format!(
                        "no detection resolved to role {}",
                        role.as_id_str()
                    ),
                    true,
                )
            })
    };

    let top_left = get(MarkerRole::TopLeft)?;
    let top_right = get(MarkerRole::TopRight)?;
    let bottom_left = get(MarkerRole::BottomLeft)?;
    let bottom_right = get(MarkerRole::BottomRight)?;

    let orientation = orientation_from_top_edge(
        top_left.detection.center,
        top_right.detection.center,
    )?;

    unexpected_tag_ids.sort_unstable();
    Ok(ResolvedPageMarkers {
        markers: [top_left, top_right, bottom_left, bottom_right],
        orientation,
        unexpected_tag_ids,
    })
}

fn orientation_from_top_edge(
    top_left: ImagePoint,
    top_right: ImagePoint,
) -> Result<PageOrientation, A2dError> {
    let dx = top_right.x - top_left.x;
    let dy = top_right.y - top_left.y;
    if !dx.is_finite() || !dy.is_finite() || (dx == 0.0 && dy == 0.0) {
        return Err(processing_error(
            "MARKER_ORIENTATION_UNRESOLVABLE",
            "top-left and top-right marker centers do not define a finite edge",
            true,
        ));
    }

    Ok(if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            PageOrientation::Degrees0
        } else {
            PageOrientation::Degrees180
        }
    } else if dy >= 0.0 {
        PageOrientation::Degrees90
    } else {
        PageOrientation::Degrees270
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detection(id: u32, x: f64, y: f64) -> MarkerDetection {
        MarkerDetection {
            family: MarkerFamily::TagStandard41h12,
            id,
            hamming_errors: 0,
            decision_margin: 50.0,
            center: ImagePoint { x, y },
            corners: [
                ImagePoint {
                    x: x - 1.0,
                    y: y - 1.0,
                },
                ImagePoint {
                    x: x + 1.0,
                    y: y - 1.0,
                },
                ImagePoint {
                    x: x + 1.0,
                    y: y + 1.0,
                },
                ImagePoint {
                    x: x - 1.0,
                    y: y + 1.0,
                },
            ],
        }
    }

    fn layout() -> MarkerIdLayout {
        MarkerIdLayout::new([
            (10, MarkerRole::TopLeft),
            (11, MarkerRole::TopRight),
            (12, MarkerRole::BottomRight),
            (13, MarkerRole::BottomLeft),
        ])
        .unwrap()
    }

    #[test]
    fn resolves_all_roles_and_preserves_unexpected_tags() {
        let resolved = resolve_page_markers(
            &[
                detection(10, 10.0, 10.0),
                detection(11, 90.0, 10.0),
                detection(12, 90.0, 90.0),
                detection(13, 10.0, 90.0),
                detection(99, 50.0, 50.0),
            ],
            &layout(),
        )
        .unwrap();

        assert_eq!(resolved.orientation, PageOrientation::Degrees0);
        assert_eq!(resolved.unexpected_tag_ids, vec![99]);
        assert_eq!(resolved.marker(MarkerRole::BottomRight).id, 12);
    }

    #[test]
    fn rejects_duplicate_detected_ids() {
        let err = resolve_page_markers(
            &[
                detection(10, 10.0, 10.0),
                detection(10, 11.0, 11.0),
                detection(11, 90.0, 10.0),
                detection(12, 90.0, 90.0),
                detection(13, 10.0, 90.0),
            ],
            &layout(),
        )
        .unwrap_err();
        assert_eq!(
            err.code.to_string(),
            "MARKER_DETECTION_DUPLICATE_TAG_ID"
        );
    }

    #[test]
    fn rejects_missing_expected_role() {
        let err = resolve_page_markers(
            &[
                detection(10, 10.0, 10.0),
                detection(11, 90.0, 10.0),
                detection(12, 90.0, 90.0),
            ],
            &layout(),
        )
        .unwrap_err();
        assert_eq!(err.code.to_string(), "MARKER_DETECTION_MISSING_ROLE");
    }

    #[test]
    fn identifies_cardinal_page_rotations_from_the_semantic_top_edge() {
        let cases = [
            (
                ImagePoint { x: 0.0, y: 0.0 },
                ImagePoint { x: 10.0, y: 0.0 },
                PageOrientation::Degrees0,
            ),
            (
                ImagePoint { x: 0.0, y: 0.0 },
                ImagePoint { x: 0.0, y: 10.0 },
                PageOrientation::Degrees90,
            ),
            (
                ImagePoint { x: 10.0, y: 0.0 },
                ImagePoint { x: 0.0, y: 0.0 },
                PageOrientation::Degrees180,
            ),
            (
                ImagePoint { x: 0.0, y: 10.0 },
                ImagePoint { x: 0.0, y: 0.0 },
                PageOrientation::Degrees270,
            ),
        ];

        for (left, right, expected) in cases {
            assert_eq!(orientation_from_top_edge(left, right).unwrap(), expected);
        }
    }

    #[test]
    fn layout_rejects_duplicate_tag_ids() {
        let err = MarkerIdLayout::new([
            (10, MarkerRole::TopLeft),
            (10, MarkerRole::TopRight),
            (12, MarkerRole::BottomRight),
            (13, MarkerRole::BottomLeft),
        ])
        .unwrap_err();
        assert_eq!(err.code.to_string(), "MARKER_LAYOUT_DUPLICATE_TAG_ID");
    }
}
