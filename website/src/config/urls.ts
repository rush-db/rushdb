const email = 'hi@rushdb.com' as const

const emailUrl = `mailto:${email}` as const

export const socials = {
  discord: 'https://discord.gg/bdjTEybp',
  github: 'https://github.com/rush-db/rushdb',
  email,
  emailUrl,
  x: 'https://twitter.com/CollectAPI',
  linkedIn: 'https://www.linkedin.com/company/rushdb/',
  blog: '/blog'
} as const

export const links = {
  docs: 'https://docs.rushdb.com',
  getStarted: 'https://docs.rushdb.com/quick-start/installation',
  tutorials: 'https://docs.rushdb.com',
  pricing: '/pricing',
  app: 'https://app.rushdb.com',
  appPricing: 'https://app.rushdb.com/pricing',
  contactUs: 'mailto:hi@rushdb.com'
} as const
