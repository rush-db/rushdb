export const isDevMode = (callback?: () => void) => {
  const __DEV__ = process.env.NODE_ENV === 'development'
  if (typeof callback !== 'undefined' && __DEV__) {
    callback()
  }
  return __DEV__
}
