/**
 * Swizzled DocSidebarItem/Category component.
 * Adds Lucide icon support via `customProps.icon` on sidebar category items.
 *
 * Usage in sidebars.ts:
 *   { type: 'category', label: 'Deploy', customProps: { icon: 'Server' }, ... }
 */
import React, { useEffect, useMemo, type ReactNode } from 'react'
import clsx from 'clsx'
import {
  ThemeClassNames,
  useThemeConfig,
  usePrevious,
  Collapsible,
  useCollapsible
} from '@docusaurus/theme-common'
import { isSamePath } from '@docusaurus/theme-common/internal'
import {
  isActiveSidebarItem,
  findFirstSidebarItemLink,
  useDocSidebarItemsExpandedState
} from '@docusaurus/plugin-content-docs/client'
import Link from '@docusaurus/Link'
import { translate } from '@docusaurus/Translate'
import useIsBrowser from '@docusaurus/useIsBrowser'
import DocSidebarItems from '@theme/DocSidebarItems'
import * as LucideIcons from 'lucide-react'
import type { LucideProps } from 'lucide-react'

type LucideIconComponent = React.ComponentType<LucideProps>

function SidebarIcon({ name }: { name: string }): ReactNode {
  const IconComponent = (LucideIcons as Record<string, unknown>)[name] as LucideIconComponent | undefined
  if (!IconComponent) return null
  return <IconComponent size={15} strokeWidth={1.75} className="sidebar-category-icon" aria-hidden="true" />
}

function useAutoExpandActiveCategory({
  isActive,
  collapsed,
  updateCollapsed
}: {
  isActive: boolean
  collapsed: boolean
  updateCollapsed: (toCollapsed?: boolean) => void
}) {
  const wasActive = usePrevious(isActive)
  useEffect(() => {
    const justBecameActive = isActive && !wasActive
    if (justBecameActive && collapsed) {
      updateCollapsed(false)
    }
  }, [isActive, wasActive, collapsed, updateCollapsed])
}

function useCategoryHrefWithSSRFallback(item: SidebarItemCategory): string | undefined {
  const isBrowser = useIsBrowser()
  return useMemo(() => {
    if (item.href && !item.linkUnlisted) {
      return item.href
    }
    if (isBrowser || !item.collapsible) {
      return undefined
    }
    return findFirstSidebarItemLink(item)
  }, [item, isBrowser])
}

function CollapseButton({
  collapsed,
  categoryLabel,
  onClick
}: {
  collapsed: boolean
  categoryLabel: string
  onClick: React.MouseEventHandler<HTMLButtonElement>
}) {
  return (
    <button
      aria-label={
        collapsed ?
          translate(
            {
              id: 'theme.DocSidebarItem.expandCategoryAriaLabel',
              message: "Expand sidebar category '{label}'",
              description: 'The ARIA label to expand the sidebar category'
            },
            { label: categoryLabel }
          )
        : translate(
            {
              id: 'theme.DocSidebarItem.collapseCategoryAriaLabel',
              message: "Collapse sidebar category '{label}'",
              description: 'The ARIA label to collapse the sidebar category'
            },
            { label: categoryLabel }
          )
      }
      aria-expanded={!collapsed}
      type="button"
      className="clean-btn menu__caret"
      onClick={onClick}
    />
  )
}

// Minimal local type — mirrors PropSidebarItemCategory from Docusaurus
interface SidebarItemCategory {
  type: 'category'
  href?: string
  label: string
  items: unknown[]
  collapsible: boolean
  collapsed: boolean
  className?: string
  customProps?: Record<string, unknown>
  linkUnlisted?: boolean
}

interface Props {
  item: SidebarItemCategory
  onItemClick?: (item: SidebarItemCategory) => void
  activePath: string
  level: number
  index: number
}

export default function DocSidebarItemCategory({
  item,
  onItemClick,
  activePath,
  level,
  index,
  ...props
}: Props): ReactNode {
  const { items, label, collapsible, className, href } = item
  const iconName = item.customProps?.icon as string | undefined

  const {
    docs: {
      sidebar: { autoCollapseCategories }
    }
  } = useThemeConfig()

  const hrefWithSSRFallback = useCategoryHrefWithSSRFallback(item)
  const isActive = isActiveSidebarItem(item, activePath)
  const isCurrentPage = isSamePath(href, activePath)

  const { collapsed, setCollapsed } = useCollapsible({
    initialState: () => {
      if (!collapsible) {
        return false
      }
      return isActive ? false : item.collapsed
    }
  })

  const { expandedItem, setExpandedItem } = useDocSidebarItemsExpandedState()

  const updateCollapsed = (toCollapsed = !collapsed) => {
    setExpandedItem(toCollapsed ? null : index)
    setCollapsed(toCollapsed)
  }

  useAutoExpandActiveCategory({ isActive, collapsed, updateCollapsed })

  useEffect(() => {
    if (collapsible && expandedItem != null && expandedItem !== index && autoCollapseCategories) {
      setCollapsed(true)
    }
  }, [collapsible, expandedItem, index, setCollapsed, autoCollapseCategories])

  return (
    <li
      className={clsx(
        ThemeClassNames.docs.docSidebarItemCategory,
        ThemeClassNames.docs.docSidebarItemCategoryLevel(level),
        'menu__list-item',
        {
          'menu__list-item--collapsed': collapsed
        },
        className
      )}
    >
      <div
        className={clsx('menu__list-item-collapsible', {
          'menu__list-item-collapsible--active': isCurrentPage
        })}
      >
        <Link
          className={clsx('menu__link', {
            'menu__link--sublist': collapsible,
            'menu__link--sublist-caret': collapsible,
            'menu__link--active': isActive
          })}
          onClick={
            collapsible ?
              (e) => {
                onItemClick?.(item)
                if (href) {
                  if (isCurrentPage) {
                    e.preventDefault()
                  }
                  updateCollapsed()
                } else {
                  e.preventDefault()
                  updateCollapsed()
                }
              }
            : () => {
                onItemClick?.(item)
              }
          }
          aria-current={isCurrentPage ? 'page' : undefined}
          role={collapsible && !href ? 'button' : undefined}
          aria-expanded={collapsible && !href ? !collapsed : undefined}
          href={collapsible ? (hrefWithSSRFallback ?? '#') : hrefWithSSRFallback}
          {...props}
        >
          {iconName && <SidebarIcon name={iconName} />}
          {label}
        </Link>
      </div>

      <Collapsible lazy as="ul" className="menu__list" collapsed={collapsed}>
        <DocSidebarItems
          items={items as Parameters<typeof DocSidebarItems>[0]['items']}
          tabIndex={collapsed ? -1 : 0}
          onItemClick={onItemClick as Parameters<typeof DocSidebarItems>[0]['onItemClick']}
          activePath={activePath}
          level={level + 1}
        />
      </Collapsible>
    </li>
  )
}
