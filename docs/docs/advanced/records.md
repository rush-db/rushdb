---
sidebar_position: 3
---
# Records

:::tip
If you're seeking documentation for the Records API, you can find it by following the link to [Records API](/basic-concepts/records).
:::

In RushDB, you have the capability to store meaningful data within **Records**. These **Records** consist of individual data 
pieces, each containing keys and their corresponding values (**Properties**).


## How it works

Consider a **Record** as a row in a database, with each **Property** being like a column. Though the inner implementation may involve 
complex graph structures, at its core, a **Record** is just an object that holds simple keys and values. You can think of 
it as a "dictionary", "map", "hash table", or "associative array" depending on your programming language or context.

For example:
```typescript
// Record:User
const user = {
    id: "si310jfpiej20i9h",
    name: "John Galt",
    emailConfirmed: true,
    registeredAt: "2022-07-19T08:30:28.000Z",
    rating: 4.98,
    currency: "USD",
    email: "john.galt@example.com"
}
```
It's just an object that holds some keys and values.


Or this one:
```typescript
// Record:Coffee
const coffee = {
    origin: "Guatemala", 
    process: "washed", 
    cupping: 86, 
    inStock: true,
    roasted: "2023-07-20T14:50:00Z",
    notes: ["Nuts", "Caramel", "Lime"]
}
```
According to [Data Types](/advanced/data-types) this **Record** contains an array of values and other available value types.


## Complex data
In the realm of real-world data and a human-centric mindset, there may arise a need to store nested data within **Records**.
In the context of RushDB, this becomes a critical requirement in order to maintain simplicity and eliminate the layers 
of abstraction that exist between the raw data and our conceptual understanding of it.


This is precisely why we have
engineered RushDB to have the capability to contain **Records** within other **Records**. While each individual **Record**
remains distinct and accessible on its own, you retain the flexibility to structure your data in a manner that aligns
with your thought process. Read further in the [Nesting](/advanced/nesting) section.

