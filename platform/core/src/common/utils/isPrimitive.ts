export const isPrimitive = (value: unknown) => {
  return (
    typeof value === 'number' || typeof value === 'string' || typeof value === 'boolean' || value === null
  )
}
