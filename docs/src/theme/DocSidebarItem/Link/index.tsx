/**
 * Swizzled DocSidebarItem/Link component.
 * - Connect items: rendered with connect-sidebar-link classes and hardcoded icons.
 * - Any other item: rendered as a plain menu__link with optional customProps.icon (Lucide).
 */
import React, { type ReactNode } from 'react'
import clsx from 'clsx'
import Link from '@docusaurus/Link'
import { ThemeClassNames } from '@docusaurus/theme-common'
import { isActiveSidebarItem } from '@docusaurus/plugin-content-docs/client'
import { Bot, Code2, Cpu, Sparkles } from 'lucide-react'
import * as LucideIcons from 'lucide-react'
import type { LucideProps } from 'lucide-react'

type LucideIconComponent = React.ComponentType<LucideProps>

function ConnectIcon({ href, label }: { href: string; label: string }): ReactNode {
  const path = href.toLowerCase()

  if (path.includes('rushdb.com/agent-setup') || path.startsWith('/connect/agents')) {
    return <Bot size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
  }
  if (path.startsWith('/connect/mcp')) {
    return <Cpu size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
  }
  if (path.startsWith('/connect/sdks')) {
    return <Code2 size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
  }
  if (path === '/connect/' || path === '/connect') {
    return <Sparkles size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
  }

  switch (label) {
    case 'Skills':
      return <Sparkles size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
    case 'MCP':
      return <Cpu size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
    case 'Agents':
      return <Bot size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
    case 'SDKs':
      return <Code2 size={15} strokeWidth={1.75} className="sidebar-link-icon" aria-hidden="true" />
    default:
      return null
  }
}

function GenericIcon({ name }: { name: string }): ReactNode {
  const IconComponent = (LucideIcons as Record<string, unknown>)[name] as LucideIconComponent | undefined
  if (!IconComponent) return null
  return <IconComponent size={15} strokeWidth={1.75} className="sidebar-category-icon" aria-hidden="true" />
}

interface SidebarItemLink {
  type: 'link'
  href: string
  label: string
  className?: string
  customProps?: Record<string, unknown>
  autoAddBaseUrl?: boolean
}

interface Props {
  item: SidebarItemLink
  onItemClick?: (item: SidebarItemLink) => void
  activePath: string
  level: number
  index: number
}

export default function DocSidebarItemLink({
  item,
  onItemClick,
  activePath,
  level,
  index,
  ...props
}: Props): ReactNode {
  const { href, label, className } = item
  const isActive = isActiveSidebarItem(item, activePath)
  const isExternal = /^https?:\/\//.test(href)
  const isConnectItem = className?.includes('connect-sidebar-link')
  const genericIcon = item.customProps?.icon as string | undefined

  if (isConnectItem) {
    return (
      <li
        className={clsx(
          ThemeClassNames.docs.docSidebarItemLink,
          ThemeClassNames.docs.docSidebarItemLinkLevel(level),
          'menu__list-item',
          className
        )}
      >
        <Link
          className={clsx('menu__link', 'connect-sidebar-link__link', {
            'menu__link--active': isActive,
            'connect-sidebar-link__link--external': isExternal
          })}
          aria-current={isActive ? 'page' : undefined}
          to={href}
          onClick={onItemClick ? () => onItemClick(item) : undefined}
          {...props}
        >
          <ConnectIcon href={href} label={label} />
          <span className="connect-sidebar-link__label">{label}</span>
        </Link>
      </li>
    )
  }

  return (
    <li
      className={clsx(
        ThemeClassNames.docs.docSidebarItemLink,
        ThemeClassNames.docs.docSidebarItemLinkLevel(level),
        'menu__list-item'
      )}
    >
      <Link
        className={clsx('menu__link', { 'menu__link--active': isActive })}
        aria-current={isActive ? 'page' : undefined}
        to={href}
        onClick={onItemClick ? () => onItemClick(item) : undefined}
        {...props}
      >
        {genericIcon && <GenericIcon name={genericIcon} />}
        {label}
      </Link>
    </li>
  )
}
