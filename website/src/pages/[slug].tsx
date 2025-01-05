import { MDXRemoteSerializeResult } from "next-mdx-remote"
import { serialize } from "next-mdx-remote/serialize"

import { Layout } from "~/components/Layout"
import { GetStaticPaths, GetStaticProps } from "next"
import { Post } from "~/sections/blog/types"
import { MDXRenderer } from "~/sections/blog/MDXRenderer"
import { getPost, getPosts } from "~/sections/blog/utils"

type Props = {
  serializedPost: MDXRemoteSerializeResult
  data: Post["data"]
}

type Params = {
  slug: string
}

export default function PostPage({ serializedPost, data }: Props) {
  return (
    <Layout
      title={data.title}
      description={data.description}
      image={data.image}
    >
      <MDXRenderer {...serializedPost} data={data} />
    </Layout>
  )
}

export const getStaticProps: GetStaticProps<Props, Params> = async ({
  params,
}) => {
  if (!params?.slug) {
    return {
      notFound: true,
    }
  }

  const post = getPost(params.slug)

  const serializedPost = await serialize(post.content, {
    scope: post.data,
  })

  return {
    props: {
      serializedPost,
      data: post.data,
    },
  }
}

export const getStaticPaths: GetStaticPaths = async () => {
  const paths = getPosts().map(({ slug }) => ({
    params: { slug },
  }))

  return {
    paths,
    fallback: false,
  }
}
