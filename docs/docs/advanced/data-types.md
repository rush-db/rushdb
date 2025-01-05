---
sidebar_position: 2
---
# Supported Data Types

RushDB supports a wide range of data types to accommodate diverse data needs and provide a flexible environment for your applications. Below is a comprehensive list of the supported data types along with their descriptions:

### `string`
This data type is used for any textual information and can hold text of unlimited length. 
For example: 

`"Pat spit the pips in the tin."`

`"John Galt"`

`"Silence is golden"` 

`"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."`

---
### `number`
This data type accommodates both floating-point numbers and integers. For instance, it can handle values like 
`-120.209817` (a float) or `42` (an integer).

### `datetime`
This data type adheres to the ISO 8601 format, including timezones. For example: `2012-12-21T18:29:37Z`.

### `boolean`
This data type can only have two possible values: `true` or `false`.

### `null`
This data type has only one possible value: `null`.


---
## Arrays

In essence, RushDB supports all the data types that JSON does. However, when it comes to arrays, RushDB can indeed 
hold them as **Property** values, but it's important to note that it can only store <u>consistent values</u> within those 
arrays. To learn more, check out the [Properties](/advanced/properties) section.

Here some valid examples:
- `["apple", "banana", "carrot"]` - good
- `[null, null, null, null, null]` - wierd, but works fine 🤔
- `[4, 8, 15, 16, 23, 42]` - works as well
- `["2023-09-17T02:47:54+04:00", "1990-08-18T04:35:00+05:00"]` - also good
- `[true, false, true, false, true]` - love is an answer (🌼)

