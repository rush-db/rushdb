//! The `point` type: a 2D or 3D point in WGS-84 (longitude, latitude,
//! height) or in a Cartesian plane, with the SRIDs Neo4j uses (4326, 4979,
//! 7203, 9157). Literals are GeoJSON points:
//! `{"type": "Point", "coordinates": [lon, lat]}`, a third coordinate for
//! 3D, and a `crs` member naming `cartesian` for the Cartesian plane.
//!
//! A point's ordered payload starts with its CRS and a Hilbert cell of x
//! and y, so an ordered index on a point property keeps nearby points in
//! nearby entries: a box or radius query reads a few ranges of cells
//! ([`cover`]) and checks the points it finds exactly.

use serde_json::{json, Map, Value as Json};

/// A point's coordinate reference system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Crs {
    /// WGS-84 longitude and latitude (SRID 4326).
    Wgs84,
    /// WGS-84 longitude, latitude and height in meters (SRID 4979).
    Wgs84_3d,
    /// A Cartesian plane (SRID 7203).
    Cartesian,
    /// Cartesian space (SRID 9157).
    Cartesian3d,
}

impl Crs {
    /// The CRS's SRID.
    #[must_use]
    pub const fn srid(self) -> u32 {
        match self {
            Crs::Wgs84 => 4_326,
            Crs::Wgs84_3d => 4_979,
            Crs::Cartesian => 7_203,
            Crs::Cartesian3d => 9_157,
        }
    }

    /// The CRS of an SRID.
    #[must_use]
    pub const fn of_srid(srid: u32) -> Option<Self> {
        Some(match srid {
            4_326 => Crs::Wgs84,
            4_979 => Crs::Wgs84_3d,
            7_203 => Crs::Cartesian,
            9_157 => Crs::Cartesian3d,
            _ => return None,
        })
    }

    /// The CRS's name, as Neo4j names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Crs::Wgs84 => "wgs-84",
            Crs::Wgs84_3d => "wgs-84-3d",
            Crs::Cartesian => "cartesian",
            Crs::Cartesian3d => "cartesian-3d",
        }
    }

    /// The CRS a name or `EPSG:<srid>` names.
    #[must_use]
    pub fn of_name(name: &str) -> Option<Self> {
        let name = name.trim().to_ascii_lowercase();
        if let Some(srid) = name.strip_prefix("epsg:") {
            return Self::of_srid(srid.parse().ok()?);
        }
        [Crs::Wgs84, Crs::Wgs84_3d, Crs::Cartesian, Crs::Cartesian3d]
            .into_iter()
            .find(|crs| crs.name() == name)
    }

    /// Whether the CRS has a third coordinate.
    #[must_use]
    pub const fn three_d(self) -> bool {
        matches!(self, Crs::Wgs84_3d | Crs::Cartesian3d)
    }

    /// Whether the CRS is geographic (WGS-84).
    #[must_use]
    pub const fn geographic(self) -> bool {
        matches!(self, Crs::Wgs84 | Crs::Wgs84_3d)
    }

    const fn code(self) -> u8 {
        match self {
            Crs::Wgs84 => 1,
            Crs::Wgs84_3d => 2,
            Crs::Cartesian => 3,
            Crs::Cartesian3d => 4,
        }
    }

    const fn of_code(code: u8) -> Option<Self> {
        Some(match code {
            1 => Crs::Wgs84,
            2 => Crs::Wgs84_3d,
            3 => Crs::Cartesian,
            4 => Crs::Cartesian3d,
            _ => return None,
        })
    }
}

/// A point. Coordinates are finite; for WGS-84, `x` is the longitude in
/// [-180, 180] and `y` the latitude in [-90, 90].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// The coordinate reference system.
    pub crs: Crs,
    /// x, or the longitude.
    pub x: f64,
    /// y, or the latitude.
    pub y: f64,
    /// z, or the height, in a 3D CRS.
    pub z: Option<f64>,
}

impl Point {
    /// A point, checked: finite coordinates, a third one exactly in a 3D
    /// CRS, and WGS-84 coordinates in range.
    #[must_use]
    pub fn new(crs: Crs, x: f64, y: f64, z: Option<f64>) -> Option<Self> {
        let finite = x.is_finite() && y.is_finite() && z.is_none_or(f64::is_finite);
        let in_range =
            !crs.geographic() || ((-180.0..=180.0).contains(&x) && (-90.0..=90.0).contains(&y));
        (finite && in_range && z.is_some() == crs.three_d()).then_some(Self { crs, x, y, z })
    }
}

/// Parse a `GeoJSON` point. `crs` may be a CRS name (`"cartesian"`), the
/// `GeoJSON` 2008 named form (`{"type": "name", "properties": {"name":
/// "EPSG:7203"}}`), or absent (WGS-84). Members other than `type`,
/// `coordinates` and `crs` make the object not a point.
#[must_use]
pub fn parse_geojson(value: &Json) -> Option<Point> {
    let object = value.as_object()?;
    if object.get("type")?.as_str()? != "Point"
        || object
            .keys()
            .any(|key| !matches!(key.as_str(), "type" | "coordinates" | "crs"))
    {
        return None;
    }
    let coordinates: Vec<f64> = object
        .get("coordinates")?
        .as_array()?
        .iter()
        .map(Json::as_f64)
        .collect::<Option<_>>()?;
    let named = match object.get("crs") {
        None => None,
        Some(Json::String(name)) => Some(Crs::of_name(name)?),
        Some(Json::Object(crs)) => {
            Some(Crs::of_name(crs.get("properties")?.get("name")?.as_str()?)?)
        }
        Some(_) => return None,
    };
    // Without a CRS a point is WGS-84, 2D or 3D by its coordinates; a
    // named CRS must have as many coordinates as it has axes.
    let crs = match (named, coordinates.len()) {
        (None, 2) => Crs::Wgs84,
        (None, 3) => Crs::Wgs84_3d,
        (Some(crs), 2) if !crs.three_d() => crs,
        (Some(crs), 3) if crs.three_d() => crs,
        _ => return None,
    };
    Point::new(
        crs,
        coordinates[0],
        coordinates[1],
        coordinates.get(2).copied(),
    )
}

/// A point as `GeoJSON`, naming its CRS only when it is Cartesian.
#[must_use]
pub fn to_geojson(point: Point) -> Json {
    let mut coordinates = vec![json!(point.x), json!(point.y)];
    if let Some(z) = point.z {
        coordinates.push(json!(z));
    }
    let mut object = Map::new();
    object.insert("type".into(), json!("Point"));
    object.insert("coordinates".into(), Json::Array(coordinates));
    if !point.crs.geographic() {
        object.insert(
            "crs".into(),
            json!({"type": "name", "properties": {"name": point.crs.name()}}),
        );
    }
    Json::Object(object)
}

/// Mean Earth radius in meters, as Neo4j's `point.distance` uses.
pub const EARTH_RADIUS_METERS: f64 = 6_378_140.0;

/// The distance between two points of one CRS: haversine meters for WGS-84
/// (height ignored, as Neo4j does for 2D; 3D adds the height difference
/// Euclidean), Euclidean for Cartesian. `None` across CRSs.
#[must_use]
pub fn distance(a: Point, b: Point) -> Option<f64> {
    if a.crs != b.crs {
        return None;
    }
    if a.crs.geographic() {
        let (lat1, lat2) = (a.y.to_radians(), b.y.to_radians());
        let dlat = lat2 - lat1;
        let dlon = (b.x - a.x).to_radians();
        let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
        let surface = 2.0 * EARTH_RADIUS_METERS * h.sqrt().min(1.0).asin();
        return Some(match (a.z, b.z) {
            (Some(za), Some(zb)) => surface.hypot(za - zb),
            _ => surface,
        });
    }
    let planar = (a.x - b.x).hypot(a.y - b.y);
    Some(match (a.z, b.z) {
        (Some(za), Some(zb)) => planar.hypot(za - zb),
        _ => planar,
    })
}

/// Whether `point` lies in the box from `low` (south-west) to `high`
/// (north-east), edges included, as Neo4j's `point.withinBBox`: for
/// WGS-84 a box whose west edge is east of its east edge crosses the
/// antimeridian. `None` across CRSs.
#[must_use]
pub fn within_box(point: Point, low: Point, high: Point) -> Option<bool> {
    if point.crs != low.crs || point.crs != high.crs {
        return None;
    }
    let y = (low.y..=high.y).contains(&point.y);
    let x = if point.crs.geographic() && low.x > high.x {
        point.x >= low.x || point.x <= high.x
    } else {
        (low.x..=high.x).contains(&point.x)
    };
    let z = match (point.z, low.z, high.z) {
        (Some(z), Some(lz), Some(hz)) => (lz..=hz).contains(&z),
        _ => true,
    };
    Some(x && y && z)
}

/// Grid cells per axis of the Hilbert curve: 2^`ORDER`.
const ORDER: u32 = 32;

/// A coordinate's cell on its axis: linear over the WGS-84 range, and over
/// a total-order mapping of the float for Cartesian (any f64, denser near
/// zero), which keeps a box of coordinates a box of cells.
fn axis_cell(crs: Crs, value: f64, low: f64, high: f64) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the scaled value is clamped to [0, 2^32 - 1]"
    )]
    if crs.geographic() {
        let unit = ((value - low) / (high - low)).clamp(0.0, 1.0);
        return (unit * f64::from(u32::MAX)).round() as u32;
    }
    let bits = value.to_bits();
    let ordered = if bits >> 63 == 0 {
        bits | (1 << 63)
    } else {
        !bits
    };
    (ordered >> 32) as u32
}

/// The cell of `point`.
fn cell_xy(point: Point) -> (u32, u32) {
    (
        axis_cell(point.crs, point.x, -180.0, 180.0),
        axis_cell(point.crs, point.y, -90.0, 90.0),
    )
}

/// The Hilbert index of cell `(x, y)` on a 2^32 grid.
#[must_use]
pub fn hilbert_index(x: u32, y: u32) -> u64 {
    let last = u64::from(u32::MAX);
    let (mut x, mut y) = (u64::from(x), u64::from(y));
    let mut d = 0u64;
    let mut s = 1u64 << (ORDER - 1);
    while s > 0 {
        let rx = u64::from(x & s > 0);
        let ry = u64::from(y & s > 0);
        d += s * s * ((3 * rx) ^ ry);
        // Rotate the quadrant (within the whole grid).
        if ry == 0 {
            if rx == 1 {
                x = last - x;
                y = last - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s >>= 1;
    }
    d
}

/// Order-preserving f64 bytes (sign bit flipped, negatives inverted).
fn float_bytes(value: f64) -> [u8; 8] {
    let bits = value.to_bits();
    let ordered = if bits >> 63 == 0 {
        bits | (1 << 63)
    } else {
        !bits
    };
    ordered.to_be_bytes()
}

fn float_of(bytes: &[u8]) -> Option<f64> {
    let raw = u64::from_be_bytes(bytes.try_into().ok()?);
    let bits = if raw >> 63 == 1 {
        raw & !(1 << 63)
    } else {
        !raw
    };
    Some(f64::from_bits(bits))
}

/// The ordered payload of a point: `crs ‖ hilbert cell ‖ x ‖ y [‖ z]`.
#[must_use]
pub fn encode(point: Point) -> Vec<u8> {
    let (cx, cy) = cell_xy(point);
    let mut out = Vec::with_capacity(33);
    out.push(point.crs.code());
    out.extend_from_slice(&hilbert_index(cx, cy).to_be_bytes());
    out.extend_from_slice(&float_bytes(point.x));
    out.extend_from_slice(&float_bytes(point.y));
    if let Some(z) = point.z {
        out.extend_from_slice(&float_bytes(z));
    }
    out
}

/// Decode [`encode`]'s payload.
#[must_use]
pub fn decode(bytes: &[u8]) -> Option<Point> {
    let (&code, rest) = bytes.split_first()?;
    let crs = Crs::of_code(code)?;
    let expected = if crs.three_d() { 32 } else { 24 };
    if rest.len() != expected {
        return None;
    }
    let x = float_of(&rest[8..16])?;
    let y = float_of(&rest[16..24])?;
    let z = if crs.three_d() {
        Some(float_of(&rest[24..32])?)
    } else {
        None
    };
    let point = Point::new(crs, x, y, z)?;
    (encode(point) == bytes).then_some(point)
}

/// The cell range `[start, end]` prefix bytes (`crs ‖ cell`) of entries.
#[must_use]
pub fn cell_prefix(crs: Crs, cell: u64) -> [u8; 9] {
    let mut out = [0u8; 9];
    out[0] = crs.code();
    out[1..].copy_from_slice(&cell.to_be_bytes());
    out
}

/// At most about `max_ranges` inclusive ranges of Hilbert cells covering
/// every point of `crs` in the box `low`–`high` (x, y only; an antimeridian
/// box is two boxes). Points in the ranges but outside the box are possible:
/// callers check each point exactly.
#[must_use]
pub fn cover(crs: Crs, low: (f64, f64), high: (f64, f64), max_ranges: usize) -> Vec<(u64, u64)> {
    if crs.geographic() && low.0 > high.0 {
        let mut ranges = cover(crs, low, (180.0, high.1), max_ranges / 2);
        ranges.extend(cover(crs, (-180.0, low.1), high, max_ranges / 2));
        return merge(ranges);
    }
    let corner = |x: f64, y: f64| cell_xy(Point { crs, x, y, z: None });
    let (x0, y0) = corner(low.0, low.1);
    let (x1, y1) = corner(high.0, high.1);
    let mut ranges = Vec::new();
    // Descend the curve's quadtree: a square inside the box is one range,
    // one outside is skipped, one crossing it is split until the budget of
    // ranges is spent (then taken whole).
    let mut squares = vec![(0u64, 0u64, ORDER)];
    while let Some((qx, qy, level)) = squares.pop() {
        let side = 1u64 << level;
        let (ax, ay, bx, by) = (qx, qy, qx + side - 1, qy + side - 1);
        if bx < u64::from(x0) || ax > u64::from(x1) || by < u64::from(y0) || ay > u64::from(y1) {
            continue;
        }
        let inside = ax >= u64::from(x0)
            && bx <= u64::from(x1)
            && ay >= u64::from(y0)
            && by <= u64::from(y1);
        if inside || level == 0 || ranges.len() + squares.len() >= max_ranges.max(1) * 4 {
            if level == ORDER {
                ranges.push((0, u64::MAX));
                continue;
            }
            #[expect(clippy::cast_possible_truncation, reason = "cells are below 2^32")]
            let first = hilbert_index(ax as u32, ay as u32);
            // An aligned square of the curve covers the consecutive indexes
            // of one block of `side²`.
            let cells = side * side;
            let start = first & !(cells - 1);
            ranges.push((start, start + (cells - 1)));
            continue;
        }
        let half = side / 2;
        for (dx, dy) in [(0, 0), (half, 0), (0, half), (half, half)] {
            squares.push((qx + dx, qy + dy, level - 1));
        }
    }
    let mut ranges = merge(ranges);
    // Join the closest ranges until the budget holds.
    while ranges.len() > max_ranges.max(1) {
        let gap = (1..ranges.len())
            .min_by_key(|&i| ranges[i].0 - ranges[i - 1].1)
            .unwrap_or(1);
        ranges[gap - 1].1 = ranges[gap].1;
        ranges.remove(gap);
    }
    ranges
}

fn merge(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ranges.sort_unstable();
    let mut out: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match out.last_mut() {
            Some(last) if start <= last.1.saturating_add(1) => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

/// The box around `center` holding every point within `meters` of it (for
/// WGS-84, degrees widened by latitude; a box reaching a pole spans every
/// longitude).
#[must_use]
pub fn radius_box(center: Point, meters: f64) -> (Point, Point) {
    let corner = |x: f64, y: f64| Point {
        crs: center.crs,
        x,
        y,
        z: None,
    };
    if !center.crs.geographic() {
        return (
            corner(center.x - meters, center.y - meters),
            corner(center.x + meters, center.y + meters),
        );
    }
    let dlat = (meters / EARTH_RADIUS_METERS).to_degrees();
    let (south, north) = (center.y - dlat, center.y + dlat);
    if south <= -90.0 || north >= 90.0 {
        return (
            corner(-180.0, south.max(-90.0)),
            corner(180.0, north.min(90.0)),
        );
    }
    let widest = south.abs().max(north.abs()).to_radians().cos();
    let dlon = (meters / (EARTH_RADIUS_METERS * widest)).to_degrees();
    if dlon >= 180.0 {
        return (corner(-180.0, south), corner(180.0, north));
    }
    let wrap = |x: f64| {
        if x < -180.0 {
            x + 360.0
        } else if x > 180.0 {
            x - 360.0
        } else {
            x
        }
    };
    (
        corner(wrap(center.x - dlon), south),
        corner(wrap(center.x + dlon), north),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wgs(x: f64, y: f64) -> Point {
        Point::new(Crs::Wgs84, x, y, None).unwrap()
    }

    #[test]
    fn geojson_round_trips_and_refuses_other_objects() {
        for value in [
            json!({"type": "Point", "coordinates": [13.4, 52.5]}),
            json!({"type": "Point", "coordinates": [13.4, 52.5, 34.0]}),
            json!({"type": "Point", "coordinates": [1.0, 2.0],
                   "crs": {"type": "name", "properties": {"name": "cartesian"}}}),
        ] {
            let point = parse_geojson(&value).unwrap();
            assert_eq!(to_geojson(point), value);
            assert_eq!(decode(&encode(point)), Some(point));
        }
        assert_eq!(
            parse_geojson(&json!({"type": "Point", "coordinates": [1, 2], "crs": "EPSG:9157"})),
            None,
            "a 3D CRS needs three coordinates"
        );
        assert!(parse_geojson(&json!({"type": "Point", "coordinates": [200.0, 0.0]})).is_none());
        assert!(
            parse_geojson(&json!({"type": "Point", "coordinates": [1.0, 2.0], "name": "x"}))
                .is_none()
        );
        assert!(
            parse_geojson(&json!({"type": "LineString", "coordinates": [[1.0, 2.0]]})).is_none()
        );
    }

    #[test]
    fn distance_and_boxes_follow_neo4j() {
        // Berlin to Paris: about 878 km on Neo4j's sphere.
        let meters = distance(wgs(13.405, 52.52), wgs(2.3522, 48.8566)).unwrap();
        assert!((meters - 878_000.0).abs() < 2_000.0, "{meters}");
        let cartesian = |x, y| Point::new(Crs::Cartesian, x, y, None).unwrap();
        assert_eq!(
            distance(cartesian(0.0, 0.0), cartesian(3.0, 4.0)),
            Some(5.0)
        );
        assert_eq!(distance(wgs(0.0, 0.0), cartesian(0.0, 0.0)), None);
        // A box across the antimeridian.
        assert_eq!(
            within_box(wgs(179.5, 0.0), wgs(170.0, -1.0), wgs(-170.0, 1.0)),
            Some(true)
        );
        assert_eq!(
            within_box(wgs(0.0, 0.0), wgs(170.0, -1.0), wgs(-170.0, 1.0)),
            Some(false)
        );
    }

    #[test]
    fn a_cover_holds_every_point_of_its_box() {
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            #[expect(clippy::cast_precision_loss, reason = "a test's random coordinates")]
            let unit = (state >> 11) as f64 / (1u64 << 53) as f64;
            unit
        };
        for _ in 0..50 {
            let (x0, y0) = (next() * 360.0 - 180.0, next() * 180.0 - 90.0);
            let (w, h) = (next() * 40.0, next() * 20.0);
            let (low, high) = ((x0, y0), ((x0 + w).min(180.0), (y0 + h).min(90.0)));
            let ranges = cover(Crs::Wgs84, low, high, 16);
            assert!(ranges.len() <= 16);
            for _ in 0..200 {
                let point = wgs(
                    low.0 + next() * (high.0 - low.0),
                    low.1 + next() * (high.1 - low.1),
                );
                let (cx, cy) = cell_xy(point);
                let cell = hilbert_index(cx, cy);
                assert!(
                    ranges.iter().any(|(s, e)| (*s..=*e).contains(&cell)),
                    "{point:?} not covered by {ranges:?}"
                );
            }
        }
    }
}
