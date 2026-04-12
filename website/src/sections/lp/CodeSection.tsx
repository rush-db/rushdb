import { useState } from 'react'
import { motion } from 'framer-motion'
import { CodeBlock } from '~/components/CodeBlock'
import { LPSection, LPContainer, LPEyebrow, LPSectionHeading, LPTabBtn } from '~/components/lp/ui'
import { LangIcon } from '~/components/lp/LangIcon'

const TS_AFTER = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('RUSHDB_API_KEY')

// 1. Create an index — RushDB embeds 'output' on every write
await db.ai.indexes.create({ label: 'MEMORY', propertyName: 'output' })

// 2. Store a memory — no embedder, no vectors array
const { data: record } = await db.records.create({
  label: 'MEMORY',
  data: {
    agent_id: 'agent-42',
    action: 'summarized',
    topic: 'Q4 results',
    output: summaryText,
  },
})

// 3. Semantic recall — just pass the query string
const { data: recall } = await db.ai.search({
  label: 'MEMORY',
  propertyName: 'output',
  query: currentQuery,
  where: { agent_id: 'agent-42' },
  limit: 10,
})`

const PY_AFTER = `from rushdb import RushDB

db = RushDB('RUSHDB_API_KEY')

# 1. Create an index — RushDB embeds 'output' on every write
db.ai.indexes.create(label='MEMORY', property_name='output')

# 2. Store a memory — no embedder, no vectors array
db.records.create(
    label='MEMORY',
    data={
        'agent_id': 'agent-42',
        'action': 'summarized',
        'topic': 'Q4 results',
        'output': summary_text,
    },
)

# 3. Semantic recall — just pass the query string
response = db.ai.search({
    'label': 'MEMORY',
    'propertyName': 'output',
    'query': current_query,
    'where': {'agent_id': 'agent-42'},
    'limit': 10,
})`

const SHELL_AFTER = `# 1. Create an index — one-time setup
curl -X POST https://api.rushdb.com/api/v1/ai/indexes \\\n  -H 'Authorization: Bearer RUSHDB_API_KEY' \\\n  -H 'Content-Type: application/json' \\\n  -d '{"label": "MEMORY", "propertyName": "output"}'

# 2. Store a memory — no vectors needed
curl -X POST https://api.rushdb.com/api/v1/records \\\n  -H 'Authorization: Bearer RUSHDB_API_KEY' \\\n  -H 'Content-Type: application/json' \\\n  -d '{
    "label": "MEMORY",
    "data": {
      "agent_id": "agent-42",
      "action": "summarized",
      "output": "Summarized Q4 results..."
    }
  }'

# 3. Semantic search — pass query text, no vector needed
curl -X POST https://api.rushdb.com/api/v1/ai/search \\\n  -H 'Authorization: Bearer RUSHDB_API_KEY' \\\n  -H 'Content-Type: application/json' \\\n  -d '{
    "labels": ["MEMORY"],
    "propertyName": "output",
    "query": "Q4 financial summary",
    "where": { "agent_id": "agent-42" },
    "limit": 10
  }'`

const BEFORE_CODE = `# 3 services. 1 pipeline. Schema meeting required.

pip install pinecone neo4j redis

# 1. Define Neo4j schema
# 2. Write embedding pipeline
# 3. Sync to Pinecone
# 4. Cache in Redis
# 5. Write join logic
# ... 200 lines later`

type Lang = 'typescript' | 'python' | 'shell'

export function LPCodeSection() {
  const [lang, setLang] = useState<Lang>('typescript')

  return (
    <LPSection surface borderY>
      <LPContainer>
        <LPEyebrow>Code</LPEyebrow>
        <LPSectionHeading className="mb-12">Before and after.</LPSectionHeading>

        {/* Language tabs */}
        <div className="mb-6 flex gap-1">
          {(['typescript', 'python', 'shell'] as Lang[]).map((l) => (
            <LPTabBtn key={l} active={lang === l} onClick={() => setLang(l)}>
              <LangIcon lang={l} size={14} className="inline-block shrink-0" />
              {l}
            </LPTabBtn>
          ))}
        </div>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.45 }}
          className="grid grid-cols-2 gap-4 md:grid-cols-1"
        >
          {/* Before */}
          <div className="border-lp-border flex flex-col overflow-hidden rounded-none border">
            <div
              className="flex items-center gap-2 border-b px-4 py-2 font-mono text-sm uppercase tracking-wider"
              style={{
                background: 'rgba(255,68,68,0.12)',
                borderColor: 'var(--lp-border)',
                color: '#ff6b6b'
              }}
            >
              <span className="h-2 w-2 rounded-full bg-[#ff4444]" />
              Before
            </div>
            <div className="flex-1">
              <CodeBlock code={BEFORE_CODE} language="bash" className="h-full text-xs" />
            </div>
          </div>

          {/* After */}
          <div className="border-lp-border flex flex-col overflow-hidden rounded-none border">
            <div
              className="flex items-center gap-2 border-b px-4 py-2 font-mono text-sm uppercase tracking-wider"
              style={{
                background: 'rgba(var(--lp-accent-rgb), 0.10)',
                borderColor: 'var(--lp-border)',
                color: 'var(--lp-accent)'
              }}
            >
              <span className="bg-lp-accent h-2 w-2 rounded-full" />
              After — RushDB
            </div>
            <div className="flex-1">
              <CodeBlock
                code={
                  lang === 'typescript' ? TS_AFTER
                  : lang === 'python' ?
                    PY_AFTER
                  : SHELL_AFTER
                }
                language={
                  lang === 'typescript' ? 'typescript'
                  : lang === 'python' ?
                    'python'
                  : 'bash'
                }
                className="h-full text-xs"
              />
            </div>
          </div>
        </motion.div>
      </LPContainer>
    </LPSection>
  )
}
