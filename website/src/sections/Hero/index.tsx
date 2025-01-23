import cx from 'classnames'
import Link from 'next/link'
import { BookIcon } from 'lucide-react'

import { Button, MainCta } from '~/components/Button'
import { links } from '~/config/urls'
import { CodeBlock } from '~/components/CodeBlock'

const code = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('api_token')

const user = await db.records.create('USER', {
  name: 'Paul Schmitz',
  lastActive: '2024-12-02T11:38:29Z',
  verified: true,
  size: 9.5,
  favoriteTags: ['Daily Run', 'Foam']
})

await db.records.find({
  labels: ['USER'],
  where: {
    size: { $gte: 9 },
    favoriteTags: {
      $in: ['Foam']
    }
  }
})`

export const Hero = () => {
  return (
    <>
      <div className="hero absolute top-0 z-0 min-h-dvh w-full md:bg-cover" />

      <section
        className={cx(
          'container z-10 mt-[-97px] grid min-h-dvh place-content-center items-center',
          'md:mt-0'
        )}
      >
        <div className="z-0 flex min-h-dvh flex-col items-center justify-between">
          <div className="mb-4 grid grow grid-cols-2 items-center gap-24 self-center md:grid-cols-1 md:gap-0 md:text-center">
            <div className="mb-8 items-center">
              <h1 role="heading" className={cx('typography-4xl !font-extrabold sm:text-2xl')}>
                Instant database
                <br />
                for modern apps
              </h1>
              <h2 className={cx('text-content3 text-md mb-8 !font-medium !tracking-normal md:text-lg')}>
                Focus on building features. <br />
                RushDB handles the rest.
              </h2>
              <div className="flex gap-4 md:justify-center">
                <MainCta variant="accent" text="Create Project" />
                <Button as={Link} href={links.getStarted} variant="outline" className="bg-fill">
                  Docs <BookIcon />
                </Button>
              </div>
            </div>
            <div className={cx('flex w-full items-center justify-start md:hidden md:flex-col')}>
              <CodeBlock
                code={code}
                className="grid place-content-center md:hidden md:w-full lg:w-fit"
                preClassName="md:w-full"
              />
            </div>
          </div>
          <div
            className={cx(
              'w-full justify-center self-end pb-16 text-center align-bottom md:hidden md:flex-col md:pb-4'
            )}
          >
            <p className={cx('text-content3 text-md mb-4 text-center !font-medium md:text-lg')}>
              RushDB is an open-source, graph-powered <span className="underline">zero-config</span> database
            </p>
            <CodeBlock
              code={'pnpm add @rushdb/javascript-sdk'}
              className="m-auto grid place-content-center md:w-full lg:w-fit"
              preClassName="md:w-full"
              copyButton={true}
            >
              <img
                src="https://img.shields.io/npm/v/@rushdb/javascript-sdk"
                alt="npm-version"
                className="md:hidden"
              />
            </CodeBlock>
            <img
              src="https://img.shields.io/npm/v/@rushdb/javascript-sdk"
              alt="npm-version"
              className="m-auto hidden py-8 md:block"
            />
          </div>
        </div>
      </section>
    </>
  )
}
