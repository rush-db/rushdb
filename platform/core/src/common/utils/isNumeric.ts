export const isNumeric = (value: unknown): value is number =>
  typeof value === 'number' ?
    Number.isFinite(value)
  : typeof value === 'string' && value.trim() !== '' && Number.isFinite(Number(value))
