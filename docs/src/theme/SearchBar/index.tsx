import React from 'react'
import { Search } from 'lucide-react'

function openSearchModal() {
  document.dispatchEvent(new CustomEvent('rushdb:open-search'))
}

export default function SearchBar() {
  return (
    <button
      type="button"
      className="clean-btn navbar__search-trigger"
      onClick={openSearchModal}
      aria-label="Search"
    >
      <Search size={15} aria-hidden="true" className="navbar__search-trigger__icon" />
      <span className="navbar__search-trigger__label">Search</span>
      <kbd className="navbar__search-trigger__kbd">⌘K</kbd>
    </button>
  )
}
