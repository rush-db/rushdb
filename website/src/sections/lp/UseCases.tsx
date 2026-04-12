import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { CodeBlock } from '~/components/CodeBlock'
import { LPSection, LPContainer, LPEyebrow, LPSectionHeading, LPTabBtn } from '~/components/lp/ui'
import { LangIcon } from '~/components/lp/LangIcon'

type Lang = 'typescript' | 'python' | 'shell'

interface Tab {
  label: string
  tagline: string
  body: string
  why: string
  codes: Record<Lang, string>
}

const TABS: Tab[] = [
  {
    label: 'Agent Memory',
    tagline: 'Every agent action, indexed and retrievable.',
    body: 'Store and retrieve agent state, tool outputs, and conversation history across sessions. Query by agent ID, time range, or semantic similarity — all in one call.',
    why: 'No separate vector store. Graph links sessions, actions, and context automatically.',
    codes: {
      typescript: `// One-time setup: tell RushDB to auto-embed 'output' on every write
await db.ai.indexes.create({ label: 'MEMORY', propertyName: 'output' })

// Store a memory — no embedder needed, server handles it
const { data: record } = await db.records.create({
  label: 'MEMORY',
  data: {
    agent_id: 'agent-42',
    action: 'summarized',
    topic: 'Q4 results',
    output: summaryText,
  },
})

// Recall semantically — just pass the query string
const { data: recall } = await db.ai.search({
  labels: ['MEMORY'],
  propertyName: 'output',
  query: currentQuery,
  where: { agent_id: 'agent-42' },
  limit: 10,
})`,
      python: `# One-time setup: tell RushDB to auto-embed 'output' on every write
db.ai.indexes.create(label='MEMORY', property_name='output')

# Store a memory — no embedder needed, server handles it
record = db.records.create(
    label='MEMORY',
    data={
        'agent_id': 'agent-42',
        'action': 'summarized',
        'topic': 'Q4 results',
        'output': summary_text,
    },
)

# Recall semantically — just pass the query string
response = db.ai.search({
    'labels': ['MEMORY'],
    'propertyName': 'output',
    'query': current_query,
    'where': {'agent_id': 'agent-42'},
    'limit': 10,
})`,
      shell: `# One-time setup: create a managed embedding index
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{"label": "MEMORY", "propertyName": "output"}'

# Write record — 'output' is auto-embedded server-side
curl -X POST https://api.rushdb.com/api/v1/records \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "MEMORY",
    "data": { "agent_id": "agent-42", "output": "Summarized Q4 results..." }
  }'

# Semantic search — query text, no vector needed
curl -X POST https://api.rushdb.com/api/v1/ai/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["MEMORY"],
    "propertyName": "output",
    "query": "Q4 financial summary",
    "where": { "agent_id": "agent-42" },
    "limit": 10
  }'`
    }
  },
  {
    label: 'Knowledge Base',
    tagline: 'Ingest anything. Search everything.',
    body: 'Push documents, CSVs, API dumps — query them semantically without building a pipeline. Relationships between docs are inferred automatically.',
    why: 'Push docs in any shape and search semantically without a pipeline.',
    codes: {
      typescript: `// One-time setup: auto-embed 'content' on every document write
await db.ai.indexes.create({ label: 'DOCUMENT', propertyName: 'content' })

// Ingest a document — content is embedded server-side automatically
const { data: doc } = await db.records.create({
  label: 'DOCUMENT',
  data: {
    title: document.title,
    content: document.text,
    source: document.url,
    author: document.author,
  },
})

// Search — just pass the query string
const { data: results } = await db.ai.search({
  labels: ['DOCUMENT'],
  propertyName: 'content',
  query: userQuery,
  where: { source: { $contains: 'blog' } },
  limit: 10,
})`,
      python: `# One-time setup: auto-embed 'content' on every document write
db.ai.indexes.create(label='DOCUMENT', property_name='content')

# Ingest a document — content is embedded server-side automatically
db.records.create(
    label='DOCUMENT',
    data={
        'title': document['title'],
        'content': document['text'],
        'source': document['url'],
        'author': document['author'],
    },
)

# Search — just pass the query string
response = db.ai.search({
    'labels': ['DOCUMENT'],
    'propertyName': 'content',
    'query': user_query,
    'where': {'source': {'$contains': 'blog'}},
    'limit': 10,
})`,
      shell: `# One-time setup: create a managed embedding index
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{"label": "DOCUMENT", "propertyName": "content"}'

# Ingest a document — content is auto-embedded
curl -X POST https://api.rushdb.com/api/v1/records \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "DOCUMENT",
    "data": { "title": "...", "content": "...", "source": "https://..." }
  }'

# Semantic + structural search — query text, no vector needed
curl -X POST https://api.rushdb.com/api/v1/ai/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["DOCUMENT"],
    "propertyName": "content",
    "query": "getting started guide",
    "where": { "source": { "$contains": "blog" } },
    "limit": 10
  }'`
    }
  },
  {
    label: 'Graph RAG',
    tagline: 'Find context flat search misses.',
    body: 'Multi-hop retrieval via graph relationships. Discover related context chunks that nearest-neighbor search would never surface.',
    why: 'Not just nearest neighbor — connected neighbor. Graph traversal + semantic similarity in one query.',
    codes: {
      typescript: `// One-time setup: auto-embed 'text' on every CHUNK write
await db.ai.indexes.create({ label: 'CHUNK', propertyName: 'text' })

// Graph RAG — semantic search + multi-hop graph traversal
async function retrieve(query: string, k = 5) {
  // 1. Find top-k chunks by semantic similarity
  const { data: chunks } = await db.ai.search({
    query, propertyName: 'text', labels: ['CHUNK'], limit: k,
  })

  // 2. Enrich with graph context: SOURCE → HAS_CHUNK → CHUNK
  return Promise.all(chunks.map(async (chunk) => {
    const { data: [source] } = await db.records.find({
      labels: ['SOURCE'],
      where: {
        CHUNK: { $relation: { type: 'HAS_CHUNK', direction: 'out' }, __id: chunk.__id },
      },
    })
    const { data: [author] } = await db.records.find({
      labels: ['AUTHOR'],
      where: {
        SOURCE: { $relation: { type: 'AUTHORED', direction: 'out' }, __id: source?.__id },
      },
    })
    return { text: chunk.text, score: chunk.__score, file: source?.filename, author: author?.name }
  }))
}`,
      python: `# One-time setup: auto-embed 'text' on every CHUNK write
db.ai.indexes.create(label='CHUNK', property_name='text')

# Step 1: find top-k chunks by semantic similarity
results = db.ai.search({
    'query': query, 'propertyName': 'text', 'labels': ['CHUNK'], 'limit': 5,
})

# Step 2: enrich with graph context — SOURCE → HAS_CHUNK → CHUNK
context = []
for chunk in results.data:
    src = db.records.find({
        'labels': ['SOURCE'],
        'where': {'CHUNK': {'$relation': {'type': 'HAS_CHUNK', 'direction': 'out'}, '__id': chunk.id}},
    }).data
    source = src[0] if src else None
    author = None
    if source:
        authors = db.records.find({
            'labels': ['AUTHOR'],
            'where': {'SOURCE': {'$relation': {'type': 'AUTHORED', 'direction': 'out'}, '__id': source.id}},
        }).data
        author = authors[0] if authors else None
    context.append({
        'text': chunk.data['text'], 'score': chunk.data.get('__score'),
        'file': source.data.get('filename') if source else None,
        'author': author.data.get('name') if author else None,
    })`,
      shell: `# One-time setup: create a managed embedding index
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{"label": "CHUNK", "propertyName": "text"}'

# Step 1: semantic search — get top-k relevant chunks
curl -X POST https://api.rushdb.com/api/v1/ai/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["CHUNK"],
    "propertyName": "text",
    "query": "caching layer invalidation",
    "limit": 5
  }'

# Step 2: single nested traversal — multi-hop in one query
# Find AUTHORS who wrote SOURCES that contain matching CHUNKs
curl -X POST https://api.rushdb.com/api/v1/records/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["AUTHOR"],
    "where": {
      "SOURCE": {
        "$relation": { "type": "AUTHORED", "direction": "out" },
        "CHUNK": {
          "$relation": { "type": "HAS_CHUNK", "direction": "out" },
          "text": { "$contains": "caching" }
        }
      }
    }
  }'`
    }
  },
  {
    label: 'Data Exploration',
    tagline: 'Your data, shaped on arrival.',
    body: 'Stop modeling before ingesting. Push CSV or JSON from any API. Explore the structure after the fact — no migration headaches.',
    why: 'Zero-schema ingestion. Explore relationships discovered automatically.',
    codes: {
      typescript: `// Drop in any JSON records — no schema needed
await db.records.createMany({
  label: 'EVENT',
  data: csvRows,
})

// Query whatever you find
const { data: signups } = await db.records.find({
  labels: ['EVENT'],
  where: { category: 'signup' },
  orderBy: { timestamp: 'DESC' },
  limit: 50,
})`,
      python: `# Drop in any records — no schema needed
db.records.create_many(
    label='EVENT',
    data=csv_rows,
)

# Query whatever you find
response = db.records.find(
    labels=['EVENT'],
    where={'category': 'signup'},
    order_by=[{'field': 'timestamp', 'direction': 'DESC'}],
    limit=50,
)`,
      shell: `# Bulk ingest — no schema required
curl -X POST https://api.rushdb.com/api/v1/records/import/json \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "EVENT",
    "data": [
      { "category": "signup", "timestamp": "2026-01-01", "user": "u_1" },
      { "category": "purchase", "timestamp": "2026-01-02", "user": "u_2" }
    ]
  }'

# Query after the fact
curl -X POST https://api.rushdb.com/api/v1/records/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["EVENT"],
    "where": { "category": "signup" },
    "orderBy": { "timestamp": "DESC" }
  }'`
    }
  },
  {
    label: 'AI Analytics',
    tagline: 'Ontology-aware agents that understand your domain.',
    body: 'Model your domain as typed entities and relationships. Agents traverse the ontology to answer questions no flat query could — products → categories → trends → competitors.',
    why: 'Graph traversal turns raw data into domain knowledge. Agents reason across entities, not just rows.',
    codes: {
      typescript: `// One-time setup: auto-embed product names for semantic search
await db.ai.indexes.create({ label: 'PRODUCT', propertyName: 'name' })

// Build ontology: Product → Category → Market — no manual embedding
await db.records.createMany({
  label: 'PRODUCT',
  data: products.map(p => ({
    name: p.name,
    category: p.category,
    market: p.market,
    revenue: p.revenue,
  })),
})

// Agent: find all products in markets similar to "AI infrastructure"
const { data: similar } = await db.ai.search({
  labels: ['PRODUCT'],
  propertyName: 'name',
  query: 'AI infrastructure tooling',
  where: { revenue: { $gt: 100000 } },
  limit: 20,
})`,
      python: `# One-time setup: auto-embed product names for semantic search
db.ai.indexes.create(label='PRODUCT', property_name='name')

# Build ontology: Product → Category → Market — no manual embedding
db.records.create_many(
    label='PRODUCT',
    data=[{
        'name': p['name'],
        'category': p['category'],
        'market': p['market'],
        'revenue': p['revenue'],
    } for p in products],
)

# Agent: find products in markets similar to a concept
response = db.ai.search({
    'labels': ['PRODUCT'],
    'propertyName': 'name',
    'query': 'AI infrastructure tooling',
    'where': {'revenue': {'$gt': 100000}},
    'limit': 20,
})`,
      shell: `# One-time setup: create a managed embedding index
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{"label": "PRODUCT", "propertyName": "name"}'

# Ingest ontology nodes — names are auto-embedded
curl -X POST https://api.rushdb.com/api/v1/records/import/json \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "PRODUCT",
    "data": [
      { "name": "RushDB", "category": "database", "market": "AI infra", "revenue": 500000 }
    ]
  }'

# Agent semantic + structural query — pass text, no vector needed
curl -X POST https://api.rushdb.com/api/v1/ai/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["PRODUCT"],
    "propertyName": "name",
    "query": "AI infrastructure tooling",
    "where": { "revenue": { "$gt": 100000 } },
    "limit": 20
  }'`
    }
  },
  {
    label: 'Vibe Coding',
    tagline: 'Ship your idea before the session ends.',
    body: 'No schema design, no migrations, no ORM setup. Push whatever JSON your app produces and start querying immediately. Perfect for prototypes, hackathons, and exploratory builds.',
    why: 'Zero config, instant graph + vector. Iterate on data shape without touching a migration file.',
    codes: {
      typescript: `// Just push data — figure out shape later
await db.records.createMany({
  label: 'POST',
  data: posts,
})

// Query whatever you decided to store
const { data: trending } = await db.records.find({
  labels: ['POST'],
  where: {
    likes: { $gt: 100 },
    published: true,
  },
  orderBy: { likes: 'DESC' },
  limit: 10,
})

// Add semantic search in two lines — no embedder needed
await db.ai.indexes.create({ label: 'POST', propertyName: 'body' })
const { data: similar } = await db.ai.search({
  labels: ['POST'], propertyName: 'body', query: text, limit: 5,
})`,
      python: `# Just push data — figure out shape later
db.records.create_many(
    label='POST',
    data=posts,
)

# Query whatever you decided to store
response = db.records.find(
    labels=['POST'],
    where={
        'likes': {'$gt': 100},
        'published': True,
    },
    order_by=[{'field': 'likes', 'direction': 'DESC'}],
    limit=10,
)

# Add semantic search in two lines — no embedder needed
db.ai.indexes.create(label='POST', property_name='body')
results = db.ai.search({
    'labels': ['POST'], 'propertyName': 'body', 'query': text, 'limit': 5,
})`,
      shell: `# Push any shape — no schema required
curl -X POST https://api.rushdb.com/api/v1/records/import/json \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "POST",
    "data": [
      { "title": "Fast idea", "likes": 142, "published": true }
    ]
  }'

# Instant structured query
curl -X POST https://api.rushdb.com/api/v1/records/search \\
  -H 'Authorization: Bearer RUSHDB_API_KEY' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["POST"],
    "where": { "likes": { "$gt": 100 }, "published": true },
    "orderBy": { "likes": "DESC" },
    "limit": 10
  }'`
    }
  }
]

export function LPUseCases() {
  const [active, setActive] = useState(0)
  const [lang, setLang] = useState<Lang>('typescript')

  return (
    <LPSection>
      <LPContainer>
        <LPEyebrow>Use cases</LPEyebrow>
        <LPSectionHeading className="mb-10">Built for.</LPSectionHeading>

        {/* Use-case tabs */}
        <div className="border-lp-border mb-8 flex flex-wrap gap-1 border-b pb-0">
          {TABS.map((tab, i) => (
            <button
              key={i}
              onClick={() => setActive(i)}
              className={[
                '-mb-px border-b-2 px-4 py-3 font-mono text-sm uppercase tracking-wider transition-colors',
                active === i ?
                  'border-lp-accent text-lp-accent'
                : 'text-lp-muted hover:text-lp-text border-transparent'
              ].join(' ')}
            >
              {tab.label}
            </button>
          ))}
        </div>

        {/* Tab panel */}
        <AnimatePresence mode="wait">
          <motion.div
            key={active}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
            className="grid grid-cols-2 gap-8 md:grid-cols-1"
          >
            {/* Text column */}
            <div className="flex flex-col justify-center gap-4">
              <p className="text-lp-text font-mono text-sm font-bold">"{TABS[active].tagline}"</p>
              <p className="text-lp-muted font-sans text-sm leading-relaxed">{TABS[active].body}</p>
              <div className="border-lp-accent border-l-2 pl-4">
                <p className="text-lp-muted font-mono text-sm">
                  <span className="text-lp-accent font-bold">Why RushDB: </span>
                  {TABS[active].why}
                </p>
              </div>
            </div>

            {/* Code column */}
            <div className="border-lp-border flex flex-col overflow-hidden border">
              {/* Language switcher */}
              <div className="border-lp-border flex gap-1 border-b px-3 py-2">
                {(['typescript', 'python', 'shell'] as Lang[]).map((l) => (
                  <LPTabBtn key={l} active={lang === l} onClick={() => setLang(l)}>
                    <LangIcon lang={l} size={14} className="inline-block shrink-0" />
                    {l}
                  </LPTabBtn>
                ))}
              </div>
              <CodeBlock
                code={TABS[active].codes[lang]}
                language={lang === 'shell' ? 'bash' : lang}
                className="text-xs"
              />
            </div>
          </motion.div>
        </AnimatePresence>
      </LPContainer>
    </LPSection>
  )
}
