import React from 'react'
import { useDoc } from '@docusaurus/plugin-content-docs/client'
import DocBreadcrumbs from '@theme-original/DocBreadcrumbs'
import type DocBreadcrumbsType from '@theme/DocBreadcrumbs'
import type { WrapperProps } from '@docusaurus/types'
import CopyPageButton, { WidescreenButton } from '@site/src/components/CopyPageButton'

type Props = WrapperProps<typeof DocBreadcrumbsType>
type PageFrontMatter = {
  hide_breadcrumbs?: boolean
}

export default function DocBreadcrumbsWrapper(props: Props) {
  const { frontMatter } = useDoc()
  const pageFrontMatter = frontMatter as typeof frontMatter & PageFrontMatter

  if (pageFrontMatter.hide_breadcrumbs) {
    return null
  }

  return (
    <div className="breadcrumbs-with-copy">
      <DocBreadcrumbs {...props} />
      <div className="breadcrumbs-actions">
        <CopyPageButton />
        <WidescreenButton />
      </div>
    </div>
  )
}
