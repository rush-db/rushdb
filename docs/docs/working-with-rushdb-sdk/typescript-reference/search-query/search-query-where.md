---
sidebar_position: 2
---

# SearchQueryWhere

The `SearchQueryWhere` type is used to define the filtering conditions for a query. It supports logical grouping and field-specific conditions.

### Type Definition
```typescript
type SearchQueryWhereClause<T extends FlatObject | Schema = Schema> = {
    where?: SearchQueryWhere<T>;
};

type SearchQueryWhere<T extends FlatObject | Schema = Schema> =
    | SearchQueryCondition<T>
    | RequireAtLeastOne<SearchQueryLogicalGrouping<T>>;

type SearchQueryCondition<T extends FlatObject | Schema = Schema> = {
    [K in keyof T]?: T extends Schema ? CollectWhereValueByType[T[K]['type']]
        : T[K] extends number ? NumberValue
            : T[K] extends boolean ? BooleanValue
                : T[K] extends string ? DatetimeValue | StringValue
                    : T[K] extends null ? NullValue
                        : CollectWhereValue
};

type CollectWhereValueByType = {
    boolean: BooleanValue;
    datetime: DatetimeValue;
    null: NullValue;
    number: NumberValue;
    string: StringValue;
};


type CollectWhereValue = BooleanValue | DatetimeValue | NullValue | NumberValue | StringValue;
```

### Properties

#### Logical Grouping

- **Type:** `SearchQueryLogicalGrouping`
- **Optional:** Yes

Defines logical groupings (`$and`, `$or`, `$not`, `$nor`, `$xor`) for combining multiple conditions.

#### Field Conditions

- **Type:** `SearchQueryCondition`
- **Optional:** Yes

Defines the conditions for individual fields based on their types.

### Example Usage

Here is an example of how to define filtering conditions using `SearchQueryWhere`:
```typescript
const queryWhere: SearchQueryWhere<typeof AuthorSchema> = {
  $and: [
    { age: { $gt: 25 } },
    { name: { $startsWith: 'A' } }
  ]
};
```

In this example:
- The query filters records where the `age` field is greater than 25 and the `name` field starts with 'A'.
