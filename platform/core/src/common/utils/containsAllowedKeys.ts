export const containsAllowedKeys = (obj: any, allowedKeys: string[]) => {
  const keys = Object.keys(obj)
  for (const key of keys) {
    if (!allowedKeys.includes(key)) {
      return false
    }
  }
  return true
}
