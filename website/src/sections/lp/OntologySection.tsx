import { useState } from 'react'
import { motion } from 'framer-motion'
import { CodeBlock } from '~/components/CodeBlock'
import { LangIcon } from '~/components/lp/LangIcon'
import {
  fadeUp,
  LPSection,
  LPContainer,
  LPEyebrow,
  LPSectionHeading,
  LPTabBtn,
  LPAccentCard
} from '~/components/lp/ui'

type OntologyFormat = 'md' | 'json'
type CodeLang = 'typescript' | 'python' | 'shell'

const MD_EXAMPLE = `# Graph Ontology

## Labels

### PRODUCT
**Records:** 4,821
**Properties:**
- \`name\` (string) — 4,821 records
- \`price\` (number) — range: 9.99 – 2,499.00 — 4,821 records
- \`category\` (string) — values: "electronics", "furniture", "clothing"
- \`inStock\` (boolean) — 4,609 records
- \`rating\` (number) — range: 1.0 – 5.0 — 3,104 records

### ORDER
**Records:** 12,304
**Properties:**
- \`total\` (number) — range: 9.99 – 8,750.00 — 12,304 records
- \`status\` (string) — values: "pending", "shipped", "delivered", "cancelled"
- \`createdAt\` (datetime) — range: 2024-01-01 – 2026-04-10

### CUSTOMER
**Records:** 6,882
**Properties:**
- \`email\` (string) — 6,882 records
- \`country\` (string) — values: "US", "DE", "GB", "FR", "JP"
- \`tier\` (string) — values: "free", "pro", "enterprise"

## Relationships

- CUSTOMER → [:PLACED] → ORDER     (1-to-many)
- ORDER    → [:CONTAINS] → PRODUCT (1-to-many)`

const JSON_EXAMPLE = `{
  "labels": [
    {
      "name": "PRODUCT",
      "count": 4821,
      "properties": [
        { "name": "price", "type": "number",
          "min": 9.99, "max": 2499.00, "count": 4821 },
        { "name": "category", "type": "string",
          "values": ["electronics","furniture","clothing"],
          "count": 4821 },
        { "name": "rating", "type": "number",
          "min": 1.0, "max": 5.0, "count": 3104 }
      ]
    },
    {
      "name": "ORDER",
      "count": 12304,
      "properties": [
        { "name": "total", "type": "number",
          "min": 9.99, "max": 8750.00, "count": 12304 },
        { "name": "status", "type": "string",
          "values": ["pending","shipped","delivered","cancelled"] }
      ]
    }
  ],
  "relationships": [
    { "from": "CUSTOMER", "type": "PLACED",
      "to": "ORDER", "cardinality": "one-to-many" },
    { "from": "ORDER", "type": "CONTAINS",
      "to": "PRODUCT", "cardinality": "one-to-many" }
  ]
}`

const TS_SNIPPET = `// Called once at agent session start
const schema = await db.ai.ontology({ format: "md" })

// Inject into system prompt
systemPrompt += \`\\n\\nDatabase schema:\\n\${schema}\`

// Agent now constructs all queries against real structure`

const PY_SNIPPET = `# Called once at agent session start
schema = db.ai.ontology(format="md")

# Inject into system prompt
system_prompt += f"\\n\\nDatabase schema:\\n{schema}"

# Agent now constructs all queries against real structure`

const SHELL_SNIPPET = `# Called once at agent session start
curl -X GET "https://api.rushdb.com/api/v1/ai/ontology/md" \\
  -H "Authorization: Bearer $RUSHDB_API_KEY" \\
  -H "Accept: text/markdown"

# Pipe into your prompt template
SCHEMA=$(curl -s ...)
echo "Database schema:\\n\${SCHEMA}" >> system_prompt.txt`

const OUTCOME_CARDS = [
  {
    label: 'Valid filter ranges',
    body: "Agent knows price ranges from $9.99 to $2,499. It won't filter for price > $10,000 and return empty results."
  },
  {
    label: 'Real property names',
    body: 'Agent knows the field is "category", not "type" or "productType". No hallucinated field names. No silent query failures.'
  },
  {
    label: 'Traversable relationships',
    body: 'Agent knows CUSTOMER → ORDER → PRODUCT is the real topology. Multi-hop queries built with confidence, not guesswork.'
  }
]

const BEFORE_AFTER = [
  {
    before: 'Agent filters price > 50,000 — 0 results, no explanation',
    after: 'Agent knows max price is $2,499 — constructs meaningful range query'
  },
  {
    before: "Agent queries productType field — doesn't exist, silent fail",
    after: 'Agent queries category — real field, real values'
  },
  {
    before: "Agent traverses USER → ITEM — relationship doesn't exist",
    after: 'Agent traverses CUSTOMER → ORDER → PRODUCT — exact topology'
  }
]

export function LPOntologySection() {
  const [fmt, setFmt] = useState<OntologyFormat>('md')
  const [lang, setLang] = useState<CodeLang>('typescript')

  return (
    <LPSection surface borderTop>
      <LPContainer>
        <LPEyebrow>Ontology API</LPEyebrow>
        <LPSectionHeading className="mb-4">
          Agents that know your data
          <br />
          before they query it.
        </LPSectionHeading>

        {/* Subline */}
        <motion.p
          {...fadeUp(0.1)}
          className="text-lp-muted mb-16 max-w-2xl font-sans text-base leading-relaxed"
        >
          RushDB builds a live ontology from every record you push. Agents read it once per session — and
          never construct a query blind again.
        </motion.p>

        {/* Two-panel grid */}
        <div className="mb-16 grid grid-cols-2 gap-6 md:grid-cols-1">
          {/* Left — ontology response */}
          <motion.div
            initial={{ opacity: 0, x: -20 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.45, ease: 'easeOut' }}
            className="border-lp-border flex flex-col overflow-hidden border"
          >
            {/* Tab bar */}
            <div className="border-lp-border flex gap-1 border-b px-3 py-2">
              {(['md', 'json'] as OntologyFormat[]).map((f) => (
                <LPTabBtn key={f} active={fmt === f} onClick={() => setFmt(f)}>
                  {f === 'md' ? 'Markdown' : 'JSON'}
                </LPTabBtn>
              ))}
            </div>
            <div
              className="border-lp-border flex items-center gap-2 border-b px-4 py-2 font-mono text-sm"
              style={{ color: 'var(--lp-accent)', background: 'rgba(var(--lp-accent-rgb), 0.06)' }}
            >
              GET /api/v1/ai/ontology/{fmt === 'md' ? 'md' : 'json'}
            </div>
            <CodeBlock
              code={fmt === 'md' ? MD_EXAMPLE : JSON_EXAMPLE}
              language={fmt === 'md' ? 'markdown' : 'json'}
              className="text-xs"
            />
          </motion.div>

          {/* Right — what agent now knows */}
          <motion.div
            initial={{ opacity: 0, x: 20 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.45, delay: 0.15, ease: 'easeOut' }}
            className="flex flex-col gap-4"
          >
            <p className="text-lp-muted mb-2 font-mono text-sm uppercase tracking-widest">
              What the agent now knows
            </p>

            {OUTCOME_CARDS.map(({ label, body }, i) => (
              <LPAccentCard key={label} label={label} body={body} delay={0.2 + i * 0.1} />
            ))}
          </motion.div>
        </div>

        {/* API call strip */}
        <motion.div {...fadeUp(0.1)} className="border-lp-border mb-16 overflow-hidden border">
          {/* Tab bar */}
          <div className="border-lp-border flex gap-1 border-b px-3 py-2">
            {(['typescript', 'python', 'shell'] as CodeLang[]).map((l) => (
              <LPTabBtn key={l} active={lang === l} onClick={() => setLang(l)}>
                <LangIcon lang={l} size={14} className="inline-block shrink-0" />
                {l}
              </LPTabBtn>
            ))}
          </div>
          <CodeBlock
            code={
              lang === 'typescript' ? TS_SNIPPET
              : lang === 'python' ?
                PY_SNIPPET
              : SHELL_SNIPPET
            }
            language={lang === 'shell' ? 'bash' : lang}
            className="text-xs"
          />
          <div className="border-lp-border border-t px-4 py-3">
            <p className="text-lp-muted font-mono text-sm">
              One call. One session. <span className="text-lp-accent font-bold">No hallucinated schema.</span>
            </p>
          </div>
        </motion.div>

        {/* Before / After table */}
        <motion.div {...fadeUp(0.1)} className="mb-10">
          <div className="border-lp-border grid grid-cols-[1fr_auto_1fr] gap-0 border md:grid-cols-1">
            {/* Header row */}
            <div
              className="border-lp-border border-b px-4 py-3 font-mono text-sm"
              style={{ color: '#ff6b6b', background: 'rgba(255,68,68,0.06)' }}
            >
              Before
            </div>
            <div className="border-lp-border text-lp-muted border-b border-l border-r px-3 py-3 text-center font-mono text-sm md:hidden">
              →
            </div>
            <div
              className="border-lp-border border-b border-l px-4 py-3 font-mono text-sm md:border-l-0"
              style={{ color: 'var(--lp-accent)', background: 'rgba(var(--lp-accent-rgb), 0.06)' }}
            >
              After
            </div>

            {/* Data rows */}
            {BEFORE_AFTER.map(({ before, after }, i) => (
              <>
                <motion.div
                  key={`before-${i}`}
                  initial={{ opacity: 0 }}
                  whileInView={{ opacity: 1 }}
                  viewport={{ once: true }}
                  transition={{ duration: 0.3, delay: i * 0.08 }}
                  className="border-lp-border text-lp-muted px-4 py-4 font-sans text-sm leading-relaxed"
                  style={{ borderTop: i > 0 ? '1px solid var(--lp-border)' : undefined }}
                >
                  {before}
                </motion.div>
                <div
                  key={`arrow-${i}`}
                  className="border-lp-border text-lp-muted border-l border-r px-3 py-4 text-center font-mono text-sm md:hidden"
                  style={{ borderTop: i > 0 ? '1px solid var(--lp-border)' : undefined }}
                >
                  →
                </div>
                <motion.div
                  key={`after-${i}`}
                  initial={{ opacity: 0 }}
                  whileInView={{ opacity: 1 }}
                  viewport={{ once: true }}
                  transition={{ duration: 0.3, delay: i * 0.08 + 0.1 }}
                  className="border-lp-border text-lp-text border-l px-4 py-4 font-sans text-sm leading-relaxed md:border-l-0"
                  style={{ borderTop: i > 0 ? '1px solid var(--lp-border)' : undefined }}
                >
                  {after}
                </motion.div>
              </>
            ))}
          </div>
        </motion.div>

        {/* Tagline */}
        <motion.p
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.5, delay: 0.2 }}
          className="text-lp-accent text-center font-mono text-sm"
        >
          Fewer empty results. Fewer hallucinations. Agents that explain themselves.
        </motion.p>
      </LPContainer>
    </LPSection>
  )
}
