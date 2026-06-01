import React from 'react'

export default function SidebarCommunity({ className }: { className?: string }) {
  return (
    <div className={['rushdb-sidebar-community', className].filter(Boolean).join(' ')}>
      <a
        href="https://rushdb.com/blog"
        target="_blank"
        rel="noopener noreferrer"
        className="rushdb-sidebar-community__link"
      >
        Blog
      </a>
      <a
        href="https://x.com/RushDatabase"
        target="_blank"
        rel="noopener noreferrer"
        className="rushdb-sidebar-community__link"
      >
        X (Twitter)
      </a>
      <a
        href="https://www.linkedin.com/company/rushdb"
        target="_blank"
        rel="noopener noreferrer"
        className="rushdb-sidebar-community__link"
      >
        LinkedIn
      </a>
      <a
        href="https://github.com/rush-db/rushdb"
        target="_blank"
        rel="noopener noreferrer"
        className="rushdb-sidebar-community__link"
      >
        GitHub
      </a>
      <span className="rushdb-sidebar-community__copy">
        © {new Date().getFullYear()}, Collect Software Inc.
      </span>
    </div>
  )
}
