export const isObject = <T extends object = object>(input: unknown): input is T =>
  input !== null && Object.prototype.toString.call(input) === '[object Object]'
