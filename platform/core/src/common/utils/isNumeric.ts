export const isNumeric = (value: string) => {
  if (typeof value !== 'string') {
    return false
  }
  return !isNaN(Number(value)) && !isNaN(parseFloat(value))
}
