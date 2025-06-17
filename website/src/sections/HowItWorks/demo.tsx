import { CSSProperties, PropsWithoutRef, forwardRef, ReactNode, useState, useEffect } from 'react'

import { CodeBlock } from '~/components/CodeBlock'

import cn from 'classnames'
import { MonitorPlay } from 'lucide-react'

type DemoProps = {
  className?: string
  wrapperClassName?: string
  preClassName?: string
  style?: CSSProperties
  children?: ReactNode
  copyButton?: boolean
}

const TsLogo = ({ className }: { className: string }) => (
  <svg
    fill="none"
    height="128"
    viewBox="0 0 128 128"
    width="128"
    xmlns="http://www.w3.org/2000/svg"
    className={className}
  >
    <rect fill="#3178c6" height="128" rx="6" width="128" />
    <path
      clipRule="evenodd"
      d="m74.2622 99.468v14.026c2.2724 1.168 4.9598 2.045 8.0625 2.629 3.1027.585 6.3728.877 9.8105.877 3.3503 0 6.533-.321 9.5478-.964 3.016-.643 5.659-1.702 7.932-3.178 2.272-1.476 4.071-3.404 5.397-5.786 1.325-2.381 1.988-5.325 1.988-8.8313 0-2.5421-.379-4.7701-1.136-6.6841-.758-1.9139-1.85-3.6159-3.278-5.1062-1.427-1.4902-3.139-2.827-5.134-4.0104-1.996-1.1834-4.246-2.3011-6.752-3.353-1.8352-.7597-3.4812-1.4975-4.9378-2.2134-1.4567-.7159-2.6948-1.4464-3.7144-2.1915-1.0197-.7452-1.8063-1.5341-2.3598-2.3669-.5535-.8327-.8303-1.7751-.8303-2.827 0-.9643.2476-1.8336.7429-2.6079s1.1945-1.4391 2.0976-1.9943c.9031-.5551 2.0101-.9861 3.3211-1.2929 1.311-.3069 2.7676-.4603 4.3699-.4603 1.1658 0 2.3958.0877 3.6928.263 1.296.1753 2.6.4456 3.911.8109 1.311.3652 2.585.8254 3.824 1.3806 1.238.5552 2.381 1.198 3.43 1.9285v-13.1051c-2.127-.8182-4.45-1.4245-6.97-1.819s-5.411-.5917-8.6744-.5917c-3.3211 0-6.4674.3579-9.439 1.0738-2.9715.7159-5.5862 1.8336-7.844 3.353-2.2578 1.5195-4.0422 3.4553-5.3531 5.8075-1.311 2.3522-1.9665 5.1646-1.9665 8.4373 0 4.1785 1.2017 7.7433 3.6052 10.6945 2.4035 2.9513 6.0523 5.4496 10.9466 7.495 1.9228.7889 3.7145 1.5633 5.375 2.323 1.6606.7597 3.0954 1.5486 4.3044 2.3668s2.1628 1.7094 2.8618 2.6736c.7.9643 1.049 2.06 1.049 3.2873 0 .9062-.218 1.7462-.655 2.5202s-1.1 1.446-1.9885 2.016c-.8886.57-1.9956 1.016-3.3212 1.337-1.3255.321-2.8768.482-4.6539.482-3.0299 0-6.0305-.533-9.0021-1.6-2.9715-1.066-5.7245-2.666-8.2591-4.799zm-23.5596-34.9136h18.2974v-11.5544h-51v11.5544h18.2079v51.4456h14.4947z"
      fill="#fff"
      fillRule="evenodd"
    />
  </svg>
)
const ReactLogo = ({ className }: { className: string }) => (
  <svg xmlns="http://www.w3.org/2000/svg" viewBox="-11.5 -10.23174 23 20.46348" className={className}>
    <title>React Logo</title>
    <circle cx="0" cy="0" r="2.05" fill="#61dafb" />
    <g stroke="#61dafb" strokeWidth="1" fill="none">
      <ellipse rx="11" ry="4.2" />
      <ellipse rx="11" ry="4.2" transform="rotate(60)" />
      <ellipse rx="11" ry="4.2" transform="rotate(120)" />
    </g>
  </svg>
)

const dbCode = `import RushDB from '@rushdb/javascript-sdk'

export const db = new RushDB('RUSHDB_API_TOKEN')

// get your token at app.rushdb.com`

const apiCode = `import { keepPreviousData, useQuery } from '@tanstack/react-query'
import { db } from '@/db'
import { useSearchQuery } from '@/context/search-query-context'

// Fetch Records from RushDB
export function useRecords() {
  const { where, labels, skip, limit } = useSearchQuery()

  return useQuery({
    queryKey: ['records', where, labels, skip, limit],
    queryFn: () => db.records.find({ where, labels, skip, limit })
  })
}

// Fetch Properties from RushDB
export function useProperties() {
  const { where = {}, labels, skip, limit } = useSearchQuery()

  return useQuery({
    queryKey: ['properties', where, labels, skip, limit],
    queryFn: () => db.properties.find({ where, labels, skip, limit })
  })
}

// Fetch Record's from Relationships
export function useRecordRelations(id: string) {
  return useQuery({
    queryKey: ['record-relations', id],
    queryFn: () => db.records.relations(id)
  })
}

// Fetch Property's values from RushDB
export function usePropertyValues(propertyId: string, query?: string) {
  const { where = {}, labels } = useSearchQuery()

  return useQuery({
    queryKey: ['property-values', propertyId, where, labels, query],
    queryFn: () => db.properties.values(propertyId, { query })
  })
}

// Fetch Labels from RushDB
export function useLabels() {
  const { where, skip, limit } = useSearchQuery()

  return useQuery({
    queryKey: ['labels', where, skip, limit],
    queryFn: () => db.labels.find({ where, skip, limit })
  })
}
`

const recordsCode = `
import { useState } from 'react'
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { RecordModal } from '@/components/record-modal'
import { db } from '@/db'
import { useRecords } from '@/hooks/use-records'
import { Loader } from 'lucide-react'
import { DBRecord } from '@rushdb/javascript-sdk'

export default function RecordsGrid() {
  const [currentRecord, setCurrentRecord] = useState<DBRecord | undefined>()

  const { data: records, isLoading, isFetching } = useRecords()

  if (isLoading || isFetching) {
    return (
      <div className="flex-1 p-6 pl-80">
        <div className="pl-4 items-center grid w-full h-full animate-pulse text-center justify-center">
          <Loader className="animate-spin" />
        </div>
      </div>
    )
  }

  return (
    <div className="flex-1 p-6 pl-80">
      <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 pl-4">
        {records?.data?.map((record) => (
          <Card key={record.__id} className="flex flex-col shadow-none">
            <CardHeader>
              <CardTitle>{record.__label}</CardTitle>
              <CardDescription>{record.__id}</CardDescription>
            </CardHeader>
            <CardContent className="flex-grow">
              <p className="">{db.toInstance(record).date.toISOString()}</p>
            </CardContent>
            <CardFooter>
              <Button
                onClick={() => setCurrentRecord(record)}
                className="w-full"
                variant="outline"
              >
                Details
              </Button>
            </CardFooter>
          </Card>
        ))}
      </div>
      {currentRecord && (
        <RecordModal
          record={currentRecord}
          isOpen={!!currentRecord}
          onClose={() => setCurrentRecord(undefined)}
        />
      )}
    </div>
  )
}`

const code = {
  'db.ts': dbCode,
  'api.ts': apiCode,
  'Records.tsx': recordsCode
}

const tabs = {
  'Live Preview': MonitorPlay,
  'db.ts': TsLogo,
  'api.ts': TsLogo,
  'Records.tsx': ReactLogo
} as const

const Logo = ({ tab, className }: { tab: string; className: string }) => {
  // @ts-ignore
  const Comp = tabs[tab]

  return <Comp className={className} />
}

export const Demo = forwardRef<HTMLDivElement, PropsWithoutRef<DemoProps>>(
  ({ className, preClassName, wrapperClassName, children: extra, copyButton, style }, ref) => {
    const [tab, setTab] = useState<keyof typeof tabs>('Live Preview')

    const [isClient, setIsClient] = useState(false)

    useEffect(() => {
      // Ensures this runs only in the browser
      setIsClient(true)
    }, [])

    return (
      <div
        className={cn(className, 'w-full rounded-xl bg-[#131313] dark:bg-[#0a0a0a]')}
        ref={ref}
        style={style}
      >
        <div className="text-content-contrast border-b-stroke mb-2 flex w-full cursor-pointer justify-between rounded-t-xl border-b bg-[#131313] text-center font-mono font-bold dark:border-b-[#404040] dark:bg-[#0a0a0a]">
          {Object.keys(tabs).map((key) => (
            <div
              key={key}
              onClick={() => setTab(key as keyof typeof tabs)}
              className={cn(
                'w-full grow py-2 transition hover:bg-gray-800 hover:grayscale-0 dark:hover:bg-gray-700',
                {
                  grayscale: key !== tab,
                  'border-accent border-b-2 text-white dark:text-white': key === tab,
                  'text-gray-400 dark:text-gray-300': key !== tab
                }
              )}
            >
              {key} <Logo className="h-8 w-8 p-2" tab={key} />
            </div>
          ))}
        </div>
        {isClient ?
          tab === 'Live Preview' ?
            <iframe
              src={'https://main.d2d2gymukglcvg.amplifyapp.com/'}
              className="h-[85vh] w-full"
              title="RushDB Demo App"
            />
          : <CodeBlock
              code={code[tab]}
              className={className}
              preClassName={preClassName}
              wrapperClassName={cn(wrapperClassName, 'rounded-xl rounded-t-none ')}
              copyButton={copyButton}
              showLineNumbers={true}
            />

        : null}
      </div>
    )
  }
)

Demo.displayName = 'Demo'
