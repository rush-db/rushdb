import { MDXRemoteSerializeResult } from 'next-mdx-remote'

import { Layout } from '~/components/Layout'
import { GetStaticProps, GetStaticPaths } from 'next'
import { Page } from '~/sections/blog/types'
import { MDXPageRenderer } from '~/sections/blog/MDXRenderer'
import { getRemotePage } from '~/sections/blog/remote-utils'
import remarkGfm from 'remark-gfm'
import { serialize } from 'next-mdx-remote/serialize'

type Props = {
  serializedPage: MDXRemoteSerializeResult
  data: Page['data']
}

type Params = {
  slug: string
}

export const revalidate = 3600

export default function StaticPage({ serializedPage, data }: Props) {
  return (
    <Layout title={data.title}>
      <MDXPageRenderer {...serializedPage} data={data} />
    </Layout>
  )
}

export const getStaticPaths: GetStaticPaths = async () => {
  // For static pages, we'll use fallback: 'blocking' to generate pages on-demand
  // This allows for ISR with new pages that weren't pre-built
  return {
    paths: [], // We'll generate pages on-demand
    fallback: 'blocking'
  }
}

export const getStaticProps: GetStaticProps<Props, Params> = async ({ params }) => {
  if (!params?.slug) {
    return {
      notFound: true
    }
  }

  const page = await getRemotePage(params.slug)

  if (!page?.__id) {
    return {
      notFound: true
    }
  }

  const serializedPage = await serialize(page?.content, {
    mdxOptions: {
      remarkPlugins: [remarkGfm]
    },
    scope: page
  })

  return {
    props: {
      serializedPage,
      data: page
    },
    revalidate: 3600 // Revalidate every hour
  }
}
