import { MDXRemoteSerializeResult } from 'next-mdx-remote'
import { serialize } from 'next-mdx-remote/serialize'

import { Layout } from '~/components/Layout'
import { GetServerSideProps } from 'next'
import { LetterTypingText } from '~/components/LetterTypingText'
import { getRemoteBlogPost, getRemoteBlogPosts } from '~/sections/blog/remote-utils'
import { Post } from '~/sections/blog/types'
import { MDXRenderer } from '~/sections/blog/MDXRenderer'
import { PostCard } from '~/sections/blog/PostCard'
import Head from 'next/head'
import { getAbsoluteURL } from '~/utils'
import { Faq } from '~/components/Faq'
import { CallToAction } from '~/components/CallToAction'
import remarkGfm from 'remark-gfm'
import { generateJsonLd } from '~/utils/jsonLd'
import { useRouter } from 'next/router'
import mermaidPlugin from 'mdx-mermaid' // this might bring commonJS vs ESM problems, I am not 100% sure

type Props = {
  serializedPost: MDXRemoteSerializeResult
  data: Post['data']
  morePosts: Array<Post['data']>
}

type Params = {
  slug: string
}

export default function PostPage({ serializedPost, data, morePosts }: Props) {
  const router = useRouter()

  const jsonLdData = generateJsonLd('blog', {
    title: data.title,
    description: data.excerpt,
    datePublished: data.publishedAt,
    url: getAbsoluteURL(router.asPath)
  })

  return (
    <Layout title={data.title} description={data.excerpt} image={getAbsoluteURL(data.image)}>
      <Head>
        <meta name="robots" content="index, follow" />
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLdData) }} />
      </Head>
      <MDXRenderer {...serializedPost} data={data} showDate />
      <div className="container">
        <div className="m-auto mb-8 mt-8 max-w-5xl border-t pb-8">
          <CallToAction />

          <Faq />
        </div>
      </div>
      {morePosts?.length > 0 && (
        <section className="container mb-32 mt-16">
          <div className="m-auto max-w-5xl border-t pt-16">
            <LetterTypingText as="h2" className="typography-xl container m-0 mb-8 p-0">
              More Blog Posts
            </LetterTypingText>

            <div className="grid grid-cols-3 gap-4">
              {morePosts.slice(0, 3).map((post, idx) => (
                <PostCard key={post.slug} post={post} />
              ))}
            </div>
          </div>
        </section>
      )}
    </Layout>
  )
}

export const getServerSideProps: GetServerSideProps<Props> = async ({ params, query }) => {
  if (!params?.slug) {
    return {
      notFound: true
    }
  }

  try {
    const previewId = query.preview as string | undefined

    // Fetch post and all posts in parallel
    const [post, allPosts] = await Promise.all([
      getRemoteBlogPost(params.slug as string, previewId),
      getRemoteBlogPosts()
    ])

    if (!post?.__id) {
      return {
        notFound: true
      }
    }

    // Serialize MDX content
    const serializedPost = await serialize(post?.content, {
      mdxOptions: {
        remarkPlugins: [remarkGfm, mermaidPlugin]
      },
      scope: post
    })

    // Filter and limit related posts
    const morePosts = allPosts.filter((p) => p.slug !== params.slug).slice(0, 3)

    return {
      props: {
        serializedPost,
        data: post,
        morePosts
      }
    }
  } catch (error) {
    console.error('Error in getServerSideProps:', error)
    return {
      notFound: true
    }
  }
}
