const email = 'hi@rushdb.com' as const

const emailUrl = `mailto:${email}` as const

export const socials = {
  discord: 'https://discord.gg/bdjTEybp',
  github: 'https://github.com/rush-db/rushdb',
  email,
  emailUrl,
  x: 'https://x.com/RushDatabase',
  linkedIn: 'https://www.linkedin.com/company/rushdb/',
  blog: '/blog'
} as const

export const links = {
  docs: 'https://docs.rushdb.com',
  getStarted: 'https://docs.rushdb.com/get-started/quick-tutorial',
  tutorials: 'https://docs.rushdb.com',
  examples: 'https://github.com/rush-db/examples',
  pricing: '/pricing',
  app: 'https://app.rushdb.com',
  appPricing: 'https://app.rushdb.com/pricing',
  contactUs: 'mailto:hi@rushdb.com'
} as const
