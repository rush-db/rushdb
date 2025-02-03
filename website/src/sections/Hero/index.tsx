import cx from 'classnames'
import Link from 'next/link'
import { BookIcon } from 'lucide-react'

import { Button, MainCta } from '~/components/Button'
import { links, socials } from '~/config/urls'
import { CodeBlock } from '~/components/CodeBlock'
import { GitHub } from '~/components/Icons/GitHub'
import { CodeBlockWithLanguageSelector } from '~/components/CodeBlockWithLanguageSelector'
import { useContext } from 'react'
import { CodingLanguage } from '~/pages'

const code = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('rushdb-api-key')

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

const codePy = `from rushdb import RushDB

db = RushDB("API_TOKEN")

user = db.records.create(
    "USER",
    {
        "name": "Paul Schmitz",
        "lastActive": "2024-12-02T11:38:29Z",
        "verified": True,
        "size": 9.5,
        "favoriteTags": ["Daily Run", "Foam"]
    }
)

db.records.find(
    {
        "labels": ["USER"],
        "where": {
            "size": {"$gte": 9}, 
            "favoriteTags": {"$in": ["Foam"]}
        }
    }
)
`

export const Hero = () => {
  const { language } = useContext(CodingLanguage)

  return (
    <>
      <div id="hero" className="hero absolute top-0 z-0 min-h-dvh w-full md:bg-cover" />

      <section
        className={cx(
          'container z-10 mt-[-97px] grid min-h-dvh place-content-center items-center',
          'md:mt-0'
        )}
      >
        <div className="z-0 flex min-h-dvh flex-col">
          <div className="mb-4 mt-32 grid grow grid-cols-2 items-center gap-24 md:mt-0 md:grid-cols-1 md:gap-0 md:text-center">
            <div className="mb-8 items-center md:mt-24">
              <h1 role="heading" className={cx('typography-4xl !font-extrabold sm:text-2xl')}>
                Instant database
                <br />
                for modern apps<span className="hidden"> - RushDB</span>
              </h1>
              <h2
                className={cx(
                  'text-content3 text-md mb-8 mt-10 !font-medium !tracking-normal md:mt-0 md:text-base'
                )}
              >
                Focus on building features. RushDB handles the rest.
              </h2>

              <Link href={socials.github} target="__blank" rel="noopener noreferrer" aria-label="Github">
                <div className="bg-secondary m-auto mt-16 hidden w-fit items-center gap-4 rounded-full border p-2 px-4 md:flex">
                  <p className="hidden font-mono text-base font-bold md:block">
                    v{process.env.NEXT_PUBLIC_APP_VERSION}
                  </p>
                  <GitHub className="h-5 w-5" />
                </div>
              </Link>
              <div className="mt-16 flex gap-4 md:justify-center">
                <MainCta variant="accent" size="small" text="Create Project" />
                <Button as={Link} href={links.getStarted} variant="outline" size="small" className="bg-fill">
                  Docs <BookIcon />
                </Button>
              </div>
            </div>

            <div className="flex w-full items-center justify-start md:hidden md:flex-col">
              <CodeBlockWithLanguageSelector
                data={{
                  typescript: code,
                  python: codePy
                }}
                className="grid md:hidden md:w-full lg:w-fit"
                preClassName="md:w-full"
              />
            </div>
          </div>
          <div className="w-full justify-center self-end pb-16 text-center align-bottom md:hidden md:flex-col md:pb-4">
            <p className="text-content3 text-md mb-4 text-center !font-medium md:text-lg">
              RushDB is an open-source, graph-powered zero-config database
            </p>
            <CodeBlock
              code={language === 'typescript' ? 'pnpm add @rushdb/javascript-sdk' : 'pip install rushdb'}
              className="m-auto grid place-content-center md:w-full lg:w-fit"
              preClassName="md:w-full"
              copyButton={true}
            >
              <img
                src={
                  language === 'typescript' ?
                    'https://img.shields.io/npm/v/@rushdb/javascript-sdk'
                  : 'https://img.shields.io/pypi/v/rushdb'
                }
                alt="npm-version"
                className="md:hidden"
              />
            </CodeBlock>
          </div>
        </div>
      </section>
    </>
  )
}
