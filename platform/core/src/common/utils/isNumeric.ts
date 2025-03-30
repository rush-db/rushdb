export const isNumeric = (value: unknown) => {
  if (typeof value !== 'string') {
    return false
  }
  return !isNaN(Number(value)) && !isNaN(parseFloat(value))
}
