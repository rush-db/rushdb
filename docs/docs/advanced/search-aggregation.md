---
sidebar_position: 8
---
# Search with Data Aggregation

In addition to the classic search by specified criteria, RushDB provides data aggregation capabilities. You can return complex nested structures when searching for data, and add new fields to the search result on the fly.

## Data Aggregation Tasks and Capabilities

The main goals we pursued in RushDB when designing aggregations were to eliminate additional queries from your code for nested structures, as well as additional "on-the-fly" data modifications after they are retrieved.  
Typically, you don’t want to keep in your data model those [Properties](/basic-concepts/properties) that are filled and recalculated depending on related [Records](/basic-concepts/records) — you’d prefer to compute such data at query time.

Currently, the design of aggregations includes the following capabilities:
- Including nested structures in the response based on search criteria, with support for sorting, and pagination for nested structures (`collect` function).
- Performing mathematical operations on numeric properties and record counts (or their properties): `min`, `max`, `avg`, `sum`, and `count`.

In the future, we will expand aggregation capabilities — first and foremost, we will add support for dynamically added fields in search criteria.

Below you can see a detailed example of using different aggregations within a single code block:

```typescript
{
    where: {
        name: {
            $contains: 'John'
        },
        comments: {
            $alias: '$comment',
            likes: {
                $alias: '$like',
                likesCount: {
                    $gte: 2
                }
            }
        }
    },
    orderBy: {},
    skip: 0,
    limit: 100,
    labels: ['author'],
    aggregate: {
        authorName: '$record.name',
        commentsCount: { fn: 'count', uniq: true, alias: '$comment' },
        totalLikes: { fn: 'sum', field: 'likesCount', alias: '$like' },
        avgAuthorLikes: {
            fn: 'avg',
            field: 'likesCount',
            alias: '$like',
            precision: 0
        },
        minLikesPerComment: { fn: 'min', field: 'likesCount', alias: '$like' },
        maxLikesPerComment: { fn: 'max', field: 'likesCount', alias: '$like' },
        aggregate: {
            comments: {
                fn: 'collect',
                uniq: true,
                alias: '$comment',
                orderBy: { likesCount: 'asc' },
                limit: 5,
                skip: 0,
                aggregate: {
                    likes: {
                        fn: 'collect',
                        uniq: true,
                        alias: '$like'
                    }
                }
            }
        }
    }
}
```

In this case, you will get all authors who have more than two likes on their comments.
You will also get data slices: the minimum number of likes on a comment and the maximum number of likes on a comment, the average number of likes, their total number, and the total number of comments.

Additionally, you can set the number of returned comments in the response and, in this case, sort them by the most liked comments.

## Data Aggregation Parameters

### `fn`
Specifies the type of function to use — `collect`, `sum`, `count`, `max`, `min` or `avg`.
### `uniq`
Specifies whether to consider duplicates when retrieving data.
### `limit`, `skip`
Pagination parameter settings.
### `orderBy`
Sorting settings based on the properties of the [Records](/basic-concepts/records). Several properties are available for sorting.
### `field`
Indicates which [Properties](/basic-concepts/properties) of the [Records](/basic-concepts/records) should be used for mathematical operations.
It is not required for the `collect` and `count` functions since they can work with the [Records](/basic-concepts/records) themselves.
### `precision`
Available for the `avg` function.
### `alias`
A very important property that must be specified inside each aggregation function. It allows you to specify an `alias` within the `where` condition. It is also necessary for our API to identify references to specific [Records](/basic-concepts/records).
### `$record`
A key property that must be specified if you want to return any properties of the original (top-level) record of the search query.
