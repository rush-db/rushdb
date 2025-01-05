import cx from 'classnames'
import Link from 'next/link'
import { ArrowDownIcon, ArrowRightIcon, BookText, DatabaseIcon } from 'lucide-react'
import { useScroll, useTransform } from 'framer-motion'
import { useRef } from 'react'
import { motion } from 'framer-motion'

import { Button, MainCta } from '~/components/Button'
import { defaultDescription } from '~/components/Meta'
import { links } from '~/config/urls'
import { CodeBlock } from '~/components/CodeBlock'

const code = `const db = new RushDB("API_TOKEN")

// Any JSON is ok
await db.records.createMany("member", {
  email: "john.galt@example.com",
  verified: true,
  plan: {
    name: "pro",
    credits: 100,
    validTill: "2024-09-20T09:05:54"     
  }
})`

const code1 = `email: "string"
verified: "boolean"`

const code2 = `name: "string"
credits: "number"
validTill: "datetime"`

export const Hero = () => {
  const spacerRef = useRef(null)

  const { scrollYProgress } = useScroll({
    target: spacerRef,
    offset: ['start start', 'end start']
  })

  const opacity = useTransform(scrollYProgress, [0, 0.6], [1, 0])

  return (
    <>
      <div className="h-[90dvh] sm:hidden" aria-hidden ref={spacerRef} />

      <div className="flex">
        <motion.section
          className={cx(
            'container fixed inset-0 z-0 grid h-dvh place-content-center items-center',
            'sm:static sm:h-auto sm:place-content-start sm:pb-12 sm:pt-32 sm:!opacity-100 md:!grid-cols-1'
          )}
          style={{ opacity }}
        >
          <div className="z-0 mb-8 w-full items-center text-center">
            <h1 role="heading" className={cx('typography-4xl mb-4 !font-bold !tracking-tight md:text-2xl')}>
              Build{' '}
              <i>
                <span className="font-special relative text-[72px] md:text-[48px]">better</span>
              </i>{' '}
              software{' '}
              <i>
                <span className="font-special relative text-[72px] md:text-[48px]">faster</span>
              </i>
            </h1>
            <p className={cx('text-content2 text-xl !font-medium !tracking-normal md:text-lg')}>
              {defaultDescription}
            </p>
          </div>
          <div className={cx('mb-8 flex w-full items-center justify-center md:flex-col')}>
            <CodeBlock
              code={code}
              className="grid place-content-center md:!m-0 md:!mt-10 md:w-full lg:w-fit"
              preClassName="md:w-full border-2 border-content3"
            />
            <div className="text-content3 flex flex-col gap-28 p-8 md:flex-row md:p-4">
              <ArrowRightIcon className="md:hidden" />
              <ArrowRightIcon className="md:hidden" />
              <ArrowRightIcon className="md:hidden" />

              <ArrowDownIcon className="hidden md:block" />
              <ArrowDownIcon className="hidden md:block" />
              <ArrowDownIcon className="hidden md:block" />
            </div>
            <div className="flex flex-col items-center sm:flex-col sm:gap-0 md:flex-row md:gap-6">
              <div className={'border-accent rounded-xl border-2 bg-[#131313] p-4'}>
                <div className="mb-4 flex w-min items-center gap-2 rounded bg-[#E8FFB9] px-4 py-2 font-semibold text-[#131313]">
                  <DatabaseIcon /> <span>member</span>
                </div>
                <CodeBlock
                  code={code1}
                  className="md:!m-0 md:w-full lg:m-auto lg:w-fit"
                  preClassName="md:w-full !p-0"
                />
              </div>
              <svg
                width="12"
                height="60"
                viewBox="0 0 12 60"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
                className="text-accent scale-[1.3] sm:rotate-0 md:rotate-90"
              >
                <path
                  d="M6 0.226497L0.226497 6L6 11.7735L11.7735 6L6 0.226497ZM6 59.7735L11.7735 54L6 48.2265L0.226495 54L6 59.7735ZM5 6L5 54L7 54L7 6L5 6Z"
                  fill="currentColor"
                />
              </svg>

              <div className={'border-accent rounded-xl border-2 bg-[#131313] p-4'}>
                <div className="mb-4 flex w-min items-center gap-2 rounded bg-[#F7E1FF] px-4 py-2 font-semibold text-[#131313]">
                  <DatabaseIcon /> <span>plan</span>
                </div>
                <CodeBlock
                  code={code2}
                  className="md:!m-0 md:w-full lg:m-auto lg:w-fit"
                  preClassName="md:w-full !p-0"
                />
              </div>
            </div>
          </div>

          <div className="flex justify-center gap-4 sm:flex-col">
            <MainCta variant="accent" className="min-w-[170px]" text="Create Project" />

            <Button className="min-w-[170px]" as={Link} href={links.getStarted} variant="secondary">
              Documentation <BookText />
            </Button>
          </div>
        </motion.section>
      </div>
    </>
  )
}
