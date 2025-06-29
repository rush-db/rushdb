import RushDB, { Model } from '@rushdb/javascript-sdk'

export const db = new RushDB(process.env.RUSHDB_API_KEY)

export const PostModel = new Model('Post', {
  title: { type: 'string', required: true },
  content: { type: 'string' },
  excerpt: { type: 'string' },
  slug: { type: 'string', required: true, uniq: true },
  draft: { type: 'boolean', default: true },
  featured: { type: 'boolean', default: false },
  publishedAt: { type: 'datetime' },
  createdAt: { type: 'datetime', default: () => new Date().toISOString() },
  updatedAt: { type: 'datetime', default: () => new Date().toISOString() },
  author: { type: 'string' },
  tags: { type: 'string', multiple: true },
  category: { type: 'string' },
  readTime: { type: 'number' },
  image: { type: 'string' }
})

export const PageModel = new Model('Page', {
  title: { type: 'string', required: true },
  content: { type: 'string' },
  slug: { type: 'string', required: true, uniq: true },
  draft: { type: 'boolean', default: true },
  template: { type: 'string', default: 'default' },
  metaTitle: { type: 'string' },
  metaDescription: { type: 'string' },
  createdAt: { type: 'datetime', default: () => new Date().toISOString() },
  updatedAt: { type: 'datetime', default: () => new Date().toISOString() }
})
