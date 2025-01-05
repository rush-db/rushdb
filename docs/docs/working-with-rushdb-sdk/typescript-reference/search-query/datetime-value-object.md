---
sidebar_position: 5
---

# DatetimeValue

The `DatetimeValue` type is used to define conditions for datetime fields in a query.

### Type Definition
```typescript
type DatetimeValue =
  | DatetimeObject
  | RequireAtLeastOne<
      Record<'$gt' | '$gte' | '$lt' | '$lte' | '$not', DatetimeObject | string> &
        Record<'$in' | '$notIn', Array<DatetimeObject | string>>
    >
  | string;
```

### Properties

#### $gt, $gte, $lt, $lte, $not

- **Type:** `string | DatetimeObject`
- **Optional:** Yes

Defines greater than, greater than or equal to, less than, less than or equal to, and not conditions for datetime fields.

#### $in, $notIn

- **Type:** `Array<string | DatetimeObject>`
- **Optional:** Yes

Defines inclusion and exclusion conditions for datetime fields.

### Example Usage

Here is an example of how to define conditions for a datetime field:
```typescript
const datetimeCondition: DatetimeValue = {
  $gt: '2022-01-01T00:00:00Z'
};
```

In this example:
- The query filters records where the `created` field is greater than a specific date.

## DatetimeObject
The `DatetimeObject` type is used to define a detailed datetime object for specifying precise datetime values.

### Type Definition
```typescript
type DatetimeObject = {
    $day?: number;
    $hour?: number;
    $microsecond?: number;
    $millisecond?: number;
    $minute?: number;
    $month?: number;
    $nanosecond?: number;
    $second?: number;
    $year: number;
};
```


### Properties

#### $day

- **Type:** `number`
- **Optional:** Yes

Specifies the day of the month.

#### $hour

- **Type:** `number`
- **Optional:** Yes

Specifies the hour of the day.

#### $microsecond

- **Type:** `number`
- **Optional:** Yes

Specifies the microsecond.

#### $millisecond

- **Type:** `number`
- **Optional:** Yes

Specifies the millisecond.

#### $minute

- **Type:** `number`
- **Optional:** Yes

Specifies the minute.

#### $month

- **Type:** `number`
- **Optional:** Yes

Specifies the month.

#### $nanosecond

- **Type:** `number`
- **Optional:** Yes

Specifies the nanosecond.

#### $second

- **Type:** `number`
- **Optional:** Yes

Specifies the second.

#### $year

- **Type:** `number`
- **Required:** Yes

Specifies the year.

### Example Usage

Here is an example of how to define a detailed datetime object:
```typescript
const datetimeObject: DatetimeObject = {
  $day: 15,
  $month: 8,
  $year: 2023,
  $hour: 14,
  $minute: 30,
  $second: 45
};
```
In this example:
- The datetime object specifies a precise datetime value of August 15, 2023, at 14:30:45.