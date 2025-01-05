import classNames from "classnames"
import { MDXRemote } from "next-mdx-remote"
import Image from "next/image"
import { ComponentPropsWithoutRef, ReactNode, isValidElement } from "react"
import { CodeBlock } from "~/components/CodeBlock"
import { Faq } from "~/components/Faq"
import { LetterTypingText } from "~/components/LetterTypingText"
import { Link } from "~/components/Link"
import { Post } from "~/sections/blog/types"
import { MainCta } from "~/components/Button"
import { WireBox } from "~/components/WireBox"

const PostHeader = ({
  data,
  title,
  description,
  showDate = true,
}: {
  data: Post["data"]
  title: string
  description?: string
  showDate: boolean
}) => {
  const colors = [
    "text-accent-brand",
    "text-accent-yellow",
    "text-accent-orange",
    "text-accent-green",
    "text-accent-red",
    "text-accent-pink",
    "text-accent-blue",
    "text-accent-purple",
  ]

  const randomColor = colors[Math.floor(Math.random() * colors.length)]

  return (
    <section className="grid place-items-center pt-32 pb-16 container relative col-span-12 col-start-1">
      <div
        className="absolute top-0 left-0 h-full w-full pointer-events-none -z-10"
        aria-hidden
      >
        <WireBox
          className={classNames("w-full h-full left-0 top-0 ", randomColor)}
          wireColor="currentColor"
        />
        <div className="absolute inset-0 h-full w-full to-90% bg-gradient-to-b from-transparent to-fill pointer-events-none" />
      </div>

      {data.date && showDate ? (
        <p className="typography-base text-content2 mb-2">{data.date}</p>
      ) : null}

      <LetterTypingText
        as="h1"
        className="typography-4xl !mb-0 md:text-2xl max-w-3xl
    text-center"
        animate
      >
        {title}
      </LetterTypingText>

      {description && (
        <p className="typography-lg  py-6 text-center max-w-5xl">
          {description}
        </p>
      )}

      {/* {data?.tags && (
        <div className="flex gap-2 overflow-auto">
          {data.tags.map((tag) => (
            <Chip variant="purple" key={tag}>
              {tag}
            </Chip>
          ))}
        </div>
      )} */}
    </section>
  )
}

const P = ({ children, ...props }: ComponentPropsWithoutRef<"h2">) => {
  // Check if the only child is an img element
  if (isValidElement(children) && children.props?.mdxType === "img") {
    return children
  }

  return (
    <p
      className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12 text-base text-content2"
      {...props}
    >
      {children}
    </p>
  )
}

const H2 = ({ children, ...props }: ComponentPropsWithoutRef<"h2">) => {
  if (typeof children !== "string") {
    return null
  }

  const id = encodeURI(children)

  return (
    <div className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12 typography-2xl pt-10 pb-4 flex group">
      <LetterTypingText as="h2" id={id} animateInView {...props}>
        {children}
      </LetterTypingText>
      <a
        href={`#${id}`}
        className="ml-1 text-content3 opacity-0 group-hover:opacity-100"
      >
        #
      </a>
    </div>
  )
}

const H3 = ({ children, ...props }: ComponentPropsWithoutRef<"h2">) => {
  return (
    <h3
      className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12 typography-xl pt-4 pb-1"
      {...props}
    >
      {children}
    </h3>
  )
}

const Img = ({ alt = "", src }: ComponentPropsWithoutRef<"img">) => {
  if (!src) {
    return null
  }
  return (
    <div className="block aspect-video container relative col-span-12 w-full rounded-lg overflow-hidden">
      <Image className="object-cover " fill alt={alt} src={src} />
    </div>
  )
}

const List = <As extends React.ElementType = "ul">({
  children,
  as,
  ...props
}: TPolymorphicComponentProps<As>) => {
  const Component = as ?? "ul"

  // const ordered = Component === "ol"

  return (
    <Component
      className={classNames(
        "container col-span-8 col-start-3 md:col-start-1 md:col-span-12 flex flex-col gap-1.5",
        {
          "[&>li]:list-decimal [&>li]:list-inside": true, // ordered,
        },
      )}
      {...props}
    >
      {children}
    </Component>
  )
}

const Li = ({ children, ...props }: ComponentPropsWithoutRef<"li">) => {
  return (
    <li className="text-base" {...props}>
      {children}
    </li>
  )
}

const Pre = ({ children }: ComponentPropsWithoutRef<"pre">) => {
  if (isValidElement(children) && children.props?.mdxType === "code") {
    const language = children.props.className.split("-").pop()
    const code = children.props.children

    return (
      <CodeBlock
        language={language}
        code={code}
        className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12"
      />
    )
  }

  return null
}

const Hr = (props: ComponentPropsWithoutRef<"hr">) => {
  return <hr className="col-start-1 col-span-12 my-6 bg-stroke" {...props} />
}

const Blockquote = (props: ComponentPropsWithoutRef<"blockquote">) => {
  return (
    <blockquote
      className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12 bg-secondary [&>*]:text-inherit my-5 p-5 rounded-md"
      {...props}
    />
  )
}

const CallToAction = ({
  text,
  buttonText,
  description,
}: {
  text: string
  description?: string
  buttonText?: string
}) => (
  <div className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12 bg-secondary my-16 rounded-lg items-center  gap-5 flex p-7 sm:p-5 sm:flex-col flex-row justify-between sm:items-stretch xs:rounded-none">
    <div className="flex flex-col gap-2 max-w-xl">
      <p className="typography-xl">{text}</p>

      {description && <p className="typography-base">{description}</p>}
    </div>

    <MainCta variant="accent" className={"shrink-0"}>
      {buttonText}
    </MainCta>
  </div>
)

const getPostComponents = (data: Post["data"]) =>
  ({
    // TestComponent: dynamic(() => import("../../components/TestComponent")),
    PostHeader: (
      props: Omit<ComponentPropsWithoutRef<typeof PostHeader>, "data">,
    ) => <PostHeader data={data} {...props} />,
    p: P,
    h1: H2,
    h2: H2,
    h3: H3,
    img: Img,
    pre: Pre,
    ul: (props: ComponentPropsWithoutRef<typeof List>) => (
      <List {...props} as="ul" />
    ),
    ol: (props: ComponentPropsWithoutRef<typeof List>) => (
      <List {...props} as="ol" />
    ),
    li: Li,
    a: Link,
    hr: Hr,
    blockquote: Blockquote,
    Faq: (props: ComponentPropsWithoutRef<typeof Faq>) => (
      <Faq
        {...props}
        className="container col-span-8 col-start-3 md:col-start-1 md:col-span-12 mb-10"
      />
    ),
    CallToAction: CallToAction,
  } as unknown as Record<string, ReactNode>)

export function MDXRenderer({
  data,
  ...props
}: Omit<ComponentPropsWithoutRef<typeof MDXRemote>, "components"> & {
  data: Post["data"]
}) {
  const components = getPostComponents(data)

  return (
    <section className="grid grid-cols-12 gap-4">
      <MDXRemote {...props} components={components} />
    </section>
  )
}
