import { GetStaticProps } from 'next'

import { LPLayout } from '~/components/lp/LPLayout'
import { PostCard } from '~/sections/blog/PostCard'
import { Post } from '~/sections/blog/types'
import { getRemoteBlogPosts } from '~/sections/blog/remote-utils'

type Props = { posts: Array<Post['data']> }

export default function Index({ posts }: Props) {
  return (
    <LPLayout
      title="Blog"
      description="Stay up to date with the latest features, tutorials, and insights from the RushDB team"
    >
      <div className="mx-auto max-w-6xl px-6 py-20">
        {/* Header */}
        <div className="mb-14">
          <p className="text-lp-muted mb-3 font-mono text-sm uppercase tracking-widest">Latest Updates</p>
          <h1 className="text-lp-text font-mono text-4xl font-bold leading-tight sm:text-xl">RushDB Blog</h1>
        </div>

        {/* Featured + sidebar grid */}
        {posts.length > 0 && (
          <div className="mb-6 grid grid-cols-3 gap-4 md:grid-cols-1">
            <PostCard post={posts[0]} className="col-span-2 md:col-span-1" />
            <div className="flex flex-col gap-4">
              {posts.slice(1, 3).map((post) => (
                <PostCard key={post.slug} post={post} />
              ))}
            </div>
          </div>
        )}

        {/* Remaining posts */}
        {posts.length > 3 && (
          <div className="grid grid-cols-3 gap-4 sm:grid-cols-1 md:grid-cols-1">
            {posts.slice(3).map((post) => (
              <PostCard key={post.slug} post={post} />
            ))}
          </div>
        )}
      </div>
    </LPLayout>
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
