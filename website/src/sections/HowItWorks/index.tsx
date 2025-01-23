import cx from 'classnames'
import { Button, MainCta } from '~/components/Button'
import Link from 'next/link'
import { links } from '~/config/urls'
import { Waypoints } from 'lucide-react'
import { CodeBlock } from '~/components/CodeBlock'
import Image from 'next/image'

import dashboard from '../../images/dashboard.png'
const code1 = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('api_token')

await db.records.createMany("COMPANY", {
  name: 'Google LLC',
  address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
  foundedAt: '1998-09-04T00:00:00.000Z',
  rating: 4.9,
  DEPARTMENT: [{
    name: 'Research & Development',
    description: 'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
    PROJECT: [{
      name: 'Bard AI',
      description: 'A state-of-the-art generative AI model for natural language understanding and creation.',
      active: true,
      budget: 1200000000,
      EMPLOYEE: [{
        name: 'Jeff Dean',
        position: 'Head of AI Research',
        email: 'jeff@google.com',
        dob: '1968-07-20T00:00:00.000Z',
        salary: 3000000`

const code2 = `await db.records.find({
  labels: ['COMPANY'],
  where: {
    stage: 'seed',
    address: { $contains: 'USA' },
    foundedAt: {
      $year: { $lte: 2000 }
    },
    rating: {
      $or: [{ $lt: 2.5 }, { $gte: 4.5 }]
    },
    EMPLOYEE: {
      $alias: '$employee',
      salary: {
        $gte: 500_000
      }
    }
  },
  aggregate: {
    employees: {
      fn: 'collect',
      alias: '$employee',
      limit: 10
    }
  }
})`

const code3 = `// String Property
await db.properties.values(
  "0192397b-8579-7ce2-a899-01c59bad63f8"
)
// Response
{
  "values": [
    "Alice Johnson",
    "Best Shoe Store",
    "John Doe"
  ],
  "name": "name",
  "type": "string",
}

// Number Property
await db.properties.values(
  "019412c0-2051-71fe-bc9d-26117b52c119"
)
// Response
{
  min: 5.5,
  max: 12.5,
  values: [5.5, 6, 6.5, 7, 7.5, 8, 8.5, ...],
  type: 'number'
}
`

const codeDocker = `docker run -p 3000:3000 --name rushdb \\
-e NEO4J_URL='neo4j+s://1234567.databases.neo4j.io' \\
-e NEO4J_USERNAME='neo4j' \\
-e NEO4J_PASSWORD='password' \\
rushdb/platform`

const codeCompany = `Record
---------------------
address:     "string"
foundedAt: "datetime"
rating:      "number"`

const codeDepartment = `Record
-----------------------
name:          "string"
description: "datetime"`

const codeProject = `Record
---------------------
name:        "string"
description: "string"
active:     "boolean"
budget:      "number"`

const codeEmployee = `Record
------------------
name:     "string"
position: "string"
email:    "string"
dob:    "datetime"
salary:   "number"`

const codeProperty = `Property
-------------------
name: "description"
type:      "string"`

export const HowItWorks = () => {
  return (
    <>
      <section className={cx('mt-[1px]')}>
        <div className="container">
          <div className="outline-stroke rounded-tl-[150px] text-center outline outline-1 outline-offset-0 md:rounded-tl-[80px]">
            <h3 className={cx('typography-2xl md:typography-xl text pt-20')}>Push any JSON or CSV data</h3>
            <p className={cx('text-content3 text-md pb-20 pt-8 !font-medium !tracking-normal md:text-base')}>
              RushDB intelligently maps relationships, types,
              <br /> and labels any input data, so you don’t have to.
            </p>
            <CodeBlock
              code={code1}
              className="h-2/3 place-content-center pb-0 md:w-full lg:w-full"
              wrapperClassName="rounded-bl-none rounded-br-none pb-0"
              preClassName="md:w-full pb-0"
            >
              <div className="absolute bottom-0 left-0 h-1/2 w-full bg-gradient-to-b from-transparent to-[#131313]"></div>
            </CodeBlock>
          </div>
        </div>
      </section>

      <section className={cx('outline-stroke outline outline-1 outline-offset-0')}>
        <div className="container grid w-full grid-flow-col grid-rows-2 gap-[1px] md:grid-flow-row md:grid-rows-3">
          <div className="outline-stroke flex w-full items-center gap-4 rounded-b-[50px] p-8 outline outline-1 outline-offset-0 md:col-span-2 md:p-4">
            <span className="text-content font-mono text-xl font-bold md:text-lg">~2ms</span>{' '}
            <p className="text-md text-content3 md:text-base">Batch write speed per Record</p>
          </div>
          <div className="outline-stroke w-full rounded-l-full p-2 outline outline-1 outline-offset-0 md:hidden"></div>
          <div className="outline-stroke col-span-2 row-span-2 w-full rounded-tr-[50px] p-8 outline outline-1 outline-offset-0 md:p-4">
            <div className="flex w-full items-start gap-4">
              <span className="text-content font-mono text-xl font-bold md:text-lg">32MB</span>
              <div>
                <p className="text-md text-content3 md:text-base">In a single payload or...</p>
                <p className="text-md text-content3 md:text-base">
                  <b>10 000+</b> products for online store
                </p>
                <p className="text-md text-content3 md:text-base">
                  <b>100 000+</b> financial transactions
                </p>
                <p className="text-md text-content3 md:text-base">
                  <b>1 000 000+</b> API logs
                </p>
              </div>
            </div>
          </div>
          <div className="outline-stroke row-span-2 w-full rounded-full rounded-br-none p-8 outline outline-1 outline-offset-0 md:hidden md:p-4"></div>
        </div>
      </section>

      <section className={cx('border-b')}>
        <div className="container text-center">
          <h3 className={cx('typography-2xl md:typography-xl text pt-20')}>Automatic Data Normalization</h3>
          <p className={cx('text-content3 text-md pb-20 pt-8 !font-medium !tracking-normal md:text-base')}>
            Records are created with appropriate types and relationships between them, <br /> without any need
            for predefined models or schemas.
          </p>
          <div className="m-auto grid w-full max-w-3xl grid-flow-col grid-rows-2 gap-[1px] md:grid-rows-4">
            <div className="outline-stroke w-full rounded-bl-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3">
                  <div className="bg-accent-yellow h-6 w-6 rounded-full"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-sm">COMPANY</span>
                </div>
                <CodeBlock code={codeCompany} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
            <div className="outline-stroke w-full rounded-br-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3">
                  <div className="h-6 w-6 rounded-full bg-green-600"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-sm">PROJECT</span>
                </div>
                <CodeBlock code={codeProject} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
            <div className="outline-stroke w-full rounded-r-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3">
                  <div className="bg-accent-orange h-6 w-6 rounded-full"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-sm">DEPARTMENT</span>
                </div>
                <CodeBlock code={codeDepartment} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
            <div className="outline-stroke w-full rounded-bl-[80px] rounded-tr-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3">
                  <div className="bg-accent-purple h-6 w-6 rounded-full"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-sm">EMPLOYEE</span>
                </div>
                <CodeBlock code={codeEmployee} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className={cx('')}>
        <div className="container text-center">
          <div className="outline-stroke rounded-t-[80px] py-20 outline outline-1 outline-offset-0 md:pb-8">
            <h3 className={cx('typography-2xl md:typography-xl text')}>Query Smarter, Not Harder</h3>
            <p className={cx('text-content3 text-md pb-20 pt-8 !font-medium !tracking-normal md:text-base')}>
              Every Property gets its own "container," smartly linked to other matching Records
              <br /> by name and type, making querying easy and performant.
            </p>

            <div className="flex w-full flex-row items-center justify-center gap-4 md:gap-1">
              <CodeBlock
                code={codeProperty}
                className="place-content-center gap-[1px]"
                preClassName="md:w-full"
              />
              <Waypoints />
              <div className="flex flex-col gap-4 md:gap-2">
                <div className="flex w-fit items-center gap-4 rounded-full border px-3 py-3">
                  <div className="h-6 w-6 rounded-full bg-green-600"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-sm">PROJECT</span>
                </div>
                <div className="flex items-center gap-4 rounded-full border px-3 py-3">
                  <div className="bg-accent-orange h-6 w-6 rounded-full"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-sm">DEPARTMENT</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className={cx('border-b border-t')}>
        <div className="container m-auto grid grid-cols-2 gap-[1px] md:grid-cols-1">
          <div className="outline-stroke flex h-full w-full flex-col justify-between rounded-bl-[80px] rounded-tr-[80px] pt-12 outline outline-1 outline-offset-0">
            <div className="mx-auto mb-8 max-w-xl md:px-6">
              <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Complex Queries, Simple Syntax</h4>
              <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-base')}>
                Find exactly what you need with ease. With automated on-the-fly data normalization, query
                complex, deeply interconnected data without the acrobatics.
              </p>
            </div>
            <CodeBlock
              code={code2}
              className="mx-auto w-full max-w-xl"
              preClassName="md:w-full"
              wrapperClassName="rounded-bl-none rounded-br-none pb-0"
            ></CodeBlock>
          </div>

          <div className="outline-stroke flex h-full w-full flex-col justify-between rounded-br-[80px] pt-12 outline outline-1 outline-offset-0">
            <div className="mx-auto mb-8 max-w-xl md:px-6">
              <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Build Powerful Filters & Search</h4>
              <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-base')}>
                List every variation. Filter with ease. Build catalogs and search experiences like a pro - no
                backend, no fuss. All from a single API.
              </p>
            </div>
            <CodeBlock
              code={code3}
              className="mx-auto w-full max-w-xl"
              preClassName="md:w-full"
              wrapperClassName="rounded-bl-none rounded-br-none pb-0"
            ></CodeBlock>
          </div>
        </div>
      </section>

      <section className={cx('outline-stroke outline outline-1 outline-offset-0')} id="use-cases">
        <div className="container">
          <div className="outline-stroke outline outline-1 outline-offset-0">
            <h3 className={cx('typography-2xl md:typography-xl text py-16 text-center')}>Use Cases</h3>

            <div className="m-auto grid w-full grid-flow-col grid-rows-2 gap-[1px] md:grid-rows-4">
              <div className="outline-stroke w-full rounded-b-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">SaaS & Apps</h4>
                <p className="text-content3 text-md md:text-base">
                  Building the next big thing shouldn't start with battling clumsy databases and other stuff.
                  Focus on delivering features - RushDB takes care of the rest.
                </p>
              </div>
              <div className="outline-stroke w-full rounded-t-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Hobby Projects</h4>
                <p className="text-content3 text-md md:text-base">
                  What’s more frustrating than losing your spark of inspiration to infrastructure hassle?
                  RushDB lets you skip the grind and get back to building.
                </p>
              </div>
              <div className="outline-stroke w-full rounded-r-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">AI / ML & Research</h4>
                <p className="text-content3 text-md md:text-base">
                  Having fast and reliable data storage for AI is challenging yet rewarding. RushDB helps you
                  store structured and semi-structured data at scale.
                </p>
              </div>

              <div className="outline-stroke w-full rounded-l-[80px] rounded-br-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Search Engines</h4>
                <p className="text-content3 text-md md:text-base">
                  RushDB makes filtering large datasets fast and efficient, handling data of any shape and
                  complexity with search capabilities designed for performance.
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className={cx('mt-[1px] border-b')}>
        <div className="container text-center">
          <div className={cx('outline-stroke outline outline-1 outline-offset-0')}>
            <h3 className={cx('typography-2xl md:typography-xl text pt-20')}>Fully Featured Dashboard</h3>
            <p
              className={cx(
                'text-content3 text-md pb-20 pt-8 !font-medium !tracking-normal md:pb-8 md:text-base'
              )}
            >
              That helps you navigate your data intuitively and fast
            </p>
            <Image src={dashboard.src} alt="dashboard-preview" width={1894} height={989} className="w-full" />
          </div>
        </div>
      </section>

      <section className={cx('border-b')}>
        <div className="container text-center">
          <div className="outline-stroke rounded-b-full py-20 outline outline-1 outline-offset-0">
            <h3 className={cx('typography-2xl md:typography-xl text mb-8')}>Self-hosted? Simple.</h3>
            <p className={cx('text-content3 text-md mb-8 !font-medium !tracking-normal md:text-base')}>
              Just run Docker container with Neo4j credentials
            </p>
            <CodeBlock
              language="bash"
              code={codeDocker}
              className="mb-8 grid place-content-center gap-[1px] md:w-full lg:w-full"
              preClassName="md:w-full"
            />
            <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-base')}>
              That's it. RushDB is ready at <span className="bold font-mono font-bold">localhost:3000</span>{' '}
              🎉️
            </p>
          </div>
        </div>
      </section>

      <section className={cx('outline-stroke outline outline-1 outline-offset-0')}>
        <div className="container text-center">
          <div className="outline-stroke rounded-full py-20 outline outline-1 outline-offset-0">
            <h3 className={cx('typography-2xl md:typography-xl text mb-8')}>
              Not an infra fan? Opt for <span className="text-accent">RushDB Cloud</span>
            </h3>
            <p className={cx('text-content3 text-md mb-8 !font-medium !tracking-normal md:text-base')}>
              2 Projects Free Forever. No Maintenance Required.
              <br className="md:hidden" />
              Focus on building apps, not on managing infrastructure.
            </p>
            <div className="m-auto mb-8 flex w-full justify-center gap-4">
              <MainCta variant="accent" text="Create Project" />
              <Button as={Link} href={links.pricing} variant="outline" className="bg-fill">
                Explore Plans
              </Button>
            </div>
          </div>
        </div>
      </section>
    </>
  )
}
