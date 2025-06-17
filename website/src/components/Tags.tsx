import classNames from 'classnames'

type TagsProps = {
  tags: string[]
  limit?: number
  className?: string
  tagClassName?: string
}

export function Tags({ tags, limit = Infinity, className, tagClassName }: TagsProps) {
  const displayTags = tags?.slice(0, limit) || []

  if (displayTags.length === 0) {
    return null
  }

  return (
    <div className={classNames('flex flex-wrap gap-2', className)}>
      {displayTags.map((tag, i) => (
        <span
          key={i}
          className={classNames(
            'bg-secondary dark:bg-content2/25 dark:text-content border-content2/25 dark:border-content2/40 text-content rounded-full border px-2 py-0.5 text-xs font-medium backdrop-blur-sm',
            tagClassName
          )}
        >
          {tag}
        </span>
      ))}
    </div>
  )
}
