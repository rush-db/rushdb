import React, { Children, cloneElement, isValidElement } from 'react'
import OriginalTabs from '@theme-original/Tabs'
import type TabsType from '@theme/Tabs'
import type { WrapperProps } from '@docusaurus/types'

type Props = WrapperProps<typeof TabsType>

/**
 * Wraps every TabItem child with a `data-lang` attribute equal to its `value`.
 * The original Tabs component reads `attributes` from TabItem props and spreads
 * them onto the rendered <li role="tab"> button — so this injects the attribute
 * with zero changes to MDX files.
 */
export default function TabsWrapper({ children, ...props }: Props): React.ReactElement {
  const enhanced = Children.map(children, (child) => {
    if (!isValidElement(child)) return child
    const { value, attributes } = child.props as { value?: string; attributes?: Record<string, unknown> }
    if (!value) return child
    return cloneElement(child as unknown as React.ReactElement<Record<string, unknown>>, {
      attributes: { ...attributes, 'data-lang': value }
    })
  })

  return <OriginalTabs {...props}>{enhanced}</OriginalTabs>
}
