import { NextApiRequest, NextApiResponse } from 'next'
import { getFeaturedBlogPosts } from '~/sections/blog/remote-utils'

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== 'GET') {
    return res.status(405).end()
  }

  try {
    const posts = await getFeaturedBlogPosts(3)
    res.setHeader('Cache-Control', 'public, s-maxage=3600, stale-while-revalidate=86400')
    return res.status(200).json(posts)
  } catch {
    return res.status(200).json([])
  }
}
