export const sanitizeSettings = (settings: any) => {
  if (!settings) {
    return
  }

  const clearedSettings = {}

  for (const propName in settings) {
    const validProp =
      settings[propName] !== null &&
      settings[propName] !== undefined &&
      (typeof settings[propName] === 'string' ||
        typeof settings[propName] === 'boolean' ||
        typeof settings[propName] === 'number')

    if (validProp) {
      clearedSettings[propName] = settings[propName]
    }
  }

  return clearedSettings as unknown as string
}

export const validateEmail = (email: string) => {
  return String(email)
    .toLowerCase()
    .match(
      /^(([^<>()[\]\\.,;:\s@"]+(\.[^<>()[\]\\.,;:\s@"]+)*)|.(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/
    )
}
