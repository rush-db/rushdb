import classNames from "classnames"
import { GetStaticProps } from "next"

import { Layout } from "~/components/Layout"
import { LetterTypingText } from "~/components/LetterTypingText"
import { PostCard } from "~/sections/blog/PostCard"
import { Post } from "~/sections/blog/types"
import { getBlogPosts } from "~/sections/blog/utils"

type Props = { posts: Array<Post> }

export default function Index({ posts }: Props) {
  return (
    <Layout className="container ">
      <section>
        <div className="flex flex-col items-center pt-32 pb-16">
          <p className="text-content2 uppercase font-mono">blog</p>
          <LetterTypingText
            as="h1"
            className="text-4xl leading-[1] font-bold"
            animate
          >
            Overview
          </LetterTypingText>
        </div>

        <div className="w-full col-span-12 grid grid-cols-12 gap-3 mx-auto">
          {posts.map((post, idx) => (
            <PostCard
              key={post.slug}
              post={post}
              className={classNames({
                "row-start-1 row-span-2 col-span-8 md:col-span-12": idx === 0,
                "row-span-1 col-span-4 md:col-span-6 sm:col-span-12":
                  idx === 1 || idx === 2,
                "col-span-4 md:col-span-6 sm:col-span-12": idx > 2,
              })}
            />
          ))}
        </div>
      </section>
    </Layout>
  )
}

export const getStaticProps: GetStaticProps<Props> = () => {
  const posts = getBlogPosts()

  return { props: { posts } }
}
