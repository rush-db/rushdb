export const getAbsoluteURL = (path: string = '/') => {
  // if (typeof window === "undefined") {
  //   return path
  // }
  return `https://rushdb.com${path}`
}
