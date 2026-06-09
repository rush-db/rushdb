import React, { useEffect, useState } from 'react'
import Link from '@docusaurus/Link'
import { useColorMode, useThemeConfig } from '@docusaurus/theme-common'
import type { WrapperProps } from '@docusaurus/types'
import DocSidebarDesktop from '@theme-original/DocSidebar/Desktop'
import Logo from '@theme/Logo'
import SidebarCommunity from '@site/src/components/SidebarCommunity'
import ThemeSwitch from '@site/src/components/ThemeSwitch'
import { PanelLeftClose, PanelLeftOpen, Search } from 'lucide-react'
import type DocSidebarDesktopType from '@theme/DocSidebar/Desktop'

type Props = WrapperProps<typeof DocSidebarDesktopType>

function SidebarThemeToggle() {
  const {
    colorMode: currentColorMode,
    setColorMode,
    colorModeChoice
  } = useColorMode() as ReturnType<typeof useColorMode> & {
    colorModeChoice: 'light' | 'dark' | null
  }
  const {
    colorMode: { respectPrefersColorScheme, disableSwitch }
  } = useThemeConfig() as {
    colorMode: { respectPrefersColorScheme?: boolean; disableSwitch?: boolean }
  }
  const [mounted, setMounted] = useState(false)

  useEffect(() => {
    setMounted(true)
  }, [])

  if (disableSwitch) {
    return null
  }

  const nextMode = (() => {
    if (!respectPrefersColorScheme) {
      return currentColorMode === 'dark' ? 'light' : 'dark'
    }

    switch (colorModeChoice) {
      case null:
        return 'light'
      case 'light':
        return 'dark'
      case 'dark':
        return null
      default:
        return 'light'
    }
  })()

  const isDark = currentColorMode === 'dark'

  return (
    <ThemeSwitch
      disabled={!mounted}
      isDark={isDark}
      onToggle={() => setColorMode(isDark ? 'light' : 'dark')}
    />
  )
}

function SidebarCollapseToggle({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      className="clean-btn rushdb-sidebar-control rushdb-sidebar-control--collapse"
      aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      onClick={onToggle}
    >
      {collapsed ?
        <PanelLeftOpen aria-hidden="true" className="rushdb-sidebar-control__icon" />
      : <PanelLeftClose aria-hidden="true" className="rushdb-sidebar-control__icon" />}
    </button>
  )
}

export default function DocSidebarDesktopWrapper(props: Props) {
  const [collapsed, setCollapsed] = useState(false)

  useEffect(() => {
    const savedState = window.localStorage.getItem('rushdb-docs-sidebar-collapsed')
    setCollapsed(savedState === 'true')
  }, [])

  useEffect(() => {
    window.localStorage.setItem('rushdb-docs-sidebar-collapsed', String(collapsed))
  }, [collapsed])

  return (
    <div
      className={
        collapsed ?
          'rushdb-docs-sidebar-shell rushdb-docs-sidebar-shell--collapsed'
        : 'rushdb-docs-sidebar-shell'
      }
    >
      <div className="rushdb-docs-sidebar-header">
        <Logo />
        <Link href="https://app.rushdb.com" className="cta-button rushdb-sidebar-cta">
          GET API KEY
        </Link>
      </div>

      <button
        type="button"
        className="clean-btn rushdb-docs-sidebar-search-trigger"
        onClick={() => document.dispatchEvent(new CustomEvent('rushdb:open-search'))}
        aria-label="Open search"
      >
        <Search aria-hidden="true" className="rushdb-docs-sidebar-search-trigger__icon" size={14} />
        <span className="rushdb-docs-sidebar-search-trigger__label">Search</span>
        <kbd className="rushdb-docs-sidebar-search-trigger__kbd">⌘K</kbd>
      </button>

      <DocSidebarDesktop {...props} />

      <div className="rushdb-docs-sidebar-footer">
        <SidebarCommunity />
        <div className="rushdb-sidebar-controls">
          <SidebarThemeToggle />
          <SidebarCollapseToggle collapsed={collapsed} onToggle={() => setCollapsed((current) => !current)} />
        </div>
      </div>

      <button
        type="button"
        className="clean-btn rushdb-sidebar-expand-rail"
        aria-label="Expand sidebar"
        title="Expand sidebar"
        onClick={() => setCollapsed(false)}
      >
        <PanelLeftOpen aria-hidden="true" className="rushdb-sidebar-expand-rail__icon" />
      </button>
    </div>
  )
}
