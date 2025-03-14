---
sidebar_position: 3
---

# Managing Labels and Properties

:::note
In RushDB, labels and properties are crucial for defining and organizing your data models. Labels act as categories or tags for your records, while properties define the structure and data types of the records themselves. This section covers how to manage labels and properties using the RushDB SDK.
:::

## Understanding Labels and Properties

### Labels

Labels are used to categorize Records in RushDB. Each model or record type is associated with a label, which helps in organizing and querying the data.

### Properties

Properties are the individual fields within a record. Each property has a type, such as string, number, or datetime, and can have additional attributes like `required`, `unique`, and `default`.

## Managing Labels

You can manage labels using the `labels` property in the `RestAPI` class. This allows you to find labels associated with your records.

### Finding Labels

The `find` method under `labels` allows you to search for labels based on specific criteria.

#### Example

Finding labels with specific criteria:

```typescript
const labelSearchCriteria = {
  where: {
    $or: [
        {
            name: {
                $startswith: 'author'
            }
        },
        {
            title: { 
                $contains: 'Guide' 
            } 
        }
    ]
  }
};

const labels = await db.labels.find(labelSearchCriteria);
// Expected output:
// {
//   success: true,
//   data: {
//     author: 12,
//     post: 14
//   }
// }
```

## Managing Properties

Properties of records can be managed using the `properties` property in the `RestAPI` class. This includes actions like finding properties and their values, deleting properties, and updating property values.

### Finding Properties

The `find` method under `properties` allows you to retrieve properties of records based on specified criteria.

#### Example

Finding properties of a record:
```typescript
const propertySearchCriteria = {
  where: {
    model: 'author'
  }
};

const properties = await db.properties.find(propertySearchCriteria);
// Expected output:
// {
//   success: true,
//   data: [
//     {
//       id: 'property_id_1',
//       name: 'name',
//       type: 'string',
//     },
//     {
//       id: 'property_id_2',
//       name: 'email',
//       type: 'string',
//     }
//   ]
// }
```

### Finding Property Values

The `values` method retrieves the values of properties for a specific record.

#### Example

Retrieving property values for a property:
```typescript
const propertyValues = await db.properties.values('property_id');
// Expected output:
// {
//   success: true,
//   data: {
//     id: 'property_id',
//     name: 'age',
//     type: 'number',
//     values: [25, 30, 35, 40],
//     max: 40,
//     min: 25
//   }
// }
```

### Deleting Properties

The `delete` method removes properties from a record.

#### Example

Deleting a property from a record:
```typescript
await db.properties.delete('property_id');
```

## Conclusion

Managing labels and properties effectively allows for better organization and retrieval of your data in RushDB. This section covered the fundamental operations for handling labels and properties, providing you with the tools to maintain a well-structured and searchable data environment.
