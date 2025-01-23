import classNames from 'classnames'
import { MDXRemote } from 'next-mdx-remote'
import Image from 'next/image'
import { ComponentPropsWithoutRef, isValidElement } from 'react'
import { CodeBlock } from '~/components/CodeBlock'
import { LetterTypingText } from '~/components/LetterTypingText'
import { Link } from '~/components/Link'
import { Post } from '~/sections/blog/types'
import { Mermaid } from 'mdx-mermaid/lib/Mermaid'
import { Chip } from '~/components/Chip'

const PostHeader = ({
  data,
  title,
  description,
  showDate = true
}: {
  data: Post['data']
  title: string
  description?: string
  showDate: boolean
}) => {
  return (
    <section className="container relative col-span-12 col-start-1 grid place-items-center pb-16 pt-32">
      <div className="pointer-events-none absolute left-0 top-0 -z-10 h-full w-full" aria-hidden>
        <div className="to-fill pointer-events-none absolute inset-0 h-full w-full bg-gradient-to-b from-transparent to-90%" />
      </div>

      {data.date && showDate ?
        <p className="typography-base text-content2 mb-2">{data.date}</p>
      : null}

      <LetterTypingText as="h1" className="typography-4xl !mb-0 text-center md:text-2xl" animate>
        {title}
      </LetterTypingText>

      {description && <p className="typography-lg max-w-5xl py-6 text-center">{description}</p>}

      {data?.tags && (
        <div className="m-auto flex w-full flex-wrap items-center gap-2 overflow-auto">
          {data.tags.map((tag) => (
            <Chip variant="blue" key={tag}>
              {tag}
            </Chip>
          ))}
        </div>
      )}
    </section>
  )
}

const P = ({ children, ...props }: ComponentPropsWithoutRef<'h2'>) => {
  // Check if the only child is an img element
  if (isValidElement(children) && children.props?.type === 'img') {
    return children
  }

  return (
    <p
      className="text-content2 container col-span-8 col-start-3 text-base md:col-span-12 md:col-start-1"
      {...props}
    >
      {children}
    </p>
  )
}

const H2 = ({ children, ...props }: ComponentPropsWithoutRef<'h2'>) => {
  if (typeof children !== 'string') {
    return null
  }

  const id = encodeURI(children)

  return (
    <div className="typography-2xl group container col-span-8 col-start-3 flex pb-4 pt-10 md:col-span-12 md:col-start-1">
      <LetterTypingText as="h2" id={id} animateInView {...props}>
        {children}
      </LetterTypingText>
      <a href={`#${id}`} className="text-content3 ml-1 opacity-0 group-hover:opacity-100">
        #
      </a>
    </div>
  )
}

const H3 = ({ children, ...props }: ComponentPropsWithoutRef<'h2'>) => {
  return (
    <h3
      className="typography-xl container col-span-8 col-start-3 pb-1 pt-4 md:col-span-12 md:col-start-1"
      {...props}
    >
      {children}
    </h3>
  )
}

const Img = ({ alt = '', src }: ComponentPropsWithoutRef<'img'>) => {
  if (!src) {
    return null
  }
  return (
    <div className="container relative col-span-12 block aspect-video w-full overflow-hidden rounded-lg">
      <Image className="object-cover" fill alt={alt} src={src} />
    </div>
  )
}

const List = <As extends React.ElementType = 'ul'>({
  children,
  as,
  ...props
}: TPolymorphicComponentProps<As>) => {
  const Component = as ?? 'ul'

  // const ordered = Component === "ol"

  return (
    <Component
      className={classNames(
        'container col-span-8 col-start-3 flex flex-col gap-1.5 md:col-span-12 md:col-start-1',
        {
          '[&>li]:list-inside [&>li]:list-decimal': true // ordered,
        }
      )}
      {...props}
    >
      {children}
    </Component>
  )
}

const Li = ({ children, ...props }: ComponentPropsWithoutRef<'li'>) => {
  return (
    <li className="text-base" {...props}>
      {children}
    </li>
  )
}

const Pre = ({ children, ...props }: ComponentPropsWithoutRef<'pre'>) => {
  if (isValidElement(children) && children.type === 'code') {
    const language = children.props.className.split('-').pop()
    const code = children.props.children

    if (language === 'mermaid') {
      return (
        <div className="container col-span-8 col-start-3 md:col-span-12 md:col-start-1">
          <Mermaid chart={code as string} />
        </div>
      )
    }

    return (
      <CodeBlock
        language={language}
        code={code}
        className="container col-span-8 col-start-3 md:col-span-12 md:col-start-1"
      />
    )
  }

  return null
}

const Hr = (props: ComponentPropsWithoutRef<'hr'>) => {
  return <hr className="bg-stroke col-span-12 col-start-1 my-6" {...props} />
}

const Blockquote = (props: ComponentPropsWithoutRef<'blockquote'>) => {
  return (
    <blockquote
      className="bg-secondary container col-span-8 col-start-3 my-5 rounded-md p-5 md:col-span-12 md:col-start-1 [&>*]:text-inherit"
      {...props}
    />
  )
}

const getPostComponents = (data: Post['data']) => ({
  PostHeader: (props: Omit<ComponentPropsWithoutRef<typeof PostHeader>, 'data'>) => (
    <PostHeader data={data} {...props} />
  ),
  p: P,
  h1: H2,
  h2: H2,
  h3: H3,
  img: Img,
  pre: Pre,
  ul: (props: ComponentPropsWithoutRef<typeof List>) => <List {...props} as="ul" />,
  ol: (props: ComponentPropsWithoutRef<typeof List>) => <List {...props} as="ol" />,
  li: Li,
  a: Link,
  hr: Hr,
  blockquote: Blockquote
})
export function MDXRenderer({
  data,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof MDXRemote>, 'components'> & {
  data: Post['data']
}) {
  const components = getPostComponents(data) as unknown as null

  return (
    <section className="container">
      <MDXRemote {...props} components={components} />
    </section>
  )
}
