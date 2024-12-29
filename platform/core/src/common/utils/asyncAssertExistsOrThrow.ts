export async function asyncAssertExistsOrThrow<T>(
  valueProvider: () => Promise<T | null | undefined>,
  error: Error | string
): Promise<T> {
  const value = await valueProvider()
  if (value === null || value === undefined) {
    if (typeof error === 'string') {
      throw new Error(error)
    }
    throw error
  }
  return value
}
