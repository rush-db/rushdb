// import fs from 'fs'
// import matter from 'gray-matter'
// import path, { resolve, join } from 'path'
import { Post } from '~/sections/blog/types'

// const filePathToFileName = (filePath: string) => path.basename(filePath, path.extname(filePath))

// const POSTS_PATH = resolve(process.cwd(), 'src/posts')

// const BLOG_POSTS_PATH = join(POSTS_PATH, '/blog')

// const getFilePaths = (path: string) =>
//   fs
//     .readdirSync(path)
//     // Only include md(x) files
//     .filter((path) => /\.mdx?$/.test(path))

// const dateFormatter = new Intl.DateTimeFormat('en-US', {
//   year: 'numeric',
//   month: 'long',
//   day: 'numeric'
// })

// export const getPost = (
//   slug: string,
//   {
//     path = POSTS_PATH
//   }: {
//     path?: string
//   } = {}
// ): Post => {
//   const filePath = join(path, `${slug}.mdx`)

//   const source = fs.readFileSync(filePath)

//   const { content, data } = matter(source)

//   const fileName = filePathToFileName(filePath)

//   return {
//     content,
//     data: {
//       ...data,
//       date: data.date ? dateFormatter.format(new Date(data.date)) : ''
//     },
//     slug: fileName
//   }
// }

// export const getPosts = ({ path = POSTS_PATH }: { path?: string } = {}): Post[] => {
//   return getFilePaths(path)
//     .map((filePath) => getPost(filePathToFileName(filePath), { path }))
//     .sort((a, b) => new Date(b.data.date).getTime() - new Date(a.data.date).getTime())
// }

// export const getBlogPosts = () => getPosts({ path: BLOG_POSTS_PATH })

// export const getBlogPost = (slug: string) => getPost(slug, { path: BLOG_POSTS_PATH })

export const formatDate = (dateString: string): string => {
  const date = new Date(dateString)

  // Get day with ordinal suffix
  const day = date.getDate()
  const ordinalSuffix = getOrdinalSuffix(day)

  // Format the date
  return `${day}${ordinalSuffix} ${date.toLocaleString('en-US', { month: 'long' })} ${date.getFullYear()}`
}

// Helper function to get ordinal suffix for day (1st, 2nd, 3rd, etc.)
const getOrdinalSuffix = (day: number): string => {
  if (day > 3 && day < 21) return 'th'
  switch (day % 10) {
    case 1:
      return 'st'
    case 2:
      return 'nd'
    case 3:
      return 'rd'
    default:
      return 'th'
  }
}
