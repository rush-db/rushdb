import { MDXRemoteSerializeResult } from 'next-mdx-remote'
import { serialize } from 'next-mdx-remote/serialize'

import { Layout } from '~/components/Layout'
import { GetStaticPaths, GetStaticProps } from 'next'
import { LetterTypingText } from '~/components/LetterTypingText'
import { getBlogPost, getBlogPosts } from '~/sections/blog/utils'
import { Post } from '~/sections/blog/types'
import { MDXRenderer } from '~/sections/blog/MDXRenderer'
import { PostCard } from '~/sections/blog/PostCard'
import classNames from 'classnames'
import Head from 'next/head'
import { getAbsoluteURL } from '~/utils'
// import mdxMermaid from 'mdx-mermaid'
import { Faq } from '~/components/Faq'
import { MainCta } from '~/components/Button'
// import rehypeMermaid from 'rehype-mermaid'

type Props = {
  serializedPost: MDXRemoteSerializeResult
  data: Post['data']
  morePosts: Array<Post>
}

type Params = {
  slug: string
}

const CallToAction = ({
  text,
  buttonText,
  description
}: {
  text: string
  description?: string
  buttonText?: string
}) => (
  <div className="bg-secondary xs:rounded-none container col-span-8 col-start-3 my-16 flex flex-row items-center justify-between gap-5 rounded-lg p-7 sm:flex-col sm:items-stretch sm:p-5 md:col-span-12 md:col-start-1">
    <div className="flex max-w-xl flex-col gap-2">
      <p className="typography-xl">{text}</p>

      {description && <p className="typography-base">{description}</p>}
    </div>

    <MainCta variant="accent" className={'shrink-0'}>
      {buttonText}
    </MainCta>
  </div>
)

export default function PostPage({ serializedPost, data, morePosts }: Props) {
  return (
    <Layout title={data.title} description={data.description} image={getAbsoluteURL(data.image)}>
      <Head>
        {data?.noindex ?
          <meta name="robots" content="noindex, nofollow" />
        : <meta name="robots" content="index, follow" />}
      </Head>
      <MDXRenderer {...serializedPost} data={data} />

      <div className="mb-8 mt-8 border-t pb-8 pt-8">
        <div className="container">
          <h4 className="py-8 text-2xl font-bold leading-tight">FAQ</h4>
          <Faq
            items={[
              {
                question: 'How does RushDB differ from traditional SQL and NoSQL databases?',
                answer:
                  'RushDB bridges the gap between SQL and NoSQL by leveraging graph-based relationships and flattening JSON or CSV data into interconnected records.'
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
          <CallToAction
            text="Ready to Revolutionize Your Data Workflow?"
            description="RushDB makes data management effortless. Push your JSON or CSV data, and let RushDB handle normalization, relationships, and querying with ease. Start now and transform your data from disconnected pieces to a fully interconnected graph. Your future self—and your projects—will thank you."
          />
        </div>
      </div>
      {morePosts?.length > 0 && (
        <section className="mt-16 grid grid-cols-12 gap-16 overflow-hidden md:grid-cols-1">
          <LetterTypingText
            as="h2"
            className="typography-3xl container col-span-9 col-start-3 md:col-span-1 md:col-start-1"
          >
            More Blog Posts
          </LetterTypingText>

          <div className="col-span-9 col-start-3 gap-5 md:col-span-1 md:col-start-1">
            <div className="container flex w-full gap-5 overflow-auto">
              {morePosts.map((post, idx) => (
                <PostCard
                  key={post.slug}
                  post={post}
                  className={classNames('!aspect-[9/16] !h-[520px] !w-auto shrink-0')}
                />
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
      // remarkPlugins: [[mdxMermaid, { output: 'svg' }]],
      // rehypePlugins: [rehypeMermaid]
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
