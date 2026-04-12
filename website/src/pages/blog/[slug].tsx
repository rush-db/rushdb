import { MDXRemoteSerializeResult } from 'next-mdx-remote'
import { serialize } from 'next-mdx-remote/serialize'

import { LPLayout } from '~/components/lp/LPLayout'
import { GetStaticProps } from 'next'
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
import mermaidPlugin from 'mdx-mermaid'

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
    <LPLayout title={data.title} description={data.excerpt} image={getAbsoluteURL(data.image)}>
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
        <section className="mx-auto mb-24 mt-16 max-w-5xl">
          <div style={{ borderTop: '1px solid #1E1E22' }} className="pt-12">
            <h2 className="text-lp-muted mb-8 font-mono text-sm uppercase tracking-widest">More Posts</h2>
            <div className="grid grid-cols-3 gap-4 md:grid-cols-1">
              {morePosts.slice(0, 3).map((post) => (
                <PostCard key={post.slug} post={post} />
              ))}
            </div>
          </div>
        </section>
      )}
    </LPLayout>
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
    const previewId = undefined

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
