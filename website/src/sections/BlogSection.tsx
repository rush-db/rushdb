import Link from 'next/link'
import { PostCard } from '~/sections/blog/PostCard'
import { Post } from '~/sections/blog/types'
import { Button } from '~/components/Button'
import { ArrowRight } from 'lucide-react'
import classNames from 'classnames'
import { useEffect, useState } from 'react'

const SKELETON_CLASSES = [
  'col-span-8 row-span-2 row-start-1 md:col-span-12',
  'col-span-4 row-span-1 sm:col-span-12 md:col-span-6',
  'col-span-4 row-span-1 sm:col-span-12 md:col-span-6'
]

export const BlogSection = () => {
  const [posts, setPosts] = useState<Array<Post['data']>>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetch('/api/featured-posts')
      .then((res) => res.json())
      .then((data) => setPosts(data))
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [])

  if (!loading && posts.length === 0) {
    return null
  }

  return (
    <section className="container mx-auto max-w-6xl py-20">
      <div className="mb-12 flex flex-col items-center justify-between gap-6">
        <div className="text-center md:text-left">
          <h2 className="typography-2xl md:typography-xl font-bold">Latest from our Blog *</h2>
          <p className="text-content3 text-md mt-2 md:text-base">
            Stay updated with the latest insights, tutorials, and product updates
          </p>
        </div>
        <Link href="/blog" className="shrink-0">
          <Button variant="outline" className="group">
            Read more at our blog
            <ArrowRight className="ml-2 h-4 w-4 transition-transform group-hover:translate-x-1" />
          </Button>
        </Link>
      </div>

      <div className="col-span-12 mx-auto grid w-full grid-cols-12 gap-3">
        {loading ?
          SKELETON_CLASSES.map((cls, idx) => (
            <div key={idx} className={classNames('bg-fill2 aspect-video animate-pulse rounded-xl', cls)} />
          ))
        : posts.map((post, idx) => (
            <PostCard
              key={post.slug}
              post={post}
              className={classNames({
                'col-span-8 row-span-2 row-start-1 md:col-span-12': idx === 0,
                'col-span-4 row-span-1 sm:col-span-12 md:col-span-6': idx === 1 || idx === 2,
                'col-span-4 sm:col-span-12 md:col-span-6': idx > 2
              })}
            />
          ))
        }
      </div>
      <p className="text-content3 mt-4 text-sm">* Yes, RushDB's blog is built with RushDB 🤓</p>
    </section>
  )
}
