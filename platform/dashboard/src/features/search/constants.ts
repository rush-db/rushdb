export enum SearchOperations {
  // Perform an exact matching of property values with the given input.
  Equals = '$equals',

  // Exclude records with property values matching the given input.
  NotEquals = '$ne',

  //  Include records if some elements are matched (in: [])
  // In = '$in',

  // Exclude records if some elements are matched (in: [])
  // NotIn = '$nin',

  // lt, lte, gt, and gte: Perform comparisons (less than, less than or equal to, greater than, greater than or equal to) with numeric property values.
  Less = '$lt',
  LessOrEqual = '$lte',
  Greater = '$gt',
  GreaterOrEqual = '$gte',

  // contains, startsWith, endsWith: Perform substring comparisons
  Contains = '$contains',
  StartsWith = '$startsWith',
  EndsWith = '$endsWith'

  // exists: Check if field exists or not
  // Exists = '$exists',

  // type: Check field data type
  // Type = '$type'
}

export const SearchSymbols: Record<SearchOperations, string> = {
  [SearchOperations.Equals]: '=',
  [SearchOperations.Contains]: '=~',
  [SearchOperations.StartsWith]: '*.',
  [SearchOperations.EndsWith]: '.*',
  [SearchOperations.NotEquals]: '!=',
  [SearchOperations.Less]: '<',
  [SearchOperations.LessOrEqual]: '<=',
  [SearchOperations.Greater]: '>',
  [SearchOperations.GreaterOrEqual]: '>='
  // [SearchOperations.Exists]: '?',
  // [SearchOperations.Type]: '::'
}

export const operationsRecord: Record<
  SearchOperations,
  {
    description: string
    label: string
    symbol: string
    value: SearchOperations
  }
> = {
  [SearchOperations.Equals]: {
    label: 'Equals',
    value: SearchOperations.Equals,
    symbol: SearchSymbols[SearchOperations.Equals],
    description: 'Perform exact matching of property values with the given input.'
  },
  [SearchOperations.Contains]: {
    label: 'Contains',
    value: SearchOperations.Contains,
    symbol: SearchSymbols[SearchOperations.Contains],
    description: 'Perform matching of property values with the given input.'
  },
  [SearchOperations.StartsWith]: {
    label: 'Starts With',
    value: SearchOperations.StartsWith,
    symbol: SearchSymbols[SearchOperations.StartsWith],
    description: 'Perform matching of property values with the given input.'
  },
  [SearchOperations.EndsWith]: {
    label: 'Ends With',
    value: SearchOperations.EndsWith,
    symbol: SearchSymbols[SearchOperations.EndsWith],
    description: 'Perform matching of property values with the given input.'
  },
  // [SearchOperations.In]: {
  //   label: 'In',
  //   value: SearchOperations.In,
  //   symbol: SearchSymbols[SearchOperations.In],
  //   description:
  //     'Exclude records with property values matching the given input.'
  // },
  [SearchOperations.NotEquals]: {
    label: 'Not Equals',
    value: SearchOperations.NotEquals,
    symbol: SearchSymbols[SearchOperations.NotEquals],
    description: 'Exclude records with property values matching the given input.'
  },
  // [SearchOperations.NotIn]: {
  //   label: 'Not In',
  //   value: SearchOperations.NotIn,
  //   symbol: SearchSymbols[SearchOperations.NotIn],
  //   description:
  //     'Exclude records with property values matching the given input.'
  // },
  [SearchOperations.Less]: {
    label: 'Less',
    value: SearchOperations.Less,
    symbol: SearchSymbols[SearchOperations.Less],
    description: 'Perform comparisons with numeric property value'
  },
  [SearchOperations.LessOrEqual]: {
    label: 'Less Or Equal',
    value: SearchOperations.LessOrEqual,
    symbol: SearchSymbols[SearchOperations.LessOrEqual],
    description: 'Perform comparisons with numeric property value'
  },
  [SearchOperations.Greater]: {
    label: 'Greater',
    value: SearchOperations.Greater,
    symbol: SearchSymbols[SearchOperations.Greater],
    description: 'Perform comparisons with numeric property value'
  },
  [SearchOperations.GreaterOrEqual]: {
    label: 'Greater Or Equal',
    value: SearchOperations.GreaterOrEqual,
    symbol: SearchSymbols[SearchOperations.GreaterOrEqual],
    description: 'Perform comparisons with numeric property value'
  }
  // [SearchOperations.Exists]: {
  //   label: 'Exists',
  //   value: SearchOperations.Exists,
  //   symbol: SearchSymbols[SearchOperations.Exists],
  //   description: 'Check if property exists or not'
  // },
  // [SearchOperations.Type]: {
  //   label: 'Type',
  //   value: SearchOperations.Type,
  //   symbol: SearchSymbols[SearchOperations.Type],
  //   description: 'Check if property has a specific data type'
  // }
}

export const operatorOptions = Object.values(operationsRecord)
