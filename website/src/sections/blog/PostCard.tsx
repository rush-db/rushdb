import classNames from 'classnames'
import Image from 'next/image'
import Link from 'next/link'
import { Post } from '~/sections/blog/types'
import { formatDate } from './utils'
import { Tags } from '~/components/Tags'
import { useTheme } from '~/contexts/ThemeContext'

export function PostCard({ post, className }: { post: Post['data']; className?: string }) {
  const { theme } = useTheme()

  const defaultCoverImage =
    theme === 'dark' ? '/images/blog/default-cover-dark.png' : '/images/blog/default-cover.png'

  return (
    <Link
      key={post.slug}
      as={`/blog/${post.slug}`}
      href={`/blog/${post.slug}`}
      className={classNames(
        'bg-fill2 rounded-xl',
        'group relative block aspect-video h-full w-full overflow-hidden rounded-md [&>div]:h-full',
        className
      )}
    >
      <div className="absolute inset-0 transition group-hover:scale-105">
        <Image src={post.image || defaultCoverImage} className="object-cover" alt="" fill loading="lazy" />
      </div>

      {/* Gradient overlay to improve text visibility */}
      <div className="z-5 absolute inset-0 bg-gradient-to-t from-black/80 via-black/50 to-black/10"></div>

      <div className="relative z-10 flex h-full w-full flex-col justify-end p-6">
        <h4 className="text-md mb-1 font-bold leading-tight text-white">{post.title}</h4>

        {/* Meta information row below title */}
        <div className="mb-2 flex flex-wrap items-center gap-x-3 text-white/80">
          <span className="text-sm font-medium">{formatDate(post.publishedAt)}</span>
          {post.readTime && (
            <span className="flex items-center text-sm font-medium">
              <span className="mr-1 inline-block h-1 w-1 rounded-full bg-white/60"></span>
              {post.readTime} min read
            </span>
          )}
        </div>

        {post.tags && post.tags.length > 0 && <Tags tags={post.tags} limit={3} />}
      </div>
    </Link>
  )
}
