import NextLink from "next/link"
import { ReactNode } from "react"

export function Link({
  href,
  children,
}: {
  href: string
  children: ReactNode
}) {
  return (
    <NextLink
      className="transition italic underline text-content2 hover:text-accent underline-offset-4"
      href={href}
    >
      {children}
    </NextLink>
  )
}
