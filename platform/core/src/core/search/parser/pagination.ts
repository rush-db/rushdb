// @TODO: Configure default and max with env
export const pagination = (skipRaw = 0, limitRaw = 100) => {
  const limit = limitRaw > 0 && limitRaw <= 1000 ? limitRaw : 100
  const skip = skipRaw > 0 ? skipRaw : 0

  return { limit, skip }
}
