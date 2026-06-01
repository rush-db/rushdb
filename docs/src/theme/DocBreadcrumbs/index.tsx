import React from 'react'
import { useSidebarBreadcrumbs } from '@docusaurus/plugin-content-docs/client'
import CopyPageButton from '@site/src/components/CopyPageButton'

export default function DocBreadcrumbsWrapper() {
  const breadcrumbs = useSidebarBreadcrumbs()
  const sectionName = breadcrumbs?.[0]?.label

  return (
    <div className="doc-actions">
      {sectionName && <span className="doc-section-label">{sectionName}</span>}
      <CopyPageButton />
    </div>
  )
}
