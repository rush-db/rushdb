import { ComponentPropsWithoutRef, FunctionComponent, ReactNode } from 'react'
import { Section, SectionHeader, SectionSubtitle, SectionTitle } from '~/components/Section'
import cx from 'classnames'
import { Button } from '~/components/Button'
import { ArrowUpRight, BookOpen, Github } from 'lucide-react'
import { IconDiscord } from '~/components/Layout/IconDiscord'
import { IconX } from '~/components/Layout/IconX'
import Link from 'next/link'
import { socials } from '~/config/urls'
import { RoundedGridItem } from '~/components/RoundedGrid'
import { Grid } from '~/components/Grid'

function CommunityCard({
  title,
  description,
  action,
  icon: Icon,
  className,
  idx,
  ...props
}: ComponentPropsWithoutRef<'div'> & {
  title: ReactNode
  description: ReactNode
  action: ReactNode
  icon: FunctionComponent<{ className?: string }>
  idx: number
}) {
  return (
    <RoundedGridItem
      idx={idx}
      className={cx('bg-secondary flex flex-col items-center gap-3 p-6 text-center', className)}
      {...props}
    >
      {/* @ts-ignore */}
      <Icon className="mt-3 h-12 w-12" strokeWidth="1" />

      <h3 className="typography-lg font-bold">{title}</h3>

      <p className="text-content2 mb-3">{description}</p>

      <div className="mt-auto grid w-full">{action}</div>
    </RoundedGridItem>
  )
}

export function CommunitySection() {
  return (
    <Section className="container">
      <SectionHeader className="m-auto max-w-4xl text-center">
        <h3 className={cx('typography-3xl mb-0 md:text-2xl')}>
          Join the{' '}
          <i>
            <span className="font-special text-[56px] md:text-[48px]">Community</span>
          </i>
        </h3>
        <SectionSubtitle>
          New functionalities in RushDB are inspired by the real-world needs of our users. Have a suggestion?
          Let us know. Your input could spark our next big feature.
        </SectionSubtitle>
      </SectionHeader>

      <Grid desktopCols={4} tabletCols={2} mobileCols={1} className="grid gap-3">
        <CommunityCard
          idx={0}
          title="Blog"
          description="Read the latest news and product updates from the RushDB Blog."
          action={
            <Button size="small" variant="secondary" as={Link} href={socials.blog}>
              Read
              <ArrowUpRight />
            </Button>
          }
          icon={BookOpen}
        />
        <CommunityCard
          idx={1}
          title="Discord"
          description="Join our Discord community to chat with other developers and the RushDB team."
          action={
            <Button size="small" variant="secondary" as={Link} href={socials.discord}>
              Join
              <ArrowUpRight />
            </Button>
          }
          icon={IconDiscord}
        />
        <CommunityCard
          idx={2}
          title="X (Twitter)"
          description="Stay up to date with the latest news from RushDB."
          action={
            <Button size="small" variant="secondary" as={Link} href={socials.x}>
              Follow
              <ArrowUpRight />
            </Button>
          }
          icon={IconX}
        />
        <CommunityCard
          idx={3}
          title="Github"
          description="Dive into our GitHub repository to contribute, explore issues, and suggest enhancements."
          action={
            <Button size="small" variant="secondary" as={Link} href={socials.github}>
              Explore
              <ArrowUpRight />
            </Button>
          }
          icon={Github}
        />
      </Grid>
    </Section>
  )
}
