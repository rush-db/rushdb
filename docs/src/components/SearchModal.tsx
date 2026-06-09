import React, { useCallback, useEffect, useRef } from 'react'
import { createPortal } from 'react-dom'
import { useHistory } from '@docusaurus/router'
import SearchBar from '@theme-original/SearchBar'

interface Props {
  isOpen: boolean
  onClose: () => void
}

export default function SearchModal({ isOpen, onClose }: Props) {
  const modalRef = useRef<HTMLDivElement>(null)
  const history = useHistory()

  // Close on any navigation (covers result clicks via history.push)
  useEffect(() => {
    return history.listen(() => onClose())
  }, [history, onClose])

  // Focus the search input when modal opens
  useEffect(() => {
    if (!isOpen) return
    const t = setTimeout(() => {
      const input = modalRef.current?.querySelector<HTMLInputElement>(
        'input[type="search"], input.aa-Input, input[class*="input"]'
      )
      input?.focus()
    }, 50)
    return () => clearTimeout(t)
  }, [isOpen])

  // Escape to close
  useEffect(() => {
    if (!isOpen) return
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [isOpen, onClose])

  // Prevent layout shift caused by scrollbar disappearing
  useEffect(() => {
    if (!isOpen) return
    const scrollbarWidth = window.innerWidth - document.documentElement.clientWidth
    document.documentElement.style.paddingRight = `${scrollbarWidth}px`
    document.documentElement.style.overflow = 'hidden'
    return () => {
      document.documentElement.style.paddingRight = ''
      document.documentElement.style.overflow = ''
    }
  }, [isOpen])

  const handleOverlayClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) onClose()
    },
    [onClose]
  )

  if (!isOpen || typeof document === 'undefined') return null

  return createPortal(
    <div
      className="rushdb-search-overlay"
      onClick={handleOverlayClick}
      role="dialog"
      aria-modal="true"
      aria-label="Search"
    >
      <div className="rushdb-search-modal" ref={modalRef}>
        <div className="rushdb-search-modal__header">
          <SearchBar />
        </div>
        <button
          type="button"
          className="rushdb-search-modal__close clean-btn"
          onClick={onClose}
          aria-label="Close search"
        >
          Esc
        </button>
      </div>
    </div>,
    document.body
  )
}
