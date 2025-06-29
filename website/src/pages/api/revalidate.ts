import { NextApiRequest, NextApiResponse } from 'next'

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  // Check for secret token to secure the endpoint
  if (req.query.secret !== process.env.REVALIDATION_SECRET) {
    return res.status(401).json({ message: 'Invalid token' })
  }

  try {
    const { slug, type } = req.body

    if (type === 'post' && slug) {
      // Revalidate the specific post page
      await res.revalidate(`/blog/${slug}`)
    } else if (type === 'page' && slug) {
      // Revalidate the specific page
      await res.revalidate(`/${slug}`)
    } else if (type === 'pricing') {
      // Revalidate the pricing page
      await res.revalidate('/pricing')
    } else if (type === 'homepage') {
      // Revalidate the homepage
      await res.revalidate('/')
    }

    // Revalidate blog index regardless of type
    await res.revalidate('/blog')

    return res.json({
      revalidated: true,
      timestamp: new Date().toISOString(),
      paths:
        type === 'post' && slug ? [`/blog/${slug}`, '/blog']
        : type === 'page' && slug ? [`/${slug}`]
        : type === 'pricing' ? ['/pricing']
        : type === 'homepage' ? ['/']
        : ['/blog']
    })
  } catch (err) {
    console.error('Error during revalidation:', err)
    return res.status(500).json({
      message: 'Error revalidating',
      error: err instanceof Error ? err.message : 'Unknown error'
    })
  }
}
