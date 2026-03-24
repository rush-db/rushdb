import React, { useState, Children, isValidElement } from 'react'

interface Tab {
  value: string
  label: string
  isDefault: boolean
  content: React.ReactNode
}

interface ResponsiveTabsProps {
  children: React.ReactNode
}

/**
 * Outer demo tabs that render as tab buttons on desktop and a native <select>
 * on mobile (< 997px). Do NOT use this for programming-language tabs — that's
 * handled by <LanguageTabs>.
 */
export default function ResponsiveTabs({ children }: ResponsiveTabsProps) {
  const tabs: Tab[] = Children.toArray(children)
    .filter(isValidElement)
    .map((child: any) => ({
      value: child.props.value as string,
      label: child.props.label as string,
      isDefault: Boolean(child.props.default),
      content: child.props.children
    }))

  const defaultTab = tabs.find((t) => t.isDefault)?.value ?? tabs[0]?.value ?? ''
  const [selected, setSelected] = useState(defaultTab)

  const activeTab = tabs.find((t) => t.value === selected) ?? tabs[0]

  return (
    <div className="responsive-tabs">
      {/* Mobile: native select */}
      <div className="responsive-tabs__select-wrapper">
        <select
          className="responsive-tabs__select"
          value={selected}
          onChange={(e) => setSelected(e.target.value)}
          aria-label="Select example"
        >
          {tabs.map((t) => (
            <option key={t.value} value={t.value}>
              {t.label}
            </option>
          ))}
        </select>
      </div>

      {/* Desktop: tab buttons */}
      <div className="responsive-tabs__nav" role="tablist">
        {tabs.map((t) => (
          <button
            key={t.value}
            role="tab"
            type="button"
            aria-selected={t.value === selected}
            className={
              t.value === selected ?
                'responsive-tabs__tab responsive-tabs__tab--active'
              : 'responsive-tabs__tab'
            }
            onClick={() => setSelected(t.value)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="responsive-tabs__content" role="tabpanel">
        {activeTab?.content}
      </div>
    </div>
  )
}
