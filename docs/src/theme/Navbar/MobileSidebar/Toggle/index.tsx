import React from 'react'
import { translate } from '@docusaurus/Translate'
import { useNavbarMobileSidebar } from '@docusaurus/theme-common/internal'
import { PanelLeft } from 'lucide-react'

export default function NavbarMobileSidebarToggle() {
  const mobileSidebar = useNavbarMobileSidebar()

  return (
    <button
      type="button"
      aria-label={translate({
        id: 'theme.docs.sidebar.toggleSidebarButtonAriaLabel',
        message: 'Toggle navigation bar',
        description: 'The ARIA label for the mobile sidebar toggle button'
      })}
      aria-expanded={mobileSidebar.shown}
      className="clean-btn navbar__toggle"
      onClick={() => mobileSidebar.toggle()}
    >
      <PanelLeft aria-hidden="true" size={22} strokeWidth={1.75} />
    </button>
  )
}
