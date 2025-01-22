import cx from 'classnames'
import { Button, MainCta } from '~/components/Button'
import Link from 'next/link'
import { links } from '~/config/urls'
import { BookIcon } from 'lucide-react'
import { CodeBlock } from '~/components/CodeBlock'
import Image from 'next/image'

import d1 from '../../images/dashboard-table.png'
import d2 from '../../images/dashboard-graph.png'
const code1 = `const db = new RushDB('api_token')

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
        dob: '1968-07-16T00:00:00.000Z',
        salary: 3000000,
      }]
    }]
  }]
})`

const codeDocker = `docker run -p 3000:3000 --name rushdb \\
-e NEO4J_URL='neo4j+s://1234567.databases.neo4j.io' \\
-e NEO4J_USERNAME='neo4j' \\
-e NEO4J_PASSWORD='password' \\
rushdb/platform`

export const HowItWorks = () => {
  return (
    <>
      <section className={cx('z-10 grid w-full place-content-center items-center')}>
        <div className="border text-center">
          <h3 className={cx('typography-3xl text sm:text-2xl')}>Push any JSON or CSV data</h3>
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            RushDB intelligently maps relationships, types,
            <br /> and labels any input data, so you don’t have to.
          </p>
          <CodeBlock
            code={code1}
            className="grid place-content-center md:w-full lg:w-full"
            preClassName="md:w-full"
          />
        </div>
      </section>
      {/**/}
      <section className={cx('border')}>
        <div className="container grid w-full grid-flow-col grid-rows-2">
          <div className="flex w-full items-center gap-4 border p-8">
            <span className="text-content font-mono text-xl font-bold">~2ms</span>{' '}
            <p className="text-md text-content3">Batch write speed per Record</p>
          </div>
          <div className="w-full border p-2"></div>
          <div className="col-span-2 row-span-2 w-full border p-8">
            <div className="flex w-full items-start gap-4">
              <span className="text-content font-mono text-xl font-bold">32MB</span>
              <div>
                <p className="text-md text-content3">In a single payload or...</p>

                <p className="text-md text-content3">10,000+ products for online store</p>
                <p className="text-md text-content3">100,000+ financial transactions</p>
                <p className="text-md text-content3">1,000,000+ API logs</p>
              </div>
            </div>
          </div>
        </div>
      </section>
      {/**/}
      <section className={cx('')}>
        <div className="container text-center">
          <h3 className={cx('typography-3xl text sm:text-2xl')}>Automatic Data Normalization</h3>
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            Records are created with appropriate types and relationships between them, <br /> without any need
            for predefined models or schemas.
          </p>
          <div className="m-auto grid w-full max-w-4xl grid-flow-col grid-rows-2">
            <div className="w-full border p-12">01</div>
            <div className="w-full border p-12">02</div>
            <div className="w-full border p-12">03</div>
            <div className="w-full border p-12">04</div>
          </div>
        </div>
      </section>

      <section className={cx('border')}>
        <div className="container text-center">
          <h3 className={cx('typography-3xl text sm:text-2xl')}>Query Smarter, Not Harder</h3>
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            Every Property gets its own "container," smartly linked to other matching Records by name and
            type, making querying easy and performant.
          </p>
        </div>
      </section>

      <section className={cx('border')}>
        <div className="container m-auto grid grid-cols-2">
          <div className="w-full border p-12">
            <h3 className={cx('typography-2xl text sm:text-2xl')}>Complex Queries, Simple Syntax</h3>
            <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
              Find exactly what you need with ease. With automated on-the-fly data normalization, query
              complex, deeply interconnected data without the acrobatics.
            </p>

            <CodeBlock
              code={code1}
              className="grid place-content-center md:w-full lg:w-full"
              preClassName="md:w-full"
            />
          </div>

          <div className="w-full border p-12">
            <h3 className={cx('typography-2xl text sm:text-2xl')}>Build Powerful Filters & Search</h3>
            <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
              List every variation. Filter with ease. Build catalogs and search experiences like a pro - no
              backend, no fuss. All from a single API.
            </p>

            <CodeBlock
              code={code1}
              className="grid place-content-center md:w-full lg:w-full"
              preClassName="md:w-full"
            />
          </div>
        </div>
      </section>

      <section className={cx('border')}>
        <div className="container text-center">
          <h3 className={cx('typography-3xl text sm:text-2xl')}>Use Cases</h3>

          <div className="m-auto grid w-full grid-flow-col grid-rows-2">
            <div className="w-full border p-12">
              <h4 className="bold text-xl">SaaS & Apps</h4>
              <p className="text-content3 text-base">
                Building the next big thing shouldn't start with battling clumsy databases and other stuff.
                Focus on delivering features - RushDB takes care of the rest.
              </p>
            </div>
            <div className="w-full border p-12">
              <h4 className="bold text-xl">AI / ML & Research</h4>
              <p className="text-content3 text-base">
                Having fast and reliable data storage for AI is challenging yet rewarding. RushDB helps you
                store structured and semi-structured data at scale.
              </p>
            </div>
            <div className="w-full border p-12">
              <h4 className="bold text-xl">Hobby Projects</h4>
              <p className="text-content3 text-base">
                What’s more frustrating than losing your spark of inspiration to infrastructure hassle? RushDB
                lets you skip the grind and get back to building.
              </p>
            </div>
            <div className="w-full border p-12">
              <h4 className="bold text-xl">Search Engines</h4>
              <p className="text-content3 text-base">
                RushDB makes filtering large datasets fast and efficient, handling data of any shape and
                complexity with search capabilities designed for performance.
              </p>
            </div>
          </div>
        </div>
      </section>

      <section className={cx('border')}>
        <div className="container text-center">
          <h3 className={cx('typography-2xl text sm:text-2xl')}>Fully Featured Dashboard</h3>
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            That helps you navigate your data intuitively and fast
          </p>
        </div>
        <Image src={d1.src} alt="dashboard-preview-1" width={1544} height={527} />
        <Image src={d2.src} alt="dashboard-preview-2" width={1544} height={527} />
      </section>

      <section className={cx('border')}>
        <div className="container text-center">
          <h3 className={cx('typography-3xl text sm:text-2xl')}>Self-hosted? Simple.</h3>
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            Just run Docker container with Neo4j credentials
          </p>
          <CodeBlock
            language="bash"
            code={codeDocker}
            className="grid place-content-center md:w-full lg:w-full"
            preClassName="md:w-full"
          />
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            That's it. RushDB is ready at <span className="bold font-mono font-bold">localhost:3000</span> 🎉️
          </p>
        </div>
      </section>

      <section className={cx('border')}>
        <div className="container text-center">
          <h3 className={cx('typography-3xl text sm:text-2xl')}>Not an infra fan? Opt for RushDB Cloud</h3>
          <p className={cx('text-content3 text-md !font-medium !tracking-normal md:text-lg')}>
            2 Projects Free Forever. No Maintenance Required.
            <br />
            Focus on building apps, not on managing infrastructure.
          </p>
        </div>
      </section>
    </>
  )
}
