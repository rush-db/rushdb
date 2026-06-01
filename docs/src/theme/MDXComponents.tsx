import React from 'react'
import MDXComponents from '@theme-original/MDXComponents'
import type { MDXComponentsObject } from '@theme/MDXComponents'

function Table({ children, ...props }: React.TableHTMLAttributes<HTMLTableElement>) {
  return (
    <div className="markdown-table-wrapper">
      <table {...props}>{children}</table>
    </div>
  )
}

export default {
  ...MDXComponents,
  table: Table
} satisfies MDXComponentsObject
