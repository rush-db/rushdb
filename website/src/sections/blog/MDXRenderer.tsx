import { MDXRemote } from 'next-mdx-remote'
import { ComponentPropsWithoutRef, isValidElement } from 'react'
import { CodeBlock } from '~/components/CodeBlock'
import { Page, Post } from '~/sections/blog/types'
import { CodeBlockWithLanguageSelector } from '~/components/CodeBlockWithLanguageSelector'
import { formatDate } from './utils'
import { Tags } from '~/components/Tags'
import { Mermaid } from '~/components/Mermaid'

const Pre = ({ children, ...props }: ComponentPropsWithoutRef<'pre'>) => {
  if (isValidElement(children) && children.type === 'code') {
    // @ts-ignore
    const language = children.props.className?.split('-').pop()
    // @ts-ignore
    const code = children.props.children

    if (language === 'mermaid') {
      return (
        <div className="col-span-8 col-start-3 md:col-span-12 md:col-start-1">
          <Mermaid chart={code as string} />
        </div>
      )
    }

    if (language === 'sdk') {
      const [python, typescript] = code.split('___SPLIT___')
      return <CodeBlockWithLanguageSelector data={{ typescript: typescript.replace(/\n/, ''), python }} />
    }

    return (
      <CodeBlock
        language={language}
        code={code}
        className="col-span-8 col-start-3 md:col-span-12 md:col-start-1"
      />
    )
  }

  return null
}

const getPostComponents = () => ({
  pre: Pre,
  table: ({ children, ...props }: ComponentPropsWithoutRef<'table'>) => (
    <div className="col-span-8 col-start-3 overflow-x-auto md:col-span-12 md:col-start-1">
      <table {...props}>{children}</table>
    </div>
  ),
  mermaid: Mermaid
})

export function MDXRenderer({
  data: post,
  showDate,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof MDXRemote>, 'components'> & {
  data: Post['data']
  showDate?: boolean
}) {
  const components = getPostComponents()

  return (
    <section className="container">
      <div className="no-preflight m-auto mt-32 max-w-5xl">
        {/* Post meta information - read time and tags */}
        <div className="mb-8 flex flex-col items-center gap-3">
          <div className="text-content3 flex items-center gap-3 text-sm">
            {post.publishedAt && showDate && <span>{formatDate(post.publishedAt)}</span>}
            {post.readTime && (
              <>
                {post.publishedAt && showDate && (
                  <span className="bg-content3 inline-block h-1 w-1 rounded-full"></span>
                )}
                <span>{post.readTime} min read</span>
              </>
            )}
            <span className="bg-content3 inline-block h-1 w-1 rounded-full"></span>
            <span>{post.author || 'RushDB Team'}</span>
          </div>
          {post.tags && post.tags.length > 0 && (
            <Tags
              tags={post.tags}
              className="justify-center"
              tagClassName="bg-background2/50 text-content1"
            />
          )}
        </div>

        <MDXRemote {...props} components={components} />
      </div>
    </section>
  )
}

export function MDXPageRenderer({
  data: page,
  showDate,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof MDXRemote>, 'components'> & {
  data: Page['data']
  showDate?: boolean
}) {
  const components = getPostComponents()

  return (
    <section className="container">
      <div className="no-preflight m-auto mt-32 max-w-5xl">
        <MDXRemote {...props} components={components} />
      </div>
    </section>
  )
}
