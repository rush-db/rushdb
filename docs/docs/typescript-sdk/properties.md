---
sidebar_position: 4
---

# Properties

Inspect and manage field definitions across your project.

## Find Properties

`db.properties.find()`

Returns all property definitions matching the filter.

```typescript
const { data } = await db.properties.find({
  where: { type: "number" },
});
// [{ id, name: 'rating', type: 'number', ... }, ...]
```

## Find by ID

`db.properties.findById()`

```typescript
const prop = await db.properties.findById("property-id");
```

## Get Property Values

`db.properties.values()`

Enumerate distinct values for a property — useful for building filter UIs.

```typescript
const { data: genres } = await db.properties.values("prop-id-genre");
// ['sci-fi', 'action', 'drama', ...]

// With filter
const { data } = await db.properties.values("prop-id", {
  query: "sci", // text prefix filter
  orderBy: "asc",
  limit: 10,
});
```

## Delete Property

`db.properties.delete()`

```typescript
await db.properties.delete("property-id");
```

Deletes the property definition and removes it from all records that use it.
type: 'string',
