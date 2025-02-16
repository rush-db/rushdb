import classNames from 'classnames'
import Image from 'next/image'
import Link from 'next/link'
import { LetterTypingText } from '~/components/LetterTypingText'
import { Post } from '~/sections/blog/types'

export function PostCard({ post, className }: { post: Post; className?: string }) {
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
      {post.data.image && (
        <div className="absolute inset-0 transition group-hover:scale-105">
          <Image src={post.data.image} className="object-cover" alt="" fill />
        </div>
      )}

      <div className="bg-fill/5 relative z-10 flex h-full w-full flex-col justify-end p-6">
        <p className="mb-auto">{post.data.date}</p>

        <LetterTypingText as="h4" className="text-md max-w-sm font-bold leading-none drop-shadow-lg">
          {post.data.title as string}
        </LetterTypingText>
      </div>
    </Link>
  )
}
