import classNames from "classnames"
import Image from "next/image"
import Link from "next/link"
import { LetterTypingText } from "~/components/LetterTypingText"
import { Post } from "~/sections/blog/types"

export function PostCard({
  post,
  className,
}: {
  post: Post
  className?: string
}) {
  return (
    <Link
      key={post.slug}
      as={`/blog/${post.slug}`}
      href={`/blog/${post.slug}`}
      className={classNames(
        "rounded-xl bg-fill2",
        "relative block w-full group aspect-video rounded-md h-full overflow-hidden [&>div]:h-full",
        className,
      )}
    >
      {post.data.image && (
        <div className="absolute inset-0 transition group-hover:scale-105">
          <Image src={post.data.image} className="object-cover" alt="" fill />
        </div>
      )}

      <div className="w-full h-full flex flex-col relative justify-end z-10 p-6 bg-fill/5 ">
        <p className="mb-auto">{post.data.date}</p>

        <LetterTypingText
          as="h2"
          className="text-xl leading-none font-bold max-w-sm drop-shadow-lg"
        >
          {post.data.title as string}
        </LetterTypingText>
      </div>
    </Link>
  )
}
