import { PostModel, PageModel } from '~/models'
import { Post, Page } from './types'

// Function to get all blog posts
export const getRemoteBlogPosts = async (): Promise<Post['data'][]> => {
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
}

// Function to get a single blog post by slug
export const getRemoteBlogPost = async (slug: string, previewId?: string): Promise<Post['data'] | null> => {
  try {
    const result = await PostModel.findOne({
      where:
        previewId ?
          {
            slug,
            $id: previewId
          }
        : { slug, draft: false }
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
}

// Function to get a single page by slug
export const getRemotePage = async (slug: string): Promise<Page['data'] | null> => {
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
}

// Function to get featured blog posts
export const getFeaturedBlogPosts = async (limit?: number): Promise<Post['data'][]> => {
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
}
