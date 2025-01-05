import fs from "fs"
import matter from "gray-matter"
import { resolve, join } from "path"
import { Post } from "~/sections/blog/types"

const filePathToFileName = (path: string) =>
  path
    .split("/")
    .pop()
    ?.replace(/\.mdx?$/, "") ?? ""

const POSTS_PATH = resolve(process.cwd(), "src/posts")

const BLOG_POSTS_PATH = join(POSTS_PATH, "/blog")

const getFilePaths = (path: string) =>
  fs
    .readdirSync(path)
    // Only include md(x) files
    .filter((path) => /\.mdx?$/.test(path))

const dateFormatter = new Intl.DateTimeFormat("en-US", {
  year: "numeric",
  month: "long",
  day: "numeric",
})

export const getPost = (
  slug: string,
  {
    path = POSTS_PATH,
  }: {
    path?: string
  } = {},
): Post => {
  const filePath = join(path, `${slug}.mdx`)

  const source = fs.readFileSync(filePath)

  const { content, data } = matter(source)

  const fileName = filePathToFileName(filePath)

  return {
    content,
    data: {
      ...data,
      date: data.date ? dateFormatter.format(new Date(data.date)) : "",
    },
    slug: fileName,
  }
}

export const getPosts = ({
  path = POSTS_PATH,
}: { path?: string } = {}): Post[] => {
  return getFilePaths(path)
    .map((filePath) => getPost(filePathToFileName(filePath), { path }))
    .sort(
      (a, b) =>
        new Date(b.data.date).getTime() - new Date(a.data.date).getTime(),
    )
}

export const getBlogPosts = () => getPosts({ path: BLOG_POSTS_PATH })

export const getBlogPost = (slug: string) =>
  getPost(slug, { path: BLOG_POSTS_PATH })
