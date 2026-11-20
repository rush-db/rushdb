//! Spatial predicates: `{"location": {"$within": {"center": <GeoJSON
//! Point>, "meters": 5000}}}` (a radius, haversine meters for WGS-84,
//! Euclidean units for Cartesian) and `{"$within": {"box": [<south-west
//! Point>, <north-east Point>]}}` (Neo4j's `point.withinBBox`, a box across
//! the antimeridian included).

use rushdb_types::types::point::{self, Point};
use serde_json::Value as Json;

/// A region `$within` tests points against.
#[derive(Debug, Clone, PartialEq)]
pub enum Region {
    /// Every point within `meters` of `center`.
    Radius {
        /// The center.
        center: Point,
        /// The radius: meters for WGS-84, units for Cartesian.
        meters: f64,
    },
    /// Every point in the box from `low` (south-west) to `high`
    /// (north-east).
    Box {
        /// The south-west corner.
        low: Point,
        /// The north-east corner.
        high: Point,
    },
}

impl Region {
    /// The region a `$within` argument names, or a message saying what is
    /// wrong with it.
    ///
    /// # Errors
    ///
    /// Returns a message for an argument that is not a radius or a box of
    /// `GeoJSON` points of one CRS.
    pub fn parse(value: &Json) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("$within takes {center, meters} or {box: [low, high]}")?;
        let geojson = |value: &Json| {
            point::parse_geojson(value).ok_or("$within: not a GeoJSON point".to_owned())
        };
        if let Some(corners) = object.get("box") {
            let [low, high] = corners
                .as_array()
                .map(Vec::as_slice)
                .and_then(|corners| <&[Json; 2]>::try_from(corners).ok())
                .ok_or("$within.box takes two GeoJSON points [south-west, north-east]")?;
            let (low, high) = (geojson(low)?, geojson(high)?);
            if low.crs != high.crs || low.y > high.y || (!low.crs.geographic() && low.x > high.x) {
                return Err("$within.box: corners of one CRS, south-west first".into());
            }
            return Ok(Self::Box { low, high });
        }
        let center = geojson(
            object
                .get("center")
                .ok_or("$within takes a center or a box")?,
        )?;
        let meters = object
            .get("meters")
            .and_then(Json::as_f64)
            .filter(|meters| meters.is_finite() && *meters >= 0.0)
            .ok_or("$within.meters takes a non-negative number")?;
        Ok(Self::Radius { center, meters })
    }

    /// Whether `point` lies in the region (a point of another CRS never
    /// does).
    #[must_use]
    pub fn contains(&self, at: Point) -> bool {
        match self {
            Self::Radius { center, meters } => {
                point::distance(*center, at).is_some_and(|distance| distance <= *meters)
            }
            Self::Box { low, high } => point::within_box(at, *low, *high).unwrap_or(false),
        }
    }

    /// Whether a document value (a tagged point) lies in the region.
    #[must_use]
    pub fn holds(&self, value: &Json) -> bool {
        match rushdb_types::types::tagged_scalar(value) {
            Some(rushdb_types::value::Scalar::Point(at)) => self.contains(at),
            _ => false,
        }
    }

    /// The region's bounding box `(low, high)` (x, y), which index cell
    /// ranges cover.
    #[must_use]
    pub fn bounds(&self) -> (Point, Point) {
        match self {
            Self::Radius { center, meters } => point::radius_box(*center, *meters),
            Self::Box { low, high } => (*low, *high),
        }
    }

    /// The region's CRS.
    #[must_use]
    pub fn crs(&self) -> point::Crs {
        match self {
            Self::Radius { center, .. } => center.crs,
            Self::Box { low, .. } => low.crs,
        }
    }
}

/// The ordered index entry ranges `[start, end)` (`tag ‖ crs ‖ cell…`)
/// holding every point of `region`: at most about `max_ranges` Hilbert cell
/// ranges of its bounding box. The entries they hold are a superset: each
/// point is checked against the region.
#[must_use]
pub fn entry_ranges(region: &Region, max_ranges: usize) -> Vec<(Vec<u8>, Vec<u8>)> {
    let tag = rushdb_types::model::LogicalType::Point.tag();
    let crs = region.crs();
    let (low, high) = region.bounds();
    point::cover(crs, (low.x, low.y), (high.x, high.y), max_ranges)
        .into_iter()
        .map(|(first, last)| {
            let mut start = vec![tag];
            start.extend_from_slice(&point::cell_prefix(crs, first));
            let end = match last.checked_add(1) {
                Some(next) => {
                    let mut end = vec![tag];
                    end.extend_from_slice(&point::cell_prefix(crs, next));
                    end
                }
                // The last cell: up to the next CRS.
                None => rushdb_storage::prefix_range(&start[..2]).1,
            };
            (start, end)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn regions_parse_and_contain_as_documented() {
        let radius = Region::parse(&json!({
            "center": {"type": "Point", "coordinates": [13.405, 52.52]}, "meters": 1_000_000
        }))
        .unwrap();
        let paris =
            point::parse_geojson(&json!({"type": "Point", "coordinates": [2.3522, 48.8566]}))
                .unwrap();
        assert!(radius.contains(paris));
        let box_across = Region::parse(&json!({"box": [
            {"type": "Point", "coordinates": [170.0, -10.0]},
            {"type": "Point", "coordinates": [-170.0, 10.0]}]}))
        .unwrap();
        assert!(!box_across.contains(paris));
        assert!(
            Region::parse(&json!({"center": {"type": "Point", "coordinates": [0, 0]}})).is_err()
        );
        assert!(Region::parse(&json!({"box": [
            {"type": "Point", "coordinates": [0.0, 10.0]},
            {"type": "Point", "coordinates": [1.0, 0.0]}]}))
        .is_err());
    }
}
