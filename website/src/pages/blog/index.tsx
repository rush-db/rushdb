import classNames from 'classnames'
import { GetStaticProps } from 'next'

import { Layout } from '~/components/Layout'
import { LetterTypingText } from '~/components/LetterTypingText'
import { PostCard } from '~/sections/blog/PostCard'
import { Post } from '~/sections/blog/types'
import { getRemoteBlogPosts } from '~/sections/blog/remote-utils'

type Props = { posts: Array<Post['data']> }

export const revalidate = 3600

export default function Index({ posts }: Props) {
  return (
    <Layout className="container">
      <section>
        <div className="flex flex-col items-center pb-16 pt-32">
          <p className="text-content3 font-mono uppercase">Latest Updates</p>
          <LetterTypingText as="h1" className="text-4xl font-black leading-[1]" animate>
            RushDB Blog
          </LetterTypingText>
          <p className="text-content3 mt-4 max-w-2xl text-center">
            Stay up to date with the latest features, tutorials, and insights from the RushDB team
          </p>
        </div>

        <div className="col-span-12 mx-auto grid w-full grid-cols-12 gap-3">
          {posts.map((post, idx) => (
            <PostCard
              key={post.slug}
              post={post}
              className={classNames({
                'col-span-8 row-span-2 row-start-1 md:col-span-12': idx === 0,
                'col-span-4 row-span-1 sm:col-span-12 md:col-span-6': idx === 1 || idx === 2,
                'col-span-4 sm:col-span-12 md:col-span-6': idx > 2
              })}
            />
          ))}
        </div>
      </section>
    </Layout>
  )
}

export const getStaticProps: GetStaticProps<Props> = async () => {
  const posts = await getRemoteBlogPosts()

  return {
    props: {
      posts
    },
    revalidate: 3600 // Revalidate every hour (3600 seconds)
  }
}
