import classNames from 'classnames'
import { ArrowUpRight, CheckIcon, Code, Cpu, Database, FileText, Layers, Search, Zap } from 'lucide-react'
import NextLink from 'next/link'
import { FunctionComponent, ReactNode } from 'react'
import { Grid, GridItem } from '~/components/Grid'

import { Section } from '~/components/Section'
import cx from 'classnames'

const colors = {
  orange: 'text-accent-orange',
  green: 'text-accent-green',
  red: 'text-accent-red',
  pink: 'text-accent-pink',
  blue: 'text-accent-blue',
  purple: 'text-accent-purple'
}

const colorsBorder = {
  orange: 'from-accent-orange/30',
  green: 'from-accent-green/30',
  red: 'from-accent-red/30',
  pink: 'from-accent-pink/30',
  blue: 'from-accent-blue/30',
  purple: 'from-accent-purple/30'
}

const feats = [
  {
    icon: Database,
    title: 'Next Generation Database',
    className: cx('row-span-1 col-span-1'),
    description:
      'RushDB is a graph-based Firebase alternative, built for modern apps of any scale. ACID compliant and can handle billions of nodes and relationships.',
    color: 'green'
  },

  {
    icon: Search,
    title: 'Ultimate Querying',
    className: cx('row-span-1'),
    color: 'blue',
    description:
      'Handle complex datasets effortlessly and fast, without needing a query language—just use simple declarative descriptions.'
  },

  {
    icon: Layers,
    color: 'red',
    title: 'Framework Agnostic',
    className: cx('row-span-1 col-span-1'),
    description:
      'RushDB integrates easily with any tech stack or language using its REST API and SDK, offering simple yet powerful functionality.'
  }
] as const

const Feat = ({
  title,
  description,
  icon: Icon,
  color = 'orange',
  className,
  idx
}: {
  title: ReactNode
  description: ReactNode
  icon: FunctionComponent<{ className?: string }>
  color: keyof typeof colors
  className: string
  idx: number
}) => {
  return (
    <GridItem
      idx={idx}
      lastOfFirstRow="!bg-gradient-to-bl"
      firstOfLastRow="!bg-gradient-to-tr"
      firstOfMiddleRow="bg-gradient-to-r"
      lastOfMiddleRow="bg-gradient-to-l"
      middleOfFirstRow="bg-gradient-to-b"
      middleOfLastRow="bg-gradient-to-t"
      className={classNames(
        colorsBorder[color],
        className,
        'to-stroke group relative grid rounded-xl p-0.5',
        'first:bg-gradient-to-br',
        'last:bg-gradient-to-tl'
      )}
    >
      <div
        className={classNames('bg-fill relative flex overflow-hidden rounded-3xl p-7', 'rounded-[inherit]')}
      >
        <div
          className="to-fill pointer-events-none absolute left-0 top-0 w-full bg-gradient-to-b from-transparent"
          aria-hidden
        >
          <div className="to-fill pointer-events-none absolute inset-0 h-full w-full bg-gradient-to-b from-transparent to-60%" />
        </div>

        <div className="relative z-10 flex flex-col gap-4 self-center">
          <Icon className={classNames('h-12 w-12', colors[color])} />

          <h3 className="typography-lg text-content font-semibold">
            {title}
            <NextLink href="">
              <ArrowUpRight
                className={classNames(
                  'mb-0.5 ml-1 translate-y-full opacity-0 transition group-hover:translate-y-0 group-hover:opacity-100',
                  colors[color]
                )}
              />
            </NextLink>
          </h3>

          <p className="typography-base text-content2">{description}</p>
        </div>
      </div>
    </GridItem>
  )
}

export function FeaturesCards() {
  return (
    <Section className="container">
      <Grid className="grid-rows-1 gap-5" desktopCols={3} tabletCols={2} mobileCols={1}>
        {feats.map((feat, idx) => {
          return (
            <Feat
              key={feat.title}
              icon={feat.icon}
              className={feat.className}
              title={feat.title}
              description={feat.description}
              color={feat.color}
              idx={idx}
            />
          )
        })}
      </Grid>
    </Section>
  )
}
