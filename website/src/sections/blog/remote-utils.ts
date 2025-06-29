import { PostModel, PageModel } from '~/models'
import { Post, Page } from './types'
import { unstable_cache } from 'next/cache'

// Function to get all blog posts with Next.js caching
export const getRemoteBlogPosts = unstable_cache(
  async (): Promise<Post['data'][]> => {
    try {
      const results = await PostModel.find({
        where: {
          draft: false
        }
      })

      return results.data.map((post) => post.data)
    } catch (error) {
      console.error('Error fetching remote blog posts:', error)
      return []
    }
  },
  ['blog-posts'], // cache key
  {
    revalidate: 3600, // revalidate every hour
    tags: ['blog-posts']
  }
)

// Function to get a single blog post by slug with Next.js caching
export const getRemoteBlogPost = unstable_cache(
  async (slug: string): Promise<Post['data'] | null> => {
    try {
      const result = await PostModel.findOne({
        where: {
          slug
        }
      })

      if (result.exists()) {
        return result.data
      } else {
        console.warn(`No blog post found with slug "${slug}"`)
        return null
      }
    } catch (error) {
      console.error(`Error fetching remote blog post with slug "${slug}":`, error)
      return null
    }
  },
  ['blog-post'], // cache key prefix
  {
    revalidate: 3600, // revalidate every hour
    tags: ['blog-post']
  }
)

// Function to get a single page by slug with Next.js caching
export const getRemotePage = unstable_cache(
  async (slug: string): Promise<Page['data'] | null> => {
    try {
      const result = await PageModel.findOne({
        where: {
          slug,
          draft: false
        }
      })
      if (result.exists()) {
        return result.data
      } else {
        console.warn(`No page found with slug "${slug}"`)
        return null
      }
    } catch (error) {
      console.error(`Error fetching remote page with slug "${slug}":`, error)
      return null
    }
  },
  ['page'], // cache key prefix
  {
    revalidate: 3600, // revalidate every hour
    tags: ['page']
  }
)

// Function to get featured blog posts with Next.js caching
export const getFeaturedBlogPosts = unstable_cache(
  async (limit?: number): Promise<Post['data'][]> => {
    try {
      const results = await PostModel.find({
        where: {
          draft: false,
          featured: true
        },
        limit
      })

      return results.data.map((post) => post.data)
    } catch (error) {
      console.error('Error fetching featured blog posts:', error)
      return []
    }
  },
  ['featured-blog-posts'], // cache key
  {
    revalidate: 3600, // revalidate every hour
    tags: ['featured-blog-posts']
  }
)
