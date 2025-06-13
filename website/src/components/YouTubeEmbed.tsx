import { ComponentPropsWithoutRef } from 'react'

interface YouTubeEmbedProps extends ComponentPropsWithoutRef<'div'> {
  videoId: string
  title?: string
  aspectRatio?: '16:9' | '4:3'
}

export const YouTubeEmbed = ({
  videoId,
  title = 'YouTube video',
  aspectRatio = '16:9',
  className = '',
  ...props
}: YouTubeEmbedProps) => {
  const aspectRatioClass = aspectRatio === '16:9' ? 'aspect-video' : 'aspect-[4/3]'

  return (
    <div className={`${aspectRatioClass} w-full ${className}`} {...props}>
      <iframe
        src={`https://www.youtube.com/embed/${videoId}`}
        title={title}
        frameBorder="0"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
        allowFullScreen
        className="h-full w-full rounded-lg"
      />
    </div>
  )
}
