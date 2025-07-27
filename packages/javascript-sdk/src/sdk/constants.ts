export const ALLOWED_CONFIG_PROPERTIES = [
  'httpClient',
  'timeout',
  'host',
  'port',
  'protocol',
  'url',
  'logger',
  'options'
]

export const PlanPrefix = {
  initial: 'in',
  extended: 'ex',
  fullFeatured: 'ff'
} as const
