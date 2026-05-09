import React from 'react'
import { useDoc } from '@docusaurus/plugin-content-docs/client'
import DocItemLayout from '@theme-original/DocItem/Layout'
import type DocItemLayoutType from '@theme/DocItem/Layout'
import type { WrapperProps } from '@docusaurus/types'

type Props = WrapperProps<typeof DocItemLayoutType>
type PageFrontMatter = {
  force_container?: boolean
}

export default function DocItemLayoutWrapper(props: Props) {
  const { frontMatter } = useDoc()
  const pageFrontMatter = frontMatter as typeof frontMatter & PageFrontMatter

  if (pageFrontMatter.force_container) {
    return (
      <div className="doc-layout--force-container">
        <DocItemLayout {...props} />
      </div>
    )
  }

  return <DocItemLayout {...props} />
}
