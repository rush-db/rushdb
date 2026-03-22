import React from 'react'
import DocBreadcrumbs from '@theme-original/DocBreadcrumbs'
import type DocBreadcrumbsType from '@theme/DocBreadcrumbs'
import type { WrapperProps } from '@docusaurus/types'
import CopyPageButton, { WidescreenButton } from '@site/src/components/CopyPageButton'

type Props = WrapperProps<typeof DocBreadcrumbsType>

export default function DocBreadcrumbsWrapper(props: Props) {
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
