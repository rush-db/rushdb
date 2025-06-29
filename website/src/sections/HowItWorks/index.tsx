import { Button, MainCta } from '~/components/Button'
import Link from 'next/link'
import { links } from '~/config/urls'
import { ArrowLeftRight, ArrowRight, BookIcon } from 'lucide-react'
import { CodeBlock } from '~/components/CodeBlock'
import Image from 'next/image'

import dashboard from '../../images/dashboard.png'
import { CodeBlockWithLanguageSelector } from '~/components/CodeBlockWithLanguageSelector'
import { useContext } from 'react'
import { CodingLanguage } from '~/contexts/CodingLanguage'
import { Demo } from '~/sections/HowItWorks/demo'
import { GitHub } from '~/components/Icons/GitHub'
import { YouTubeEmbed } from '~/components/YouTubeEmbed'
import { PropertyGraphTopology } from '~/components/PropertyGraphTopology'

const code1 = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

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

db = RushDB("RUSHDB_API_KEY")

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

const code2Result = `// Response from RushDB
{
  "data": [{
    "__id": "019412c0-2051-7000-8000-850db63a1e77",
    "__label": "COMPANY",
    "name": "TechStart Inc",
    "stage": "seed",
    "address": "123 Innovation St, Austin, TX, USA",
    "foundedAt": "1999-03-15T00:00:00.000Z",
    "rating": 4.8,
    "employees": [
      {
        "__id": "019412c0-2051-7000-8000-850db63a1e78",
        "__label": "EMPLOYEE",
        "name": "Sarah Chen",
        "position": "Lead Engineer",
        "salary": 650000
      },
      {
        "__id": "019412c0-2051-7000-8000-850db63a1e79",
        "__label": "EMPLOYEE",
        "name": "Marcus Rodriguez",
        "position": "Senior Developer",
        "salary": 580000
      }
    ]
  }],
  "success": true,
  "total": 1
}`

const codeDocker = `docker run -p 3000:3000 --name rushdb \\
-e NEO4J_URL='bolt+s://your-neo4j-instance-url:7687' \\
-e NEO4J_USERNAME='neo4j' \\
-e NEO4J_PASSWORD='password' \\
rushdb/platform`

const codeAiIntegration = `import OpenAI from 'openai'
import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')
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

db = RushDB("RUSHDB_API_KEY")
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

const codeRelationships = `// Create records
const user = await db.records.create({
  label: 'USER',
  data: { name: 'John Doe', email: 'john@example.com' }
})

const project = await db.records.create({
  label: 'PROJECT',
  data: { name: 'Website Redesign', deadline: '2025-06-30' }
})

// Connect them with a relationship
await db.records.attach({
  source: user,
  target: project,
  options: {
    type: 'MANAGES'
  }
})

// Query related data naturally
const userProjects = await db.records.find({
  labels: ['PROJECT'],
  where: {
    USER: {
      $relation: {
        type : 'MANAGES'
      },
    }
  }
})`

const codeRelationshipsPy = `# Create records
user = db.records.create(
    label="USER",
    data={"name": "John Doe", "email": "john@example.com"}
)

project = db.records.create(
    label="PROJECT",
    data={"name": "Website Redesign", "deadline": "2025-06-30"}
)

# Connect them with a relationship
db.records.attach(
    source=user,
    target=project,
    options={
        "type": "MANAGES"
    }
)

# Query related data naturally
user_projects = db.records.find({
    "labels": ["PROJECT"],
    "where": {
        "USER": {
            "$relation": {
                "type" : "MANAGES"
            },
        }
    }
})`

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

      <section className="container">
        <div className="outline-stroke rounded-b-full py-20 text-center outline outline-1 outline-offset-0 md:rounded-b-[100px]">
          <h3 className="typography-2xl md:typography-xl text px-6">Smart Auto-Normalization</h3>
          <div className="mt-4 text-center">
            <p className="text-content3 text-md">
              From a single JSON payload <ArrowRight size={16} /> <b>4 normalized Records</b> with{' '}
              <b>automatic Relationships</b>
            </p>
          </div>
          <div className="mx-auto max-w-4xl">
            <div className="rounded-lg p-8 md:p-4">
              <div className="flex items-center justify-center gap-4 md:grid md:grid-cols-2 md:gap-2">
                <div className="text-center">
                  <div className="bg-accent-yellow mx-auto mb-2 h-4 w-4 rounded-full"></div>
                  <span className="text-content font-mono text-sm font-bold">COMPANY</span>
                </div>

                <div className="text-content3 md:hidden">
                  <ArrowRight />
                </div>

                <div className="text-center">
                  <div className="bg-accent-orange mx-auto mb-2 h-4 w-4 rounded-full"></div>
                  <span className="text-content font-mono text-sm font-bold">DEPARTMENT</span>
                </div>

                <div className="text-content3 md:hidden">
                  <ArrowRight />
                </div>

                <div className="text-center">
                  <div className="mx-auto mb-2 h-4 w-4 rounded-full bg-green-600"></div>
                  <span className="text-content font-mono text-sm font-bold">PROJECT</span>
                </div>

                <div className="text-content3 md:hidden">
                  <ArrowRight />
                </div>

                <div className="text-center">
                  <div className="bg-accent-purple mx-auto mb-2 h-4 w-4 rounded-full"></div>
                  <span className="text-content font-mono text-sm font-bold">EMPLOYEE</span>
                </div>
              </div>
            </div>
          </div>

          <p className="text-content3 text-md px-6 pb-12 pt-4 !tracking-normal md:text-base">
            RushDB automatically creates <b>Records</b> with proper relationships and types from your JSON.
            <br className="md:hidden" />
            Zero schemas, maximum speed — built for <b>SaaS</b>, <b>AI apps</b>, and <b>fast-moving teams</b>.
          </p>
        </div>
      </section>

      <section className="outline-stroke my-[1px] outline outline-1 outline-offset-0">
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
            <span className="text-content font-mono text-xl font-bold md:text-lg">Transactions (ACID)</span>{' '}
            <p className="text-md text-content3 md:text-base">Ensures data integrity and reliability</p>
          </div>

          <div className="outline-stroke row-span-2 w-full rounded-full rounded-br-none p-8 outline outline-1 outline-offset-0 md:hidden"></div>
        </div>
      </section>

      <section>
        <div className="container text-center">
          <div className="outline-stroke rounded-t-[80px] pt-20 outline outline-1 outline-offset-0 md:pb-8">
            <h3 className="typography-2xl md:typography-xl text px-6">Push JSON. Query JSON.</h3>
            <p className="text-content3 text-md px-6 pt-8 !tracking-normal md:text-base">
              If you can push JSON, why not query with JSON too?
              <br className="md:hidden" />
              Simple, consistent, <b>developer-friendly</b> — no SQL, no complexity.
            </p>

            <div className="mx-auto max-w-2xl">
              <div className="p-6 md:p-4">
                <div className="text-center">
                  <div className="flex items-center justify-center gap-3">
                    <span className="text-content bg-fill rounded px-3 py-1 font-mono text-lg">JSON in</span>
                    <span className="text-content3">
                      <ArrowLeftRight />
                    </span>
                    <span className="text-content bg-fill rounded px-3 py-1 font-mono text-lg">JSON out</span>
                  </div>
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
              <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Simple Syntax for Complex Queries</h4>
              <p className="text-content3 text-md !tracking-normal md:text-base">
                RushDB’s <b>graph database</b> engine and <b>API</b>s make it easy to ask real questions —
                without writing verbose logic or managing queries manually.
              </p>
            </div>
            <CodeBlockWithLanguageSelector
              data={{ typescript: code2, python: code2Py }}
              className="mx-auto w-full max-w-xl"
              preClassName="md:w-full"
              wrapperClassName="rounded-lg"
            />
          </div>

          <div className="outline-stroke flex h-full w-full flex-col justify-between rounded-br-[80px] pt-12 outline outline-1 outline-offset-0">
            <div className="mx-auto mb-8 max-w-xl md:px-6 md:text-center">
              <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">Clean JSON responses every time</h4>
              <p className="text-content3 text-md !tracking-normal md:text-base">
                Structured <b>JSON responses</b> with clear data shapes. No (de)normalization headaches, just
                clean results ready for your app.
              </p>
            </div>
            <CodeBlock
              code={code2Result}
              language="json"
              className="mx-auto w-full max-w-xl"
              preClassName="md:w-full"
              wrapperClassName="rounded-lg"
            />
          </div>
        </div>
      </section>

      <section className="container m-auto">
        <div className="rounded-[80px] p-12 md:rounded-[50px] md:p-8">
          <div className="grid grid-cols-2 md:grid-cols-1 md:gap-8">
            <div className="flex flex-col justify-between">
              <div className="mb-4">
                <h4 className="mb-8 text-xl font-bold md:mb-4 md:text-lg">
                  Smart Faceted Search Out of the Box
                </h4>
                <p className="text-content3 text-md mb-4 !tracking-normal md:text-base">
                  RushDB's <b>property graph</b> design means every filter makes your search <b>smarter</b>.
                  The more you filter, the more precise your options become — like marketplace category
                  filters that automatically reduce brand choices, but for <b>any dataset at any scale</b>.
                </p>
                <p className="text-content3 text-md !tracking-normal md:text-base">
                  <b>Properties</b> are managed <b>autonomously</b> by RushDB — search capabilities are
                  delivered instantly at record creation time. No manual indexing, no configuration — just
                  intelligent <b>faceted search</b> that works out of the box.
                </p>
              </div>
              <div className="text-center md:text-left">
                <Button
                  as={Link}
                  href={links.storage}
                  target="_blank"
                  size="small"
                  variant="outline"
                  className="bg-fill w-fit"
                >
                  RushDB Storage <BookIcon />
                </Button>
              </div>
            </div>

            <div className="flex flex-col justify-between">
              {/* Enhanced Property Graph Visualization */}
              <div className="flex-1">
                <div className="bg-fill flex h-full min-h-[200px] flex-col justify-center rounded-lg">
                  <PropertyGraphTopology />
                  <p className="text-content text-center text-sm font-medium !tracking-normal">
                    Property Graph Topology
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="border-b border-t">
        <div className="container m-auto">
          <div className="rounded-[80px] p-12 md:rounded-[50px] md:p-8">
            <div className="mb-12 text-center">
              <h3 className="typography-2xl md:typography-xl text mb-6">Relationships That Make Sense</h3>
              <p className="text-content3 text-md mx-auto max-w-4xl !tracking-normal md:text-base">
                Connect your data naturally. It's like foreign keys but without the mental overhead —
                <br className="md:hidden" />
                control relationships the way you actually think about them.
              </p>
            </div>

            <div className="grid grid-cols-2 gap-12 md:grid-cols-1 md:gap-8">
              <div className="flex flex-col justify-between">
                <div className="mb-8">
                  <h4 className="mb-6 text-xl font-bold md:mb-4 md:text-lg">Beyond Firebase Limitations</h4>
                  <p className="text-content3 text-md mb-4 !tracking-normal md:text-base">
                    <b>Firebase</b> forces you into document hierarchies. <b>Supabase</b> and ORMs make you
                    overthink schemas and joins.
                  </p>
                  <p className="text-content3 text-md mb-6 !tracking-normal md:text-base">
                    RushDB's <b>graph architecture</b> connects data instantly. No complex joins, no rigid
                    schemas — just natural <b>relationships</b> that scale.
                  </p>

                  <div className="bg-fill mb-6 rounded-lg">
                    <div className="flex items-center gap-8 py-8">
                      <div>
                        <div className="bg-accent-yellow mx-auto mb-2 h-4 w-4 rounded-full"></div>
                        <span className="text-content font-mono text-sm font-bold">USER</span>
                      </div>
                      <div className="flex items-center justify-center">
                        <span className="text-content3 font-mono text-sm">
                          MANAGES <ArrowRight />
                        </span>
                      </div>
                      <div>
                        <div className="bg-accent-purple mx-auto mb-2 h-4 w-4 rounded-full"></div>
                        <span className="text-content font-mono text-sm font-bold">PROJECT</span>
                      </div>
                    </div>
                  </div>

                  <Button
                    as={Link}
                    href={links.relationships}
                    target="_blank"
                    size="small"
                    variant="outline"
                    className="bg-fill w-fit"
                  >
                    RushDB Relationships <BookIcon />
                  </Button>
                </div>
              </div>

              <div className="flex flex-col justify-between">
                <CodeBlockWithLanguageSelector
                  data={{ typescript: codeRelationships, python: codeRelationshipsPy }}
                  className="w-full"
                  preClassName="md:w-full"
                  wrapperClassName="rounded-lg"
                />
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* {add section here} */}

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
                  Building <b>NLP pipelines</b> and <b>Graph RAG</b> was never simpler. RushDB serves as your{' '}
                  <b>AI persistence</b> layer — perfect for <b>predictive analytics</b>, <b>AI embeddings</b>,{' '}
                  <b>recommendation systems</b>, and <b>vector search</b>. With <b>low-latency</b> and{' '}
                  <b>vector database</b> capabilities powered by a <b>knowledge graph</b> engine, it's
                  optimized for <b>AI-first applications</b> and works as a plug-and-play{' '}
                  <b>backend as a service</b>.
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
                  and start building in less than 15 seconds.
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
              src={dashboard.src}
              alt="dashboard-preview"
              width={1894}
              height={989}
              className="w-full"
              loading="lazy"
            />
          </div>
        </div>
      </section>

      <section className="border-b">
        <div className="container text-center">
          <div className="outline-stroke outline outline-1 outline-offset-0">
            <h3 className="typography-2xl md:typography-xl text pt-20">See RushDB in Action</h3>
            <p className="text-content3 text-md pb-8 pt-8 !tracking-normal md:text-base">
              Watch how easy it is to build with RushDB. From zero to production in minutes.
            </p>
            <div className="mx-auto max-w-4xl px-6 pb-20">
              <YouTubeEmbed
                videoId="NKgNV3y_wVY"
                title="RushDB Demo - Build faster with zero-config database"
                className="shadow-lg"
              />
            </div>
          </div>
        </div>
      </section>
      <section className="outline-stroke outline outline-1 outline-offset-0">
        <div className="container text-center">
          <div className="outline-stroke rounded-bl-[80px] rounded-tr-[80px] py-20 outline outline-1 outline-offset-0 md:rounded-bl-[50px] md:rounded-tr-[50px]">
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
              <Link href="https://neo4j.com/product/auradb/" target="_blank" className="underline">
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
