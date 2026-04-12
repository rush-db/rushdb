import classNames from 'classnames'
import Image from 'next/image'
import Link from 'next/link'
import { Post } from '~/sections/blog/types'
import { formatDate } from './utils'

export function PostCard({ post, className }: { post: Post['data']; className?: string }) {
  return (
    <Link
      as={`/blog/${post.slug}`}
      href={`/blog/${post.slug}`}
      className={classNames('group flex flex-col overflow-hidden transition-colors', className)}
      style={{ border: '1px solid var(--lp-border)', background: 'var(--lp-surface)' }}
    >
      {/* Cover image */}
      {post.image && (
        <div className="relative w-full overflow-hidden" style={{ aspectRatio: '16/7' }}>
          <Image
            src={post.image}
            className="object-cover transition-transform duration-500 group-hover:scale-105"
            alt=""
            fill
            loading="lazy"
          />
          <div className="from-[var(--lp-surface)]/80 absolute inset-0 bg-gradient-to-t to-transparent" />
        </div>
      )}

      {/* Content */}
      <div className="flex flex-1 flex-col gap-3 p-5">
        {/* Tags */}
        {post.tags && post.tags.length > 0 && (
          <div className="flex flex-wrap gap-2">
            {post.tags.slice(0, 3).map((tag) => (
              <span
                key={tag}
                className="text-lp-muted border-lp-border font-mono text-sm uppercase tracking-wider"
                style={{ border: '1px solid var(--lp-border)', padding: '1px 8px' }}
              >
                {tag}
              </span>
            ))}
          </div>
        )}

        <h3 className="text-lp-text group-hover:text-lp-accent font-mono text-sm font-bold leading-snug transition-colors">
          {post.title}
        </h3>

        {post.excerpt && (
          <p className="text-lp-muted line-clamp-2 font-sans text-sm leading-relaxed">{post.excerpt}</p>
        )}

        {/* Meta */}
        <div className="text-lp-muted mt-auto flex flex-wrap items-center gap-x-3 font-mono text-sm">
          <span>{formatDate(post.publishedAt)}</span>
          {post.readTime && (
            <>
              <span style={{ color: 'var(--lp-border)' }}>—</span>
              <span>{post.readTime} min read</span>
            </>
          )}
        </div>
      </div>
    </Link>
  )
}
