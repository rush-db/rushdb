import cx from 'classnames'
import Link from 'next/link'
import { BookIcon, CopyIcon } from 'lucide-react'

import { Button, MainCta } from '~/components/Button'
import { links } from '~/config/urls'
import { CodeBlock } from '~/components/CodeBlock'
import { copyToClipboard } from '~/utils/copyToClipboard'

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
      <div className="hero absolute top-0 z-0 h-dvh w-full" />

      <section
        className={cx(
          'container z-10 mt-16 grid place-content-center items-center',
          'sm:static sm:h-auto sm:place-content-start sm:pb-12 sm:pt-32 md:!grid-cols-1'
        )}
      >
        <div className="z-0 flex flex-col content-center items-center justify-center gap-8">
          <div className="mb-4 grid grid-cols-2 items-center gap-24">
            <div className="mb-8 items-center">
              <h1 role="heading" className={cx('typography-4xl !font-extrabold sm:text-2xl')}>
                Instant database
                <br />
                for modern apps
              </h1>
              <h2 className={cx('text-content3 text-md mb-8 !font-medium !tracking-normal md:text-lg')}>
                Focus on frontend and features,
                <br /> RushDB handles the rest
              </h2>
              <div className="flex gap-4">
                <MainCta variant="accent" text="Create Project" />
                <Button as={Link} href={links.getStarted} variant="outline" className="bg-fill">
                  Docs <BookIcon />
                </Button>
              </div>
            </div>
            <div className={cx('flex w-full items-center justify-start md:flex-col')}>
              <CodeBlock
                code={code}
                className="grid place-content-center md:w-full lg:w-fit"
                preClassName="md:w-full"
              />
            </div>
          </div>
          <div className={cx('w-full justify-center text-center align-bottom md:flex-col')}>
            <p className={cx('text-content3 text-md mb-4 text-center !font-medium md:text-lg')}>
              RushDB is an open-source, graph-powered alternative to Firebase
            </p>
            <CodeBlock
              code={'pnpm add @rushdb/javascript-sdk'}
              className="m-auto grid place-content-center md:w-full lg:w-fit"
              preClassName="md:w-full"
              copyButton={true}
            >
              <img src="https://img.shields.io/npm/v/@rushdb/javascript-sdk" alt="npm-version" />
            </CodeBlock>
          </div>
        </div>
      </section>
    </>
  )
}
