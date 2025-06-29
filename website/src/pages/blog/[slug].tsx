import { MDXRemoteSerializeResult } from 'next-mdx-remote'
import { serialize } from 'next-mdx-remote/serialize'

import { Layout } from '~/components/Layout'
import { GetStaticProps } from 'next'
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

type Props = {
  serializedPost: MDXRemoteSerializeResult
  data: Post['data']
  morePosts: Array<Post['data']>
}

type Params = {
  slug: string
}

export const revalidate = 3600

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

export const getStaticPaths = async () => {
  // Get all published posts for static generation
  const posts = await getRemoteBlogPosts()

  const paths = posts.map((post) => ({
    params: { slug: post.slug }
  }))

  return {
    paths,
    fallback: 'blocking' // Enable ISR for new posts
  }
}

export const getStaticProps: GetStaticProps<Props, Params> = async ({ params }) => {
  if (!params?.slug) {
    return {
      notFound: true
    }
  }

  try {
    // Fetch post and all posts in parallel
    const [post, allPosts] = await Promise.all([getRemoteBlogPost(params.slug), getRemoteBlogPosts()])

    if (!post?.__id) {
      return {
        notFound: true
      }
    }

    // Serialize MDX content
    const serializedPost = await serialize(post?.content, {
      mdxOptions: {
        remarkPlugins: [remarkGfm]
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
      },
      revalidate: 3600 // Revalidate every hour
    }
  } catch (error) {
    console.error('Error in getStaticProps:', error)
    return {
      notFound: true
    }
  }
}
