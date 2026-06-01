import React from 'react'
import { translate } from '@docusaurus/Translate'
import { useNavbarMobileSidebar } from '@docusaurus/theme-common/internal'
import NavbarColorModeToggle from '@theme/Navbar/ColorModeToggle'
import NavbarLogo from '@theme/Navbar/Logo'
import { X } from 'lucide-react'

function CloseButton() {
  const mobileSidebar = useNavbarMobileSidebar()

  return (
    <button
      type="button"
      aria-label={translate({
        id: 'theme.docs.sidebar.closeSidebarButtonAriaLabel',
        message: 'Close navigation bar',
        description: 'The ARIA label for close button of mobile sidebar'
      })}
      className="clean-btn navbar-sidebar__close"
      onClick={() => mobileSidebar.toggle()}
    >
      <X aria-hidden="true" size={28} strokeWidth={1.75} />
    </button>
  )
}

export default function NavbarMobileSidebarHeader() {
  return (
    <div className="navbar-sidebar__brand">
      <NavbarLogo />
      <NavbarColorModeToggle className="margin-right--md" />
      <CloseButton />
    </div>
  )
}
