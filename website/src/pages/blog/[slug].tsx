import { MDXRemoteSerializeResult } from 'next-mdx-remote'
import { serialize } from 'next-mdx-remote/serialize'

import { Layout } from '~/components/Layout'
import { GetStaticPaths, GetStaticProps } from 'next'
import { LetterTypingText } from '~/components/LetterTypingText'
import { getBlogPost, getBlogPosts } from '~/sections/blog/utils'
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
  morePosts: Array<Post>
}

type Params = {
  slug: string
}

export default function PostPage({ serializedPost, data, morePosts }: Props) {
  const router = useRouter()

  const jsonLdData = generateJsonLd('blog', {
    title: data.title,
    description: data.description,
    datePublished: data.date,
    url: getAbsoluteURL(router.asPath)
  })

  return (
    <Layout title={data.title} description={data.description} image={getAbsoluteURL(data.image)}>
      <Head>
        {data?.noindex ?
          <meta name="robots" content="noindex, nofollow" />
        : <meta name="robots" content="index, follow" />}
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLdData) }} />
      </Head>
      <MDXRenderer {...serializedPost} data={data} showDate />
      <div className="container">
        <div className="m-auto mb-8 mt-8 max-w-5xl border-t pb-8">
          <CallToAction />

          <h4 className="py-8 text-xl font-bold leading-tight">FAQ</h4>
          <Faq
            items={[
              {
                question: 'How does RushDB differ from traditional SQL and NoSQL databases?',
                answer:
                  'RushDB bridges the gap between SQL and NoSQL by leveraging graph-based relationships and flattening JSON or CSV data into interconnected Records.'
              },
              {
                question: 'Do I need to structure my data before using RushDB?',
                answer:
                  'Not at all. RushDB automatically normalizes and connects your data, making setup effortless.'
              },
              {
                question: 'What makes RushDB ideal for data scientists and ML engineers?',
                answer:
                  'Its graph-based structure and powerful querying capabilities make it easy to explore and analyze complex relationships in your data.'
              },
              {
                question: 'Can RushDB handle nested JSON data effectively?',
                answer:
                  'Yes, RushDB splits nested JSON into distinct records and connects them as a graph for seamless querying.'
              },
              {
                question: 'How fast can I start using RushDB?',
                answer:
                  'You can set up and start working with RushDB in less than 30 seconds with an API token.'
              }
            ]}
          />
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

export const getStaticProps: GetStaticProps<Props, Params> = async ({ params }) => {
  if (!params?.slug) {
    return {
      notFound: true
    }
  }

  const post = getBlogPost(params.slug)

  const serializedPost = await serialize(post.content, {
    mdxOptions: {
      remarkPlugins: [remarkGfm]
    },
    scope: post.data
  })

  const morePosts = getBlogPosts()
    .filter((post) => post.slug !== params.slug)
    .slice(0, 3)

  return {
    props: {
      serializedPost,
      data: post.data,
      morePosts
    }
  }
}

export const getStaticPaths: GetStaticPaths = async () => {
  const paths = getBlogPosts().map(({ slug }) => ({
    params: { slug }
  }))

  return {
    paths,
    fallback: false
  }
}
