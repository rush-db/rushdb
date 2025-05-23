import Head from 'next/head'
import { useRouter } from 'next/router'
import { getAbsoluteURL } from '~/utils'
import { metaThemeColor } from '~/config/theme'

export const defaultTitle = 'RushDB – Instant Graph Database for AI & Modern Apps'

export const defaultDescription =
  'RushDB is a zero-config, graph-powered database built for AI, SaaS, and ML. Fast queries, seamless scaling, no setup. Try it now!'

export const defaultKeywords =
  'RushDB, Firebase alternative, Supabase alternative, Neo4j, AI database, graph database, vector database, knowledge graph, NoSQL database, scalable database, cloud database, open source database, serverless database, AI embeddings, vector search, time-series database, backend as a service, REST API, data-intensive apps, developer-friendly database'

export const Meta = ({
  title = defaultTitle,
  description = defaultDescription,
  image = getAbsoluteURL('/opengraph-image.png?v=' + Math.random())
}: {
  title?: string
  image?: string
  description?: string
}) => {
  const titleWithSuffix = title === defaultTitle ? defaultTitle : `${title} • RushDB`

  const router = useRouter()

  return (
    <Head>
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <meta charSet="utf-8" />
      <meta name="theme-color" content={metaThemeColor} />
      <link rel="icon" href="/favicon.ico" />

      {/* HTML Meta Tags */}
      <title>{titleWithSuffix}</title>
      <meta name="description" content={description} />
      <meta name="keywords" content={defaultKeywords} />

      {/*Facebook Meta Tags */}
      <meta property="og:url" content={getAbsoluteURL(router.asPath)} />
      <meta property="og:type" content="website" />
      <meta property="og:title" content={title} />
      <meta property="og:description" content={description} />
      <meta property="og:image" content={image} />

      {/* Twitter  Meta Tags */}
      <meta name="twitter:card" content="summary_large_image" />
      <meta property="twitter:domain" content="rushdb.com" />
      <meta property="twitter:url" content={getAbsoluteURL(router.asPath)} />
      <meta name="twitter:title" content={title} />
      <meta name="twitter:description" content={description} />
      <meta name="twitter:image" content={image} />

      <link rel="canonical" href={getAbsoluteURL(router.asPath)} />
    </Head>
  )
}
