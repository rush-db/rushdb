import React, { useMemo, useState } from 'react'
import { usePluginData } from '@docusaurus/useGlobalData'

export type TutorialEntry = {
  id: string
  title: string
  description: string
  href: string
  tags: string[]
  time: string
}

const BORDER_CLASS = 'border-[var(--ifm-color-emphasis-200)]'

const SearchIcon = () => (
  <svg
    width="16"
    height="16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="11" cy="11" r="8" />
    <path d="m21 21-4.35-4.35" />
  </svg>
)

const ClockIcon = () => (
  <svg
    width="12"
    height="12"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <circle cx="12" cy="12" r="10" />
    <polyline points="12 6 12 12 16 14" />
  </svg>
)

const ArrowIcon = () => (
  <svg
    width="14"
    height="14"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    <path d="M5 12h14" />
    <path d="m12 5 7 7-7 7" />
  </svg>
)

function TutorialCard({ tutorial: t }: { tutorial: TutorialEntry }) {
  return (
    <a
      href={t.href}
      className={`group flex flex-col rounded-xl border ${BORDER_CLASS} bg-[var(--ifm-card-background-color)] p-6 text-inherit no-underline transition-[background-color,border-color,color] duration-150 ease-out hover:bg-[var(--ifm-color-emphasis-100)] hover:no-underline focus:no-underline active:no-underline`}
    >
      <div className="mb-3.5 flex flex-wrap gap-4">
        {t.tags.map((tag) => (
          <span key={tag} className="tabs__item pointer-events-none !m-0 select-none !p-0">
            {tag}
          </span>
        ))}
      </div>

      <h3 className="mb-2.5 text-[17px] font-bold leading-[1.35] text-[var(--ifm-font-color-base)] no-underline">
        {t.title}
      </h3>

      <p className="grow pb-5 text-sm leading-[1.65] text-[var(--ifm-color-emphasis-700)] no-underline">
        {t.description}
      </p>

      <div className={`flex items-center justify-between border-t ${BORDER_CLASS} pt-3.5`}>
        <span className="flex items-center gap-1.5 text-xs text-[var(--ifm-color-emphasis-500)]">
          <ClockIcon />
          {t.time}
        </span>
        <span className="flex items-center gap-1.5 text-[13px] font-semibold text-[var(--ifm-color-emphasis-600)] transition-colors duration-150 ease-out group-hover:text-[var(--ifm-font-color-base)]">
          Read <ArrowIcon />
        </span>
      </div>
    </a>
  )
}

export default function TutorialsIndex() {
  const tutorials = usePluginData('tutorials-data') as TutorialEntry[]
  const [search, setSearch] = useState('')
  const [activeTag, setActiveTag] = useState<string | null>(null)

  const allTags = useMemo<string[]>(() => {
    const seen = new Set<string>()
    for (const tutorial of tutorials) {
      for (const tag of tutorial.tags) {
        seen.add(tag)
      }
    }
    return [...seen]
  }, [tutorials])

  const filtered = useMemo(() => {
    return tutorials.filter((tutorial) => {
      const query = search.toLowerCase()
      const matchesSearch =
        !search ||
        tutorial.title.toLowerCase().includes(query) ||
        tutorial.description.toLowerCase().includes(query)
      const matchesTag = !activeTag || tutorial.tags.includes(activeTag)
      return matchesSearch && matchesTag
    })
  }, [tutorials, search, activeTag])

  return (
    <div className="language-tabs not-prose">
      <div className="mb-10 mt-10 text-center">
        <h1 className="mb-3 text-4xl font-bold text-[var(--ifm-font-color-base)]">Tutorials</h1>
        <p className="m-0 text-[1.1rem] text-[var(--ifm-color-emphasis-600)]">
          Hands-on guides to build real things with RushDB
        </p>
      </div>

      <div className="relative mx-auto mb-8 max-w-[560px]">
        <div className="pointer-events-none absolute inset-y-0 left-4 flex items-center text-[var(--ifm-color-emphasis-500)]">
          <SearchIcon />
        </div>
        <input
          type="text"
          placeholder="Search tutorials..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className={`box-border w-full rounded-md border border-solid ${BORDER_CLASS} focus:border-accent bg-transparent py-3 pl-11 pr-4 text-[15px] text-[var(--ifm-font-color-base)] outline-none transition-colors duration-150 ease-out placeholder:text-[var(--ifm-color-emphasis-500)]`}
        />
      </div>

      <div className="tabs flex flex-wrap justify-center pb-10">
        {['All', ...allTags].map((tag) => {
          const isActive = tag === 'All' ? !activeTag : tag === activeTag

          return (
            <button
              key={tag}
              onClick={() =>
                setActiveTag(
                  tag === 'All' ? null
                  : activeTag === tag ? null
                  : tag
                )
              }
              className={`tabs__item !m-0 cursor-pointer border-0 !bg-transparent transition-[background-color,color] duration-150 ease-out ${isActive ? 'tabs__item--active' : ''}`}
            >
              {tag}
            </button>
          )
        })}
      </div>

      {filtered.length > 0 ?
        <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
          {filtered.map((tutorial) => (
            <TutorialCard key={tutorial.href} tutorial={tutorial} />
          ))}
        </div>
      : <div className="py-20 text-center text-[15px] text-[var(--ifm-color-emphasis-500)]">
          No tutorials match your search.
        </div>
      }
    </div>
  )
}
