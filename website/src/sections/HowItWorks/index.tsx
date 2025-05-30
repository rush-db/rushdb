import { Button, MainCta } from '~/components/Button'
import Link from 'next/link'
import { links } from '~/config/urls'
import { BookIcon, Waypoints } from 'lucide-react'
import { CodeBlock } from '~/components/CodeBlock'
import Image from 'next/image'

import dashboard from '../../images/dashboard.png'
import { CodeBlockWithLanguageSelector } from '~/components/CodeBlockWithLanguageSelector'
import { useContext } from 'react'
import { CodingLanguage } from '~/contexts/CodingLanguage'
import { Demo } from '~/sections/HowItWorks/demo'
import { GitHub } from '~/components/Icons/GitHub'

const code1 = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('rushdb-api-key')

await db.records.createMany({
  label: "COMPANY",
  data: {
    name: 'Google LLC',
    address: '1600 Amphitheatre Parkway, Mountain View, CA 94043, USA',
    foundedAt: '1998-09-04T00:00:00.000Z',
    rating: 4.9,
    DEPARTMENT: [{
      name: 'Research & Development',
      description: 'Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.',
      tags: ['AI', 'Cloud Computing', 'Research'],
      profitable: true,
      PROJECT: [{
        name: 'Bard AI',
        description: 'A state-of-the-art generative AI model for natural language understanding and creation.',
        active: true,
        budget: 1200000000,
        EMPLOYEE: [{
          name: 'Jeff Dean',
          position: 'Head of AI Research',
          email: 'jeff@google.com',
          salary: 3000000`

const code1Py = `from rushdb import RushDB

db = RushDB("rushdb-api-key")

db.records.create_many(
    "COMPANY",
    {
        "name": "Google LLC",
        "address": "1600 Amphitheatre Parkway, Mountain View, CA 94043, USA",
        "foundedAt": "1998-09-04T00:00:00.000Z",
        "rating": 4.9,
        "DEPARTMENT": [
            {
                "name": "Research & Development",
                "description": "Innovating and creating advanced technologies for AI, cloud computing, and consumer devices.",
                "tags": ["AI", "Cloud Computing", "Research"],
                "profitable": true,
                "PROJECT": [
                    {
                        "name": "Bard AI",
                        "description": "A state-of-the-art generative AI model for natural language understanding and creation.",
                        "active": true,
                        "budget": 1200000000,
                        "EMPLOYEE": [
                            {
                                "name": "Jeff Dean",
                                "position": "Head of AI Research",
                                "email": "jeff@google.com",
                                "salary": 3000000"`

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

const code2Py = `db.records.find(
    {
        "labels": ["COMPANY"],
        "where": {
            "stage": "seed",
            "address": {"$contains": "USA"},
            "foundedAt": {"$lte": {"$year": 2000}},
            "rating": {
                "$or": [{"$lt": 2.5}, {"$gte": 4.5}]
            },
            "EMPLOYEE": {
                "$alias": "$employee",
                "salary": {
                    "$gte": 500_000
                }
            },
        },
        "aggregate": {
            "employees": {
                "fn": "collect",
                "alias": "$employee",
                "limit": 10
            }
        },
    }
)`

const code3 = `// Property \`name\` [string]
await db.properties.values(
  '0192397b-8579-7ce2-a899-01c59bad63f8'
)
// Response
{
  values: [
    'Eleanor Whitaker',
    'Marcus Donovan',
    'Priya Kapoor',
    'Julian Alvarez'
  ],
  type: 'string'
}

// Property \`size\` [number]
await db.properties.values(
  '019412c0-2051-71fe-bc9d-26117b52c119'
)
// Response
{
  min: 5.5,
  max: 12.5,
  values: [5.5, 6, 6.5, 7, 7.5, 8, 8.5, ...],
  type: 'number'
}
`

const code3Py = `# Property \`name\` [string]
db.properties.values(
  "0192397b-8579-7ce2-a899-01c59bad63f8"
)
# Response
{
  "values": [
    "Eleanor Whitaker",
    "Marcus Donovan",
    "Priya Kapoor",
    "Julian Alvarez"
  ],
  "type": "string"
}

# Property \`size\` [number]
db.properties.values(
  "019412c0-2051-71fe-bc9d-26117b52c119"
)
# Response
{
  "min": 5.5,
  "max": 12.5,
  "values": [5.5, 6, 6.5, 7, 7.5, 8, 8.5, ...],
  "type": "number"
}
`

const codeDocker = `docker run -p 3000:3000 --name rushdb \\
-e NEO4J_URL='bolt+s://your-neo4j-instance-url:7687' \\
-e NEO4J_USERNAME='neo4j' \\
-e NEO4J_PASSWORD='password' \\
rushdb/platform`

const codeCompany = `Record
---------------------
name:        "string"
address:     "string"
foundedAt: "datetime"
rating:      "number"`

const codeDepartment = `Record
---------------------
name:        "string"
description: "string"
tags:        "string"
profitable: "boolean"`

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
salary:   "number"`

const codeProperty = `Property
-------------------
name: "description"
type:      "string"`

const codeAiIntegration = `import OpenAI from 'openai'
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('rushdb-api-key')
const openai = new OpenAI({ apiKey: 'openai-api-key' })

async function generateAndStoreData() {
  // Step 1: Call OpenAI API to generate some output
  const prompt = '...'
  const completion = await openai.chat.completions.create({
    model: 'gpt-3.5-turbo',
    messages: [{ role: 'user', content: prompt }],
    response_format: { type: 'json_object' }
  })

  // Step 2: Extract the generated content
  const generatedContent = completion.choices[0].message.content
  const parsedContent = JSON.parse(generatedContent)

  // Step 3: Store the output in RushDB
  const record = await db.records.createMany({
    label: 'AI_RESPONSE',
    data: parsedContent
  })
}`

const codeAiIntegrationPy = `import openai
from rushdb import RushDB

db = RushDB("rushdb-api-key")
openai.api_key = "openai-api-key"


def generate_and_store_data():
    # Step 1: Call OpenAI API to generate some output
    prompt = "..."
    completion = await openai.ChatCompletion.create(
        model="gpt-3.5-turbo",
        messages=[{"role": "user", "content": prompt}],
        response_format={"type": "json_object"},
    )

    # Step 2: Extract the generated content
    generated_content = completion["choices"][0]["message"]["content"]
    parsed_content = json.loads(generated_content)

    # Step 3: Store the output in RushDB
    record = await db.records.create_many("AI_RESPONSE", parsed_content)`

export const HowItWorks = () => {
  const { language } = useContext(CodingLanguage)

  return (
    <>
      <section className="mt-[1px]">
        <div className="container">
          <div className="outline-stroke rounded-tl-[150px] text-center outline outline-1 outline-offset-0 md:rounded-tl-[80px]">
            <h3 className="typography-2xl md:typography-xl text px-6 pt-20">
              RushDB turns any JSON or CSV into graph
            </h3>
            <p className="text-content3 text-md px-6 pb-20 pt-8 !tracking-normal md:text-base">
              RushDB, a high-performance <b>graph database</b>, built on top of <b>Neo4j</b>,{' '}
              <br className="md:hidden" /> intelligently maps relationships, types, and labels any input data,
              so you don’t have to.
            </p>
            <CodeBlock
              code={language === 'typescript' ? code1 : code1Py}
              className="h-2/3 place-content-center pb-0 md:w-full lg:w-full"
              wrapperClassName="rounded-bl-none rounded-br-none pb-0"
              preClassName="md:w-full pb-0"
            >
              <div className="absolute bottom-0 left-0 h-1/2 w-full bg-gradient-to-b from-transparent to-[#131313]"></div>
            </CodeBlock>
          </div>
        </div>
      </section>

      <section className="outline-stroke outline outline-1 outline-offset-0">
        <div className="container grid w-full grid-flow-col grid-rows-2 gap-[1px] md:grid-flow-row md:grid-rows-3">
          <div className="outline-stroke row-span-2 w-full rounded-full rounded-tr-none p-8 outline outline-1 outline-offset-0 md:hidden md:p-6"></div>

          <div className="outline-stroke col-span-2 row-span-2 w-full rounded-b-[50px] p-8 outline outline-1 outline-offset-0 md:p-6">
            <div className="flex w-full flex-col items-start gap-4 md:gap-2">
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
          <div className="outline-stroke flex w-full flex-col gap-4 rounded-tr-[50px] p-8 outline outline-1 outline-offset-0 md:col-span-2 md:gap-2 md:p-6">
            <span className="text-content font-mono text-xl font-bold md:text-lg">~0.25ms</span>{' '}
            <p className="text-md text-content3 md:text-base">Batch write speed per Record</p>
          </div>
          <div className="outline-stroke flex w-full flex-col gap-4 rounded-l-full p-8 outline outline-1 outline-offset-0 md:gap-2">
            <span className="text-content font-mono text-xl font-bold md:text-lg">ACID Compliance</span>{' '}
            <p className="text-md text-content3 md:text-base">Ensures data integrity and reliability</p>
          </div>

          <div className="outline-stroke row-span-2 w-full rounded-full rounded-br-none p-8 outline outline-1 outline-offset-0 md:hidden"></div>
        </div>
      </section>

      <section className="border-b">
        <div className="container text-center">
          <h3 className="typography-2xl md:typography-xl text px-6 pt-20">Automatic Data Normalization</h3>
          <p className="text-content3 text-md px-6 pb-20 pt-8 !tracking-normal md:text-base">
            Built for <b>SaaS development</b>, <b>machine learning</b>, <b>AI-driven apps</b>,{' '}
            <b>vector search</b>, <b>semi-structured</b> and <b>structured data</b> needs.{' '}
            <br className="md:hidden" />
            RushDB creates each <b>Record</b> with the proper relationships and types, so you don’t need
            predefined schemas — ideal for fast-moving teams.
          </p>
          <div className="m-auto grid w-full max-w-3xl grid-flow-col grid-rows-2 gap-[1px] md:grid-rows-4">
            <div className="outline-stroke w-full rounded-bl-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3 md:gap-2 md:p-2">
                  <div className="bg-accent-yellow h-6 w-6 rounded-full md:h-4 md:w-4"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-xs">COMPANY</span>
                </div>
                <CodeBlock code={codeCompany} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
            <div className="outline-stroke w-full rounded-br-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3 md:gap-2 md:p-2">
                  <div className="h-6 w-6 rounded-full bg-green-600 md:h-4 md:w-4"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-xs">PROJECT</span>
                </div>
                <CodeBlock code={codeProject} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
            <div className="outline-stroke w-full rounded-r-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3 md:gap-2 md:p-2">
                  <div className="bg-accent-orange h-6 w-6 rounded-full md:h-4 md:w-4"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-xs">DEPARTMENT</span>
                </div>
                <CodeBlock code={codeDepartment} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
            <div className="outline-stroke w-full rounded-bl-[80px] rounded-tr-[80px] p-12 outline outline-1 outline-offset-0 md:p-6">
              <div className="flex w-full flex-col items-start gap-4">
                <div className="m-auto flex w-fit flex-wrap items-center gap-4 rounded-full border px-3 py-3 md:gap-2 md:p-2">
                  <div className="bg-accent-purple h-6 w-6 rounded-full md:h-4 md:w-4"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-xs">EMPLOYEE</span>
                </div>
                <CodeBlock code={codeEmployee} className="m-auto" preClassName="md:w-full" />
              </div>
            </div>
          </div>
        </div>
      </section>

      <section>
        <div className="container text-center">
          <div className="outline-stroke rounded-t-[80px] py-20 outline outline-1 outline-offset-0 md:pb-8">
            <h3 className="typography-2xl md:typography-xl text px-6">Query Smarter, Not Harder</h3>
            <p className="text-content3 text-md px-6 pb-20 pt-8 !tracking-normal md:text-base">
              Built for <b>interconnected data</b> and <b>high-speed queries</b>. Each Property lives in its
              own container, linked by name and type to other <b>Records</b> — <br className="md:hidden" />{' '}
              making retrieval fast and precise. Designed for <b>rapid development</b>, <b>AI workflows</b>,
              and <b>data-intensive</b> use cases.
            </p>

            <div className="flex w-full flex-row items-center justify-center gap-4 md:gap-1">
              <CodeBlock
                code={codeProperty}
                className="place-content-center gap-[1px]"
                preClassName="md:w-full"
              />
              <Waypoints />
              <div className="flex flex-col gap-4 md:gap-2">
                <div className="flex w-fit items-center gap-4 rounded-full border px-3 py-3 md:gap-2 md:p-2">
                  <div className="h-6 w-6 rounded-full bg-green-600 md:h-4 md:w-4"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-xs">PROJECT</span>
                </div>
                <div className="flex items-center gap-4 rounded-full border px-3 py-3 md:gap-2 md:p-2">
                  <div className="bg-accent-orange h-6 w-6 rounded-full md:h-4 md:w-4"></div>
                  <span className="text-content text-md mr-2 font-mono font-bold md:text-xs">DEPARTMENT</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="border-b border-t">
        <div className="container m-auto grid grid-cols-2 gap-[1px] md:grid-cols-1">
          <div className="outline-stroke flex h-full w-full flex-col justify-between rounded-bl-[80px] rounded-tr-[80px] pt-12 outline outline-1 outline-offset-0">
            <div className="mx-auto mb-8 max-w-xl md:px-6 md:text-center">
              <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Complex Queries, Simple Syntax</h4>
              <p className="text-content3 text-md !tracking-normal md:text-base">
                Query complex, deeply <b>connected data</b> without friction. RushDB’s <b>graph database</b>{' '}
                engine and <b>REST API</b> make it easy to ask real questions — without writing verbose logic
                or managing queries manually.
              </p>
            </div>
            <CodeBlockWithLanguageSelector
              data={{ typescript: code2, python: code2Py }}
              className="mx-auto w-full max-w-xl"
              preClassName="md:w-full"
              wrapperClassName="rounded-bl-none rounded-br-none pb-0"
            />
          </div>

          <div className="outline-stroke flex h-full w-full flex-col justify-between rounded-br-[80px] pt-12 outline outline-1 outline-offset-0">
            <div className="mx-auto mb-8 max-w-xl md:px-6 md:text-center">
              <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Build Powerful Filters & Search</h4>
              <p className="text-content3 text-md !tracking-normal md:text-base">
                Create filters, lists, and <b>vector search</b> interfaces from a single <b>API</b>. No
                backend boilerplate, just a clean <b>no-code</b> <b>graph database</b>, with structured access
                to everything you store.
              </p>
            </div>
            <CodeBlockWithLanguageSelector
              data={{ typescript: code3, python: code3Py }}
              className="mx-auto w-full max-w-xl"
              preClassName="md:w-full"
              wrapperClassName="rounded-bl-none rounded-br-none pb-0"
            />
          </div>
        </div>
      </section>

      <section className="outline-stroke outline outline-1 outline-offset-0 md:hidden" id="demo">
        <div className="container">
          <div className="outline-stroke rounded-t-[80px] outline outline-1 outline-offset-0">
            <h3 className="typography-2xl md:typography-xl text mb-8 pt-16 text-center">
              Live Demo: See RushDB in Action
            </h3>
            <p className="text-content3 text-md mb-8 text-center !tracking-normal md:text-base">
              Experience the power of RushDB firsthand.
            </p>

            <Button
              as={Link}
              href={links.examples}
              target="_blank"
              size="small"
              variant="outline"
              className="bg-fill m-auto mb-16 w-fit"
            >
              Explore Examples <GitHub />
            </Button>

            <Demo />
          </div>
        </div>
      </section>

      <section className="outline-stroke outline outline-1 outline-offset-0" id="use-cases">
        <div className="container">
          <div className="outline-stroke outline outline-1 outline-offset-0">
            <h3 className="typography-2xl md:typography-xl text py-16 text-center">Use Cases</h3>

            <div className="grid grid-cols-2 gap-[1px]">
              <div className="outline-stroke row-end-1 w-full rounded-b-[80px] p-12 outline outline-1 outline-offset-0 md:col-span-2 md:row-end-1 md:rounded-b-[50px] md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">SaaS & Apps</h4>
                <p className="text-content3 text-md md:text-base">
                  Power your <b>development</b> with a scalable <b>graph database</b>. RushDB is a{' '}
                  <b>Firebase alternative</b>, <b>Supabase alternative</b>, and <b>cloud database</b> built
                  for modern <b>NoSQL</b> needs. It offers high-speed <b>data ingestion</b>,
                  <b>native graph storage</b>, and an unbeatable developer experience — ideal for{' '}
                  <b>data-intensive apps</b> and teams seeking a <b>developer-friendly database</b>.
                </p>
              </div>
              <div className="outline-stroke row-end-1 w-full rounded-r-[80px] p-12 outline outline-1 outline-offset-0 md:col-span-2 md:row-end-2 md:rounded-r-[50px] md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">AI & Machine Learning</h4>
                <p className="text-content3 text-md md:text-base">
                  Leverage RushDB as your <b>AI persistence</b> — perfect for <b>predictive analytics</b>,{' '}
                  <b>AI embeddings</b>, <b>recommendation systems</b>, and <b>vector search</b>. With{' '}
                  <b>low-latency</b> and <b>vector database</b> capabilities powered by a{' '}
                  <b>knowledge graph</b> engine, it’s optimized for <b>AI-first applications</b> and works as
                  a plug-and-play <b>backend as a service</b>.
                </p>
              </div>

              <div className="col-span-2 row-span-1 grid grid-cols-2 items-center gap-24 md:mt-0 md:grid-cols-1 md:gap-0">
                <div className="mb-8 items-center p-20 md:p-6 md:pb-0">
                  <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">
                    Persistence for AI Era: Smart and Simple
                  </h4>
                  <p className="text-content3 text-md md:text-base">
                    RushDB handles the complexity – so you can focus on building. Just push JSON, query
                    granularly via <b>JavaScript SDK</b>, <b>Python SDK</b>, or <b>REST API</b>, and let
                    automatic labeling and type suggestions do the rest. Ideal for <b>cloud-first</b> use
                    cases, <b>AI integrations</b>, and <b>GenAI</b> projects.
                  </p>
                </div>

                <div className="flex w-full items-center justify-start py-20 pr-8 md:flex-col md:p-0">
                  <CodeBlockWithLanguageSelector
                    data={{ typescript: codeAiIntegration, python: codeAiIntegrationPy }}
                    className="mx-auto w-full"
                    preClassName="md:w-full"
                    wrapperClassName="md:rounded-bl-none md:rounded-br-none md:pb-0"
                  />
                </div>
              </div>

              <div className="outline-stroke w-full rounded-t-[80px] p-12 outline outline-1 outline-offset-0 md:col-span-2 md:rounded-t-[50px] md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">IoT Projects</h4>
                <p className="text-content3 text-md md:text-base">
                  Don’t let infrastructure complexity slow down your IoT innovation. RushDB is a{' '}
                  <b>scalable database</b> that handles real-time data ingestion, <b>time-series database</b>{' '}
                  workloads, and dynamic schemas – making it a go-to <b>NoSQL database</b> for connected
                  devices.
                </p>
              </div>

              <div className="outline-stroke w-full rounded-l-[80px] rounded-br-[80px] p-12 outline outline-1 outline-offset-0 md:col-span-2 md:rounded-l-[50px] md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Search Engines</h4>
                <p className="text-content3 text-md md:text-base">
                  RushDB enables fast, flexible filtering of massive datasets — perfect for building custom{' '}
                  <b>search engines</b> using a <b>vector database</b> with <b>knowledge graph</b> support.
                  Optimized for performance and shape-agnostic queries.
                </p>
              </div>

              <div className="outline-stroke w-full rounded-t-[80px] p-12 outline outline-1 outline-offset-0 md:col-span-2 md:rounded-t-[50px] md:p-6">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Hobby Projects</h4>
                <p className="text-content3 text-md md:text-base">
                  Don’t let infra kill your weekend flow. RushDB gives you an <b>open source database</b> with{' '}
                  <b>zero config</b>, making it perfect for <b>developer-friendly</b> hacking. Push data,
                  build ideas fast, and ship in hours — not weeks.
                </p>
              </div>

              <div className="outline-stroke grid w-full items-center rounded-t-[80px] rounded-br-[80px] p-12 outline outline-1 outline-offset-0 md:hidden md:rounded-t-[50px] md:p-6">
                <p className="text-content3 text-md text-center md:text-base">
                  Create a project, grab your API token,
                  <br />
                  and start building in less than 30 seconds.
                </p>
                <div className="m-auto flex w-full justify-center gap-4">
                  <MainCta size="small" variant="accent" text="Create Project" />
                  <Button
                    as={Link}
                    href={links.getStarted}
                    size="small"
                    variant="outline"
                    className="bg-fill w-fit"
                  >
                    Read the Docs <BookIcon />
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="mt-[1px] border-b">
        <div className="container text-center">
          <div className="outline-stroke outline outline-1 outline-offset-0">
            <h3 className="typography-2xl md:typography-xl text pt-20">Fully Featured Dashboard</h3>
            <p className="text-content3 text-md pb-20 pt-8 !tracking-normal md:pb-8 md:text-base">
              Navigate your <b>data</b> quickly and clearly with an interface designed for <b>speed</b> and{' '}
              <b>clarity</b>.
            </p>
            <Image
              priority
              src={dashboard.src}
              alt="dashboard-preview"
              width={1894}
              height={989}
              className="w-full"
            />
          </div>
        </div>
      </section>
      <section className="outline-stroke outline outline-1 outline-offset-0">
        <div className="container text-center">
          <div className="outline-stroke rounded-t-full py-20 outline outline-1 outline-offset-0 md:rounded-t-[100px]">
            <h3 className="typography-2xl md:typography-xl text mb-8">
              Bring your own <span className="text-accent">Neo4j</span> instance
            </h3>
            <p className="text-content3 text-md mb-8 !tracking-normal md:text-base">
              <br className="md:hidden" />
              Scale your high-demand workloads with full data ownership.
            </p>
            <div className="m-auto flex w-full justify-center gap-4">
              <MainCta size="small" variant="accent" text="Create Project" />
              <Button as={Link} href={links.pricing} size="small" variant="outline" className="bg-fill">
                Explore Plans
              </Button>
            </div>
          </div>
        </div>
      </section>
      <section className="border-b">
        <div className="container text-center">
          <div className="outline-stroke rounded-b-full py-20 outline outline-1 outline-offset-0 md:rounded-b-[100px]">
            <h3 className="typography-2xl md:typography-xl text mb-8">Self-hosted? Simple.</h3>
            <p className="text-content3 text-md !tracking-normal md:text-base">
              1. Setup your Neo4j instance or use{' '}
              <Link
                href="https://neo4j.com/product/auradb/"
                target="_blank"
                className="underline decoration-dashed"
              >
                Neo4j Aura
              </Link>
            </p>
            <p className="text-content3 text-md mb-8 !tracking-normal md:text-base">
              2. Run RushDB container with Neo4j credentials
            </p>
            <CodeBlock
              language="bash"
              code={codeDocker}
              className="mb-8 grid place-content-center gap-[1px] md:w-full lg:w-full"
              preClassName="md:w-full"
            />
            <p className="text-content3 text-md !tracking-normal md:text-base">
              That's it. RushDB is ready at <br className="hidden md:block" />
              <span className="bold font-mono font-bold">localhost:3000</span> 🎉️
            </p>

            <Button
              as={Link}
              href={links.getStarted}
              size="small"
              variant="outline"
              className="bg-fill m-auto mt-8 w-fit"
            >
              Read the Docs <BookIcon />
            </Button>
          </div>
        </div>
      </section>

      <section className="outline-stroke outline outline-1 outline-offset-0">
        <div className="container text-center">
          <div className="outline-stroke rounded-full py-20 outline outline-1 outline-offset-0 md:rounded-[100px]">
            <h3 className="typography-2xl md:typography-xl text mb-8">
              Not an infra fan?
              <br className="hidden md:block" /> Try <span className="text-accent">RushDB Cloud</span>
            </h3>
            <p className="text-content3 text-md mb-8 !tracking-normal md:text-base">
              2 Projects Free Forever. No Maintenance Required.
              <br className="md:hidden" />
              Focus on building apps, not on managing infrastructure.
            </p>
            <div className="m-auto flex w-full justify-center gap-4">
              <MainCta size="small" variant="accent" text="Create Project" />
              <Button as={Link} href={links.pricing} size="small" variant="outline" className="bg-fill">
                Explore Plans
              </Button>
            </div>
          </div>
        </div>
      </section>
    </>
  )
}
