import Link from 'next/link'
import { BookIcon, Check, Play, X } from 'lucide-react'

import { Button, MainCta } from '~/components/Button'
import { socials } from '~/config/urls'
import { CodeBlock } from '~/components/CodeBlock'
import { GitHub } from '~/components/Icons/GitHub'
import { CodeBlockWithLanguageSelector } from '~/components/CodeBlockWithLanguageSelector'
import { useContext, useState } from 'react'
import { CodingLanguage } from '~/contexts/CodingLanguage'
import Image from 'next/image'

const code = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

const user = await db.records.create({
  label: 'USER',
  data: {
    name: 'Paul Schmitz',
    lastActive: '2024-12-02T11:38:29Z',
    verified: true,
    size: 9.5,
    favoriteTags: ['Daily Run', 'Foam']
  }
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

db = RushDB("RUSHDB_API_KEY")

user = db.records.create({
    label: "USER",
    data: {
        "name": "Paul Schmitz",
        "lastActive": "2024-12-02T11:38:29Z",
        "verified": True,
        "size": 9.5,
        "favoriteTags": ["Daily Run", "Foam"]
    }
})

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
  const [isVideoModalOpen, setIsVideoModalOpen] = useState(false)

  const openVideoModal = () => setIsVideoModalOpen(true)
  const closeVideoModal = () => setIsVideoModalOpen(false)

  return (
    <>
      <div id="hero" className="hero absolute top-0 z-0 min-h-dvh w-full md:bg-cover" />

      <section className="container z-10 mt-[-97px] grid min-h-dvh place-content-center items-center md:mt-0">
        <div className="z-0 flex min-h-dvh flex-col">
          <div className="mb-4 mt-32 grid grow grid-cols-2 items-center gap-24 md:mt-0 md:grid-cols-1 md:gap-0 md:text-center">
            <div className="mb-8 items-center md:mt-24">
              <h1 role="heading" className="typography-4xl !font-extrabold sm:text-2xl">
                Instant Database <br />
                for Modern Apps & AI<span className="hidden"> - RushDB</span>
              </h1>

              <h2 className="text-content3 text-md mb-8 mt-10 inline !font-medium !tracking-normal md:mt-0 md:text-base">
                <b>Push JSON</b>, query instantly. <br />
                <b>Zero-boilerplate</b> <b>graph database</b> that auto-normalizes your data.
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
                <MainCta variant="accent" text="Start Building Free" />
                <Button
                  as={Link}
                  href="https://docs.rushdb.com/get-started/quick-tutorial"
                  variant="outline"
                  className="bg-fill"
                >
                  5-Min Tutorial <BookIcon />
                </Button>
              </div>

              <div className="mt-16 flex justify-start md:justify-center">
                <Button
                  onClick={openVideoModal}
                  variant="outline"
                  className="bg-fill group flex h-auto items-center gap-3 !rounded-full !px-6 !py-3 shadow-sm transition-all hover:scale-105 hover:shadow-md"
                >
                  <div className="relative">
                    <Image
                      src="https://img.youtube.com/vi/wuJW9xre0xw/default.jpg"
                      alt="RushDB Video Preview"
                      width={80}
                      height={45}
                      className="rounded object-cover"
                    />
                    <div className="absolute inset-0 flex items-center justify-center">
                      <div className="flex h-8 w-8 items-center justify-center rounded-full bg-red-600 text-white shadow-lg transition-transform group-hover:scale-110">
                        <Play className="h-4 w-4 fill-current" />
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col">
                    <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                      See RushDB in Action
                    </span>
                    <span className="text-left text-xs font-medium text-red-600 dark:text-red-400">
                      Zero setup demo • 1:45
                    </span>
                  </div>
                </Button>
              </div>
            </div>

            <div className="m-auto flex min-w-[465px] items-center justify-start md:hidden md:flex-col">
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
              Focus on delivering value. RushDB handles the rest.
            </p>
            <CodeBlock
              code={language === 'typescript' ? 'pnpm add @rushdb/javascript-sdk' : 'pip install rushdb'}
              className="m-auto grid place-content-center md:w-full lg:w-fit"
              preClassName="md:w-full"
              copyButton={true}
            >
              <Image
                width={100}
                height={25}
                src={
                  language === 'typescript' ?
                    'https://img.shields.io/npm/v/@rushdb/javascript-sdk'
                  : 'https://img.shields.io/pypi/v/rushdb'
                }
                alt="package-version"
                className="md:hidden"
              />
            </CodeBlock>
          </div>
        </div>
      </section>

      {/* Video Modal */}
      {isVideoModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm">
          <div className="relative w-full max-w-4xl">
            <button
              onClick={closeVideoModal}
              className="absolute -top-12 right-0 flex h-10 w-10 items-center justify-center rounded-full bg-white/20 text-white backdrop-blur-sm transition-all hover:bg-white/30 hover:text-gray-200"
              aria-label="Close video"
            >
              <X className="h-6 w-6" />
            </button>
            <div className="relative aspect-video w-full overflow-hidden rounded-xl bg-black shadow-2xl ring-1 ring-white/20">
              <iframe
                src="https://www.youtube.com/embed/wuJW9xre0xw?autoplay=1&rel=0"
                title="RushDB Video"
                allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                allowFullScreen
                className="h-full w-full"
              />
            </div>
          </div>
        </div>
      )}
    </>
  )
}
