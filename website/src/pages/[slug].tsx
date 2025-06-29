import { MDXRemoteSerializeResult } from 'next-mdx-remote'

import { Layout } from '~/components/Layout'
import { GetServerSideProps } from 'next'
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

export default function StaticPage({ serializedPage, data }: Props) {
  return (
    <Layout title={data.title}>
      <MDXPageRenderer {...serializedPage} data={data} />
    </Layout>
  )
}

export const getServerSideProps: GetServerSideProps<Props> = async ({ params }) => {
  if (!params?.slug) {
    return {
      notFound: true
    }
  }

  const page = await getRemotePage(params.slug as string)

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
    }
  }
}
