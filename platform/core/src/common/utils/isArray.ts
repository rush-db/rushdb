export const isArray = (item: any): item is any[] =>
  typeof item === 'object' && Array.isArray(item) && item !== null
