import type { ReactNode } from 'react'

import { useStore } from '@nanostores/react'
import { Bot, Brain, Cable, Code2, ExternalLink, Info, KeyRound, Terminal } from 'lucide-react'

import { Button, CopyButton } from '~/elements/Button'
import { CodeEditorSnippet } from '~/elements/CodeEditorSnippet'
import { CopyInput } from '~/elements/Input'
import { Tab, Tabs, TabsContent, TabsList } from '~/elements/Tabs'
import { $settings } from '~/features/auth/stores/settings'
import { SelectSdkLanguage } from '~/features/onboarding/components/SelectSdkLanguage'
import { docsUrls } from '~/features/onboarding/constants'
import type { AvailableSdkLanguage } from '~/features/onboarding/types'
import { useProjectTokensQuery } from '~/features/projects/hooks/useProjectQueries'
import type { Project } from '~/features/projects/types'
import { useWorkspaceProjectsQuery } from '~/features/workspaces/hooks/useWorkspaceQueries'
import { getRoutePath } from '~/lib/router'
import { cn } from '~/lib/utils'

import { AGENT_SETUP_URL, HOSTED_MCP_URL, connectDocsUrls } from './constants'

type ConnectPath = 'hosted-mcp' | 'local-mcp' | 'sdk-rest' | 'agent-skills'

type ConnectGuideProps = {
  className?: string
  defaultPath?: ConnectPath
  projectId?: Project['id']
}

const TOKEN_PLACEHOLDER = 'RUSHDB_API_KEY'

const connectPaths: Array<{ description: string; icon: ReactNode; label: string; value: ConnectPath }> = [
  {
    value: 'hosted-mcp',
    label: 'Hosted MCP',
    description: 'ChatGPT, Claude.ai',
    icon: <Bot />
  },
  {
    value: 'local-mcp',
    label: 'Local MCP',
    description: 'Cursor, Claude Desktop, VS Code',
    icon: <Terminal />
  },
  {
    value: 'sdk-rest',
    label: 'SDK / REST',
    description: 'Application code',
    icon: <Code2 />
  },
  {
    value: 'agent-skills',
    label: 'Agent Skills',
    description: 'Querying and memory workflows',
    icon: <Brain />
  }
]

const localMcpConfig = (token: string) => `{
  "mcpServers": {
    "rushdb": {
      "command": "npx",
      "args": ["-y", "@rushdb/mcp-server"],
      "env": {
        "RUSHDB_API_KEY": "${token}"
      }
    }
  }
}`

const vscodeMcpConfig = (token: string) => `{
  "servers": {
    "rushdb": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@rushdb/mcp-server"],
      "env": {
        "RUSHDB_API_KEY": "${token}"
      }
    }
  }
}`

const sdkLanguageLabels: Record<AvailableSdkLanguage, string> = {
  python: 'Python SDK',
  shell: 'REST API',
  typescript: 'TypeScript SDK'
}

const getSdkInstallCommand = (language: AvailableSdkLanguage) =>
  ({
    python: 'pip install rushdb',
    shell: null,
    typescript: 'npm install @rushdb/javascript-sdk'
  })[language]

const getSdkConnectionSnippet = ({ language, token }: { language: AvailableSdkLanguage; token: string }) => {
  const typescript = `import RushDB from '@rushdb/javascript-sdk'

const db = new RushDB('${token}')

// Store an agent memory episode
await db.records.importJson({
  label: 'MEMORY',
  data: [
    {
      agentId: 'researcher-1',
      sessionId: 'sess-abc123',
      content: 'User prefers concise answers with code examples.',
      type: 'preference',
      createdAt: new Date().toISOString()
    }
  ]
})

// Recall relevant memories for the current agent session
await db.records.find({
  labels: ['MEMORY'],
  where: {
    agentId: { $eq: 'researcher-1' },
    type: { $eq: 'preference' }
  }
})`

  const python = `from rushdb import RushDB
from datetime import datetime, timezone

db = RushDB("${token}")

# Store an agent memory episode
db.records.create_many("MEMORY", [
    {
        "agentId": "researcher-1",
        "sessionId": "sess-abc123",
        "content": "User prefers concise answers with code examples.",
        "type": "preference",
        "createdAt": datetime.now(timezone.utc).isoformat()
    }
])

# Recall relevant memories for the current agent session
db.records.find({
    "labels": ["MEMORY"],
    "where": {
        "agentId": { "$eq": "researcher-1" },
        "type": { "$eq": "preference" }
    }
})`

  const shell = `# Store an agent memory episode
curl -X POST 'https://api.rushdb.com/api/v1/records/import/json' \\
  -H 'accept: */*' \\
  -H 'Authorization: Bearer ${token}' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "label": "MEMORY",
    "data": [
      {
        "agentId": "researcher-1",
        "sessionId": "sess-abc123",
        "content": "User prefers concise answers with code examples.",
        "type": "preference",
        "createdAt": "2025-01-01T00:00:00Z"
      }
    ]
  }'

# Recall relevant memories for the current agent session
curl -X POST 'https://api.rushdb.com/api/v1/records/search' \\
  -H 'accept: */*' \\
  -H 'Authorization: Bearer ${token}' \\
  -H 'Content-Type: application/json' \\
  -d '{
    "labels": ["MEMORY"],
    "where": {
      "agentId": { "$eq": "researcher-1" },
      "type": { "$eq": "preference" }
    }
  }'`

  return {
    python,
    shell,
    typescript
  }[language]
}

type CodeLanguage = 'json' | 'shell' | 'text' | 'typescript'

type SyntaxTone =
  | 'command'
  | 'comment'
  | 'function'
  | 'keyword'
  | 'number'
  | 'operator'
  | 'property'
  | 'string'
  | 'variable'

type SyntaxToken = {
  text: string
  tone?: SyntaxTone
}

const syntaxToneClass: Record<SyntaxTone, string> = {
  command: 'text-accent',
  comment: 'text-content3',
  function: 'text-success',
  keyword: 'text-accent',
  number: 'text-warning',
  operator: 'text-content3',
  property: 'text-badge-blue',
  string: 'text-badge-green',
  variable: 'text-warning'
}

const languageLabels: Record<CodeLanguage, string> = {
  json: 'JSON',
  shell: 'Shell',
  text: 'Text',
  typescript: 'TypeScript'
}

function normalizeCodeLanguage(language?: string): CodeLanguage {
  switch (language) {
    case 'bash':
    case 'sh':
    case 'shell':
      return 'shell'
    case 'json':
      return 'json'
    case 'ts':
    case 'typescript':
      return 'typescript'
    default:
      return 'text'
  }
}

function tokenizeLine(
  line: string,
  pattern: RegExp,
  classify: (match: string, index: number, line: string) => SyntaxTone | undefined
) {
  const tokens: Array<SyntaxToken> = []
  let cursor = 0

  pattern.lastIndex = 0

  for (const match of line.matchAll(pattern)) {
    const text = match[0]
    const index = match.index ?? 0

    if (index > cursor) {
      tokens.push({ text: line.slice(cursor, index) })
    }

    tokens.push({ text, tone: classify(text, index, line) })
    cursor = index + text.length
  }

  if (cursor < line.length) {
    tokens.push({ text: line.slice(cursor) })
  }

  return tokens
}

function highlightJsonLine(line: string) {
  return tokenizeLine(
    line,
    /"(?:\\.|[^"\\])*"|true|false|null|-?\b\d+(?:\.\d+)?\b|[{}[\],:]/g,
    (match, index, source) => {
      if (match.startsWith('"')) {
        return /^\s*:/.test(source.slice(index + match.length)) ? 'property' : 'string'
      }

      if (/^(true|false|null)$/.test(match)) {
        return 'keyword'
      }

      if (/^-?\d/.test(match)) {
        return 'number'
      }

      return 'operator'
    }
  )
}

function highlightTypeScriptLine(line: string) {
  return tokenizeLine(
    line,
    /\/\/.*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b(?:async|await|class|const|default|export|extends|false|from|function|import|interface|let|new|null|return|true|type|undefined|var)\b|\b[A-Za-z_$][\w$]*(?=\()|\b\d+(?:\.\d+)?\b|[{}[\]().,:;]/g,
    (match) => {
      if (match.startsWith('//')) {
        return 'comment'
      }

      if (/^["'`]/.test(match)) {
        return 'string'
      }

      if (
        /^(async|await|class|const|default|export|extends|false|from|function|import|interface|let|new|null|return|true|type|undefined|var)$/.test(
          match
        )
      ) {
        return 'keyword'
      }

      if (/^-?\d/.test(match)) {
        return 'number'
      }

      if (/^[A-Za-z_$]/.test(match)) {
        return 'function'
      }

      return 'operator'
    }
  )
}

function highlightShellLine(line: string) {
  return tokenizeLine(
    line,
    /#.*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|https?:\/\/[^\s\\]+|\$\{?[A-Za-z_][A-Za-z0-9_]*\}?|\b[A-Z_][A-Z0-9_]*\b|--?[A-Za-z][A-Za-z-]*|\b(?:curl|node|npm|npx|pnpm|python|uv|yarn)\b|\\|[{}[\](),:]/g,
    (match) => {
      if (match.startsWith('#')) {
        return 'comment'
      }

      if (/^["']/.test(match)) {
        return 'string'
      }

      if (/^https?:\/\//.test(match)) {
        return 'property'
      }

      if (/^(curl|node|npm|npx|pnpm|python|uv|yarn)$/.test(match)) {
        return 'command'
      }

      if (/^(--?|[$A-Z_])/.test(match)) {
        return 'variable'
      }

      return 'operator'
    }
  )
}

function highlightCodeLine(line: string, language: CodeLanguage) {
  switch (language) {
    case 'json':
      return highlightJsonLine(line)
    case 'shell':
      return highlightShellLine(line)
    case 'typescript':
      return highlightTypeScriptLine(line)
    default:
      return [{ text: line }]
  }
}

function HighlightedCode({ code, language }: { code: string; language: CodeLanguage }) {
  return (
    <>
      {code.split('\n').map((line, lineIndex) => (
        <span className="block min-h-5" key={`${lineIndex}-${line}`}>
          {highlightCodeLine(line, language).map((token, tokenIndex) => (
            <span
              className={token.tone ? syntaxToneClass[token.tone] : undefined}
              key={`${lineIndex}-${tokenIndex}-${token.text}`}
            >
              {token.text}
            </span>
          ))}
        </span>
      ))}
    </>
  )
}

function StepList({ items }: { items: Array<ReactNode> }) {
  return (
    <ol className="grid gap-3">
      {items.map((item, index) => (
        <li className="grid grid-cols-[2rem_1fr] gap-3" key={index}>
          <span className="bg-accent/20 text-content grid h-8 w-8 place-items-center rounded-full text-sm font-medium">
            {index + 1}
          </span>
          <div className="text-content2 min-w-0 pt-1 text-sm leading-6">{item}</div>
        </li>
      ))}
    </ol>
  )
}

function GuideHeader({
  children,
  description,
  icon,
  title
}: {
  children?: ReactNode
  description: ReactNode
  icon: ReactNode
  title: ReactNode
}) {
  return (
    <div className="flex flex-col gap-4 border-b pb-5 md:flex-row md:items-start md:justify-between">
      <div className="flex min-w-0 gap-3">
        <div className="bg-secondary text-content2 grid h-10 w-10 shrink-0 place-items-center rounded-md [&>svg]:h-5 [&>svg]:w-5">
          {icon}
        </div>
        <div className="min-w-0">
          <h3 className="text-content text-lg font-semibold">{title}</h3>
          <p className="text-content2 mt-1 max-w-2xl text-sm leading-6">{description}</p>
        </div>
      </div>
      {children && <div className="flex shrink-0 flex-wrap gap-2">{children}</div>}
    </div>
  )
}

function CodeSnippet({ code, language, title }: { code: string; language?: string; title: ReactNode }) {
  const normalizedLanguage = normalizeCodeLanguage(language)

  return (
    <div className="bg-fill overflow-hidden rounded-md border">
      <div className="flex items-center justify-between gap-3 border-b px-3 py-2">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <span className="text-content2 text-sm font-medium">{title}</span>
          {language && (
            <span className="text-content3 rounded border px-1.5 py-0.5 text-[10px] font-medium uppercase leading-none">
              {languageLabels[normalizedLanguage]}
            </span>
          )}
        </div>
        <CopyButton text={code} size="xsmall" variant="outline">
          Copy
        </CopyButton>
      </div>
      <pre className="text-content max-h-80 overflow-auto p-3 text-xs leading-5">
        <code className="block min-w-max whitespace-pre font-mono">
          <HighlightedCode code={code} language={normalizedLanguage} />
        </code>
      </pre>
    </div>
  )
}

function GuidePanel({ children }: { children: ReactNode }) {
  return <section className="grid gap-5 rounded-md border p-5">{children}</section>
}

function WorkspaceProjectNotice() {
  return (
    <div className="border-badge-blue/20 bg-badge-blue/10 flex flex-col gap-3 rounded-md border p-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex gap-3">
        <Info className="text-badge-blue mt-0.5 h-5 w-5 shrink-0" />
        <p className="text-badge-blue text-sm leading-6">
          Every RushDB integration — MCP, SDK, and REST — requires a project API key. API keys are issued per
          project, so create a project first.
        </p>
      </div>
      <Button as="a" href={getRoutePath('newProject')} size="small" variant="primary">
        Create project
      </Button>
    </div>
  )
}

function ProjectTokenNotice({ projectId, tokenReady }: { projectId: Project['id']; tokenReady: boolean }) {
  if (tokenReady) {
    return (
      <p className="text-content2 text-sm">
        The snippets below use this project&apos;s first API key. Rotate or create keys from the API Keys page
        when needed.
      </p>
    )
  }

  return (
    <div className="flex flex-col gap-3 rounded-md border border-dashed p-4 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex gap-3">
        <KeyRound className="text-content2 mt-0.5 h-5 w-5 shrink-0" />
        <p className="text-content2 text-sm leading-6">
          This project needs an API key before local MCP and SDK snippets can use project-specific
          configuration.
        </p>
      </div>
      <Button as="a" href={getRoutePath('projectTokens', { id: projectId })} size="small" variant="secondary">
        API Keys
      </Button>
    </div>
  )
}

function HostedMcpGuide() {
  return (
    <GuidePanel>
      <GuideHeader
        icon={<Bot />}
        title="Connect a web assistant"
        description="Use the hosted MCP connector for ChatGPT, Claude.ai, and other web assistants. No local install or API key is required."
      >
        <Button
          as="a"
          href={connectDocsUrls.mcp}
          rel="noopener noreferrer"
          size="small"
          target="_blank"
          variant="outline"
        >
          Docs
          <ExternalLink />
        </Button>
      </GuideHeader>

      <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(18rem,24rem)]">
        <StepList
          items={[
            'Open the assistant connector or integration settings.',
            'Add the RushDB MCP endpoint, then sign in with your RushDB account.',
            'Choose the project to expose to the assistant.',
            'Ask the assistant to call getOntologyMarkdown and show the project labels.'
          ]}
        />

        <div className="grid content-start gap-3">
          <div>
            <p className="text-content2 mb-2 text-sm font-medium">MCP endpoint</p>
            <CopyInput value={HOSTED_MCP_URL} />
          </div>
          <CodeSnippet
            title="Verification prompt"
            code="Call getOntologyMarkdown and show me what labels exist in my RushDB project."
          />
        </div>
      </div>
    </GuidePanel>
  )
}

function LocalMcpGuide({ projectId, token }: { projectId?: Project['id']; token: string | undefined }) {
  const apiKey = token ?? TOKEN_PLACEHOLDER

  return (
    <GuidePanel>
      <GuideHeader
        icon={<Terminal />}
        title="Connect local coding tools"
        description="Run the RushDB MCP server from Cursor, Claude Desktop, VS Code, or any MCP client that supports stdio servers."
      >
        <Button
          as="a"
          href={connectDocsUrls.mcp}
          rel="noopener noreferrer"
          size="small"
          target="_blank"
          variant="outline"
        >
          Docs
          <ExternalLink />
        </Button>
      </GuideHeader>

      {projectId && <ProjectTokenNotice projectId={projectId} tokenReady={Boolean(token)} />}

      <div className="grid gap-4 xl:grid-cols-2">
        <CodeSnippet title="Claude Desktop / Cursor" code={localMcpConfig(apiKey)} language="json" />
        <CodeSnippet title="VS Code MCP" code={vscodeMcpConfig(apiKey)} language="json" />
      </div>

      <StepList
        items={[
          'Paste the matching block into your MCP client configuration.',
          'Restart the client so the RushDB tools are discovered.',
          'Verify with getOntologyMarkdown before asking the agent to query or mutate records.'
        ]}
      />
    </GuidePanel>
  )
}

function SdkRestGuide({ projectId, token }: { projectId?: Project['id']; token: string | undefined }) {
  const apiKey = token ?? TOKEN_PLACEHOLDER
  const { sdkLanguage } = useStore($settings)
  const installCommand = getSdkInstallCommand(sdkLanguage)
  const code = getSdkConnectionSnippet({ language: sdkLanguage, token: apiKey })

  return (
    <GuidePanel>
      <GuideHeader
        icon={<Code2 />}
        title="Build with SDK or REST"
        description="Use the SDK or JSON API when RushDB is part of application code, ingestion jobs, or data pipelines."
      >
        <Button
          as="a"
          href={docsUrls.sdk[sdkLanguage].installation ?? connectDocsUrls.sdk}
          rel="noopener noreferrer"
          size="small"
          target="_blank"
          variant="outline"
        >
          Docs
          <ExternalLink />
        </Button>
      </GuideHeader>

      {projectId && <ProjectTokenNotice projectId={projectId} tokenReady={Boolean(token)} />}

      <div className="grid gap-4">
        <SelectSdkLanguage className="-ml-1" />

        {installCommand && (
          <div>
            <p className="text-content2 mb-2 text-sm font-medium">Install core SDK</p>
            <CopyInput value={installCommand} />
          </div>
        )}

        <CodeEditorSnippet title={sdkLanguageLabels[sdkLanguage]} code={code} language={sdkLanguage} />
      </div>

      {projectId && (
        <div className="flex flex-wrap gap-2">
          <Button
            as="a"
            href={getRoutePath('projectImportData', { id: projectId })}
            size="small"
            variant="secondary"
          >
            Import data
          </Button>
          <Button as="a" href={getRoutePath('project', { id: projectId })} size="small" variant="secondary">
            Records
          </Button>
        </div>
      )}
    </GuidePanel>
  )
}

function AgentSkillsGuide() {
  return (
    <GuidePanel>
      <GuideHeader
        icon={<Brain />}
        title="Teach agents the RushDB workflow"
        description="Install RushDB Agent Skills after MCP is connected so coding agents learn schema discovery, query building, data modeling, and persistent memory patterns."
      >
        <Button
          as="a"
          href={connectDocsUrls.skills}
          rel="noopener noreferrer"
          size="small"
          target="_blank"
          variant="outline"
        >
          Docs
          <ExternalLink />
        </Button>
      </GuideHeader>

      <div className="grid gap-4 xl:grid-cols-2">
        <CodeSnippet
          title="Install skills"
          code="npx skills add rush-db/rushdb --path packages/skills"
          language="shell"
        />
        <CodeSnippet
          title="Bootstrap memory"
          code={`Fetch ${AGENT_SETUP_URL} and follow the instructions exactly.`}
        />
      </div>

      <StepList
        items={[
          'Start a fresh agent session after installing skills.',
          'Ask the agent to call getOntologyMarkdown before any query.',
          'For persistent memory, give the agent the bootstrap prompt and let it validate recall.'
        ]}
      />
    </GuidePanel>
  )
}

export function ConnectGuide({ className, defaultPath = 'hosted-mcp', projectId }: ConnectGuideProps) {
  const { data: tokens, isPending } = useProjectTokensQuery()
  const { data: projects, isPending: projectsPending } = useWorkspaceProjectsQuery()
  const token = projectId ? tokens?.[0]?.value : undefined
  const showWorkspaceProjectNotice = !projectId && !projectsPending && projects?.length === 0

  return (
    <section className={cn('grid gap-5', className)}>
      {showWorkspaceProjectNotice && <WorkspaceProjectNotice />}

      <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
        <div>
          <h2 className="text-content text-2xl font-bold">Connect RushDB</h2>
          <p className="text-content2 mt-2 max-w-2xl leading-6">
            Start from the path that matches your workflow. Each setup gets to a working connection first,
            then points to the next useful action.
          </p>
        </div>
        <Button
          as="a"
          href={connectDocsUrls.overview}
          rel="noopener noreferrer"
          size="small"
          target="_blank"
          variant="outline"
        >
          Full guide
          <ExternalLink />
        </Button>
      </div>

      <Tabs defaultValue={defaultPath} className="grid gap-4">
        <TabsList className="max-w-full" data-tour="project-getting-started-finish">
          {connectPaths.map((path) => (
            <Tab key={path.value} value={path.value} className="h-auto min-h-11 py-2">
              {path.icon}
              <span className="flex flex-col items-start">
                <span>{path.label}</span>
                <span className="text-content2 text-sm font-normal">{path.description}</span>
              </span>
            </Tab>
          ))}
        </TabsList>

        <TabsContent value="hosted-mcp">
          <HostedMcpGuide />
        </TabsContent>
        <TabsContent value="local-mcp">
          <LocalMcpGuide projectId={projectId} token={isPending ? undefined : token} />
        </TabsContent>
        <TabsContent value="sdk-rest">
          <SdkRestGuide projectId={projectId} token={isPending ? undefined : token} />
        </TabsContent>
        <TabsContent value="agent-skills">
          <AgentSkillsGuide />
        </TabsContent>
      </Tabs>
    </section>
  )
}
