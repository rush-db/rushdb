import { MDXRemote } from 'next-mdx-remote'
import { ComponentPropsWithoutRef, isValidElement } from 'react'
import { CodeBlock } from '~/components/CodeBlock'
import { Post } from '~/sections/blog/types'
import { Mermaid } from 'mdx-mermaid/lib/Mermaid'

const Pre = ({ children, ...props }: ComponentPropsWithoutRef<'pre'>) => {
  if (isValidElement(children) && children.type === 'code') {
    const language = children.props.className.split('-').pop()
    const code = children.props.children

    if (language === 'mermaid') {
      return (
        <div className="col-span-8 col-start-3 md:col-span-12 md:col-start-1">
          <Mermaid chart={code as string} />
        </div>
      )
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
  pre: Pre
})
export function MDXRenderer({
  data,
  showDate,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof MDXRemote>, 'components'> & {
  data: Post['data']
  showDate?: boolean
}) {
  const components = getPostComponents() as unknown as null

  return (
    <section className="container">
      <div className="no-preflight m-auto mt-32 max-w-5xl">
        <MDXRemote {...props} components={components} />
      </div>
      <div className="mt-12 text-center">
        {data.date && showDate ?
          <p className="typography-base text-content3 m-auto mb-2">{data.date}</p>
        : null}
        <p className="typography-base text-content3 m-auto mb-2">RushDB Team</p>
      </div>
    </section>
  )
}
