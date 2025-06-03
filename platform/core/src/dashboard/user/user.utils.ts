export const sanitizeSettings = (settings: string | undefined) => {
  if (!settings) {
    return
  }

  let parsedPayload = {}

  try {
    parsedPayload = JSON.parse(settings)
  } catch (e) {
    return
  }

  const clearedSettings = {}

  for (const propName in parsedPayload) {
    const validProp =
      parsedPayload[propName] !== null &&
      parsedPayload[propName] !== undefined &&
      (typeof parsedPayload[propName] === 'string' ||
        typeof parsedPayload[propName] === 'boolean' ||
        typeof parsedPayload[propName] === 'number')

    if (validProp) {
      clearedSettings[propName] = parsedPayload[propName]
    }
  }

  return JSON.stringify(clearedSettings)
}

export const validateEmail = (email: string) => {
  return String(email)
    .toLowerCase()
    .match(
      /^(([^<>()[\]\\.,;:\s@"]+(\.[^<>()[\]\\.,;:\s@"]+)*)|.(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/
    )
}
