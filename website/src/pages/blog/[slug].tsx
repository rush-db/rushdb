import { MDXRemoteSerializeResult } from "next-mdx-remote"
import { serialize } from "next-mdx-remote/serialize"

import { Layout } from "~/components/Layout"
import { GetStaticPaths, GetStaticProps } from "next"
import { LetterTypingText } from "~/components/LetterTypingText"
import { getBlogPost, getBlogPosts } from "~/sections/blog/utils"
import { Post } from "~/sections/blog/types"
import { MDXRenderer } from "~/sections/blog/MDXRenderer"
import { PostCard } from "~/sections/blog/PostCard"
import classNames from "classnames"
import Head from "next/head"
import { getAbsoluteURL } from "~/utils"

type Props = {
  serializedPost: MDXRemoteSerializeResult
  data: Post["data"]
  morePosts: Array<Post>
}

type Params = {
  slug: string
}

export default function PostPage({ serializedPost, data, morePosts }: Props) {
  return (
    <Layout
      title={data.title}
      description={data.description}
      image={getAbsoluteURL(data.image)}
    >
      <Head>
        {data?.noindex ? (
          <meta name="robots" content="noindex, nofollow" />
        ) : (
          <meta name="robots" content="index, follow" />
        )}
      </Head>

      <MDXRenderer {...serializedPost} data={data} />

      {morePosts?.length > 0 && (
        <section className="overflow-hidden grid grid-cols-12 mt-16 gap-16 md:grid-cols-1">
          <LetterTypingText
            as="h2"
            className="container typography-3xl col-start-3 col-span-9 md:col-start-1 md:col-span-1"
          >
            More Blog Posts
          </LetterTypingText>

          <div className="gap-5 col-start-3 col-span-9 md:col-start-1 md:col-span-1">
            <div className="flex overflow-auto gap-5 w-full container">
              {morePosts.map((post, idx) => (
                <PostCard
                  key={post.slug}
                  post={post}
                  className={classNames(
                    "!h-[520px] !w-auto !aspect-[9/16] shrink-0",
                  )}
                />
              ))}
            </div>
          </div>
        </section>
      )}
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

  const post = getBlogPost(params.slug)

  const serializedPost = await serialize(post.content, {
    // mdxOptions: {
    //   remarkPlugins: [],
    //   rehypePlugins: [],
    // },
    scope: post.data,
  })

  const morePosts = getBlogPosts()
    .filter((post) => post.slug !== params.slug)
    .slice(0, 3)

  return {
    props: {
      serializedPost,
      data: post.data,
      morePosts,
    },
  }
}

export const getStaticPaths: GetStaticPaths = async () => {
  const paths = getBlogPosts().map(({ slug }) => ({
    params: { slug },
  }))

  return {
    paths,
    fallback: false,
  }
}
