# Spatial queries

This page shows how to find records by where they are: points within a
distance of a center, points inside a box, and records ordered nearest
first. It is for developers storing locations or coordinates. The point type
itself is described in
[Properties and types](../data-model/properties-and-types.md#points).

## Storing points

A point is a GeoJSON point, longitude first. It is typed as a point whatever
the write options:

```bash
curl -s http://127.0.0.1:3000/protocol/v1/execute \
  -H "Authorization: Bearer $RUSHDB_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "protocolVersion": 1,
    "operation": { "kind": "mutate", "mutation": {
      "kind": "putRecord", "mode": "create", "label": "Cafe",
      "properties": {
        "name": "Kaffeehaus",
        "location": { "type": "Point", "coordinates": [13.405, 52.52] }
      }
    } }
  }'
```

`"crs": "cartesian"` stores a point in a Cartesian plane instead (a floor
plan, a game map); distances between such points are in the plane's own
units.

## Within a radius

`$within` with `center` and `meters` matches the points at most that
distance from the center. For WGS-84 points the distance is the great-circle
(haversine) distance in meters; for Cartesian points it is the Euclidean
distance in the plane's units.

```json
{
  "labels": ["Cafe"],
  "predicate": {
    "location": {
      "$within": { "center": { "type": "Point", "coordinates": [13.405, 52.52] }, "meters": 2000 }
    }
  }
}
```

## Within a box

`$within` with `box` matches the points inside the box from its south-west
corner to its north-east corner, edges included:

```json
{
  "labels": ["Cafe"],
  "predicate": {
    "location": {
      "$within": { "box": [
        { "type": "Point", "coordinates": [13.3, 52.45] },
        { "type": "Point", "coordinates": [13.5, 52.55] }
      ] }
    }
  }
}
```

A WGS-84 box whose south-west longitude is east of its north-east longitude
crosses the antimeridian: `[[170, -10], [-170, 10]]` covers the 20 degrees
around 180°. The south-west latitude must not be north of the north-east
one, and both corners must be of one CRS.

## Nearest first

`order` with `$distance` sorts by distance from a point, nearest first.
With `limit` it answers "the five nearest":

```json
{
  "labels": ["Cafe"],
  "order": { "location": { "$distance": { "type": "Point", "coordinates": [13.405, 52.52] } } },
  "limit": 5
}
```

Combine it with `$within` to rank only the records in a region. Records
without a point in the property, or with a point of another CRS, sort after
every distance.

## Rules

- A point of another CRS than the region's never matches: a Cartesian point
  is never within a WGS-84 radius.
- On a multi-value property, a record matches when any of its points is in
  the region.
- `$within` takes no other operand: anything other than a radius or a box
  of GeoJSON points is `invalid_request`.
- Points have no order of their own: `$gt`, `$lt` and an `order` without
  `$distance` do not apply to them.

## Indexes

Without an index, `$within` checks every record of the queried labels that
holds the property. An [ordered index](../indexes-and-search/indexes.md) on
the point property answers it from the index: points are kept in Hilbert
curve order, so the region is read as a few ranges of nearby points, and
each point found is checked exactly. The cost then follows the size of the
region, not of the label.

```json
{ "kind": "index", "index": { "kind": "create", "definition": {
  "name": "cafe_location", "label": "Cafe", "property": "location", "kind": "ordered"
} } }
```

## In other languages

- **Cypher over Bolt** sends and returns points as Bolt `Point2D` and
  `Point3D` structures; see [Bolt](../connectivity/bolt.md). Cypher's own
  `point()` function and `point.distance` are not available.
- **SQL** reads a 2D point as a PostgreSQL `point` column; see
  [SQL](../languages/sql.md).

## See also

- [Properties and types](../data-model/properties-and-types.md#points)
- [Filtering](filtering.md)
- [Sorting and pagination](sorting-and-pagination.md)
- [Indexes](../indexes-and-search/indexes.md)
