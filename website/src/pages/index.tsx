import { GetStaticProps } from 'next'
import { Layout } from '~/components/Layout'
import { Hero } from '~/sections/Hero'
import { SocialProof } from '~/sections/SocialProof'
import { Mission } from '~/sections/Mission'
import { HowItWorks } from '~/sections/HowItWorks'
import { Faq } from '~/components/Faq'
import { BlogSection } from '~/sections/BlogSection'
import { Post } from '~/sections/blog/types'
import { getFeaturedBlogPosts } from '~/sections/blog/remote-utils'

type Props = {
  featuredPosts: Array<Post['data']>
}

export default function Home({ featuredPosts }: Props) {
  return (
    <Layout>
      <Hero />
      <SocialProof />
      <HowItWorks />
      <BlogSection posts={featuredPosts} />

      <section className="container mx-auto max-w-3xl">
        <Faq />
      </section>
      <Mission />
    </Layout>
  )
}

export const getStaticProps: GetStaticProps<Props> = async () => {
  const featuredPosts = await getFeaturedBlogPosts(3)

  return {
    props: {
      featuredPosts
    },
    revalidate: 3600 // Revalidate every hour
  }
}
