import type { Config } from '@docusaurus/types'
import type * as Preset from '@docusaurus/preset-classic'
import tailwindPlugin from './plugins/tailwind-config.cjs'
import * as path from 'node:path'
import * as fs from 'node:fs'

const CopyWebpackPlugin = require('copy-webpack-plugin')

// Helper function to generate categorized content files
async function generateCategorizedContentFiles(
  llmsDir: string,
  allMdx: string[],
  currentVersionDocsRoutes: Record<string, Record<string, unknown>>,
  filePathToContent: Map<string, string>
) {
  // Create mapping from file content to routes to categorize the full content
  const categories = {
    learn: [] as string[],
    connect: [] as string[],
    deploy: [] as string[],
    'rushdb-cloud': [] as string[],
    'get-started': [] as string[]
  }

  // Group content by file paths directly
  for (const [filePath, content] of filePathToContent.entries()) {
    // Categorize by file path prefix
    if (filePath.startsWith('learn/')) {
      categories.learn.push(content)
    } else if (filePath.startsWith('connect/')) {
      categories.connect.push(content)
    } else if (filePath.startsWith('deploy/')) {
      categories.deploy.push(content)
    } else if (filePath.startsWith('rushdb-cloud/')) {
      categories['rushdb-cloud'].push(content)
    } else if (filePath.startsWith('get-started/')) {
      categories['get-started'].push(content)
    }
  }

  // Generate full content files for each category
  for (const [categoryName, categoryContent] of Object.entries(categories)) {
    if (categoryContent.length > 0) {
      const categoryContentPath = path.join(llmsDir, `llms-${categoryName}-full.txt`)
      const combinedContent = categoryContent.join('\n\n---\n\n')

      try {
        await fs.promises.writeFile(categoryContentPath, combinedContent)
      } catch (err) {
        console.error(`Error writing ${categoryName} full content file:`, err)
      }
    }
  }
}

// Light code theme — matches website's lightOverride (materialLight-based warm palette)
const lightCodeTheme = {
  plain: {
    color: '#3a3530',
    backgroundColor: '#F5F2EB'
  },
  styles: [
    {
      types: ['comment', 'prolog', 'doctype', 'cdata'],
      style: { color: '#9a8f85', fontStyle: 'italic' as const }
    },
    {
      types: ['punctuation'],
      style: { color: '#6b6560' }
    },
    {
      types: ['property', 'tag', 'deleted', 'constant', 'symbol'],
      style: { color: '#b45309' }
    },
    {
      types: ['boolean', 'number'],
      style: { color: '#b45309' }
    },
    {
      types: ['selector', 'attr-name', 'string', 'char', 'builtin', 'inserted'],
      style: { color: '#C8540A' }
    },
    {
      types: ['operator', 'entity', 'url', 'variable'],
      style: { color: '#8b5c3a' }
    },
    {
      types: ['atrule', 'attr-value', 'keyword'],
      style: { color: '#7c3aed' }
    },
    {
      types: ['function', 'class-name'],
      style: { color: '#0369a1' }
    },
    {
      types: ['regex', 'important'],
      style: { color: '#b45309' }
    }
  ]
}

// Dark code theme — matches website's darkOverride (materialDark-based)
const darkCodeTheme = {
  plain: {
    color: 'rgb(195, 206, 227)',
    backgroundColor: '#111113'
  },
  styles: [
    {
      types: ['comment', 'prolog', 'doctype', 'cdata'],
      style: { color: '#546E7A', fontStyle: 'italic' as const }
    },
    {
      types: ['punctuation'],
      style: { color: '#89DDFF' }
    },
    {
      types: ['property', 'tag', 'boolean', 'number', 'constant', 'symbol', 'deleted'],
      style: { color: '#F77669' }
    },
    {
      types: ['selector', 'attr-name', 'string', 'char', 'builtin', 'inserted'],
      style: { color: '#C3E88D' }
    },
    {
      types: ['operator', 'entity', 'url', 'variable'],
      style: { color: '#EEFFFF' }
    },
    {
      types: ['atrule', 'attr-value', 'keyword'],
      style: { color: '#C792EA' }
    },
    {
      types: ['function', 'class-name'],
      style: { color: '#82AAFF' }
    },
    {
      types: ['regex', 'important'],
      style: { color: '#F07178' }
    }
  ]
}

const config: Config = {
  markdown: {
    mermaid: true
  },
  themes: [
    '@docusaurus/theme-mermaid',
    [
      require.resolve('@easyops-cn/docusaurus-search-local'),
      /** @type {import('@easyops-cn/docusaurus-search-local').PluginOptions} */
      {
        hashed: true,
        docsRouteBasePath: '/',
        searchBarShortcutHint: false
      }
    ]
  ],
  title: 'RushDB Docs',
  organizationName: 'Collect Software Inc',
  projectName: 'RushDB Docs',
  tagline: 'Instant Database for Modern Apps & AI Era',
  favicon: 'img/favicon.svg',

  // Set the production url of your site here
  url: 'https://docs.rushdb.com',
  // Set the /<baseUrl>/ pathname under which your site is served
  // For GitHub pages deployment, it is often '/<projectName>/'
  baseUrl: '/',

  // Serve raw .mdx/.md source files at their natural paths (e.g. /deploy/infra-neo4j.mdx)
  // so CopyPageButton can fetch content locally without depending on GitHub raw.
  staticDirectories: ['static', 'docs'],

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  // Even if you don't use internalization, you can use this field to set useful
  // metadata like html lang. For example, if your site is Chinese, you may want
  // to replace "en" with "zh-Hans".
  i18n: {
    defaultLocale: 'en',
    locales: ['en']
  },

  plugins: [
    tailwindPlugin,
    require('./plugins/tutorials-data.cjs'),
    async function pluginRawDocs(context) {
      return {
        name: 'raw-docs-plugin',
        configureWebpack(_config, isServer) {
          if (isServer) return {}

          return {
            plugins: [
              new CopyWebpackPlugin({
                patterns: [
                  {
                    from: path.join(context.siteDir, 'docs'),
                    to: 'raw-docs',
                    globOptions: {
                      ignore: ['**/_category_.json']
                    }
                  }
                ]
              })
            ]
          }
        }
      }
    },
    [
      '@docusaurus/plugin-client-redirects',
      {
        redirects: [
          { from: '/', to: '/get-started/quick-tutorial' },
          // Legacy concepts now live in the curated Learn sections.
          { from: '/basic-concepts/properties', to: '/build/schema/labels-and-properties' },
          { from: '/basic-concepts/records', to: '/build/data/store-records' },
          { from: '/basic-concepts/relations', to: '/build/graph/' },
          { from: '/basic-concepts/transactions', to: '/build/reliability/transactions' },
          { from: '/advanced/properties', to: '/build/schema/labels-and-properties' },
          { from: '/advanced/data-types', to: '/build/data/labeled-meta-property-graph' },
          { from: '/advanced/records', to: '/build/data/store-records' },
          { from: '/advanced/relationships', to: '/build/graph/' },
          { from: '/advanced/querying-data', to: '/reference/search-query' },
          { from: '/advanced/search-aggregation', to: '/reference/select-expressions' },
          { from: '/advanced/enhanced-typescript', to: '/reference/typescript/' },
          { from: '/concepts', to: '/build/data/labeled-meta-property-graph' },
          { from: '/concepts/bring-your-own-vectors', to: '/build/ai-search/bring-your-own-vectors' },
          { from: '/concepts/data-ingestion', to: '/build/data/import-data' },
          { from: '/concepts/labels', to: '/build/schema/labels-and-properties' },
          { from: '/concepts/properties', to: '/build/schema/labels-and-properties' },
          { from: '/concepts/records', to: '/build/data/store-records' },
          { from: '/concepts/relationships', to: '/build/graph/' },
          { from: '/concepts/search', to: '/reference/search-query' },
          { from: '/concepts/search/introduction', to: '/reference/search-query' },
          { from: '/concepts/search/labels', to: '/reference/search-labels' },
          { from: '/concepts/search/pagination-order', to: '/reference/pagination-order' },
          { from: '/concepts/search/select', to: '/reference/select-expressions' },
          { from: '/concepts/search/group-by', to: '/reference/group-by' },
          { from: '/concepts/search/where', to: '/reference/where-operators' },
          { from: '/concepts/semantic-search', to: '/build/ai-search/semantic-search' },
          { from: '/concepts/storage', to: '/build/data/labeled-meta-property-graph' },
          { from: '/concepts/transactions', to: '/build/reliability/transactions' },
          { from: '/concepts/billing-model', to: '/cloud/billing-model' },
          { from: '/concepts/knowledge-units', to: '/cloud/knowledge-units' },
          { from: '/pricing-and-billing/billing-model', to: '/cloud/billing-model' },
          { from: '/pricing-and-billing/knowledge-units', to: '/cloud/knowledge-units' },
          { from: '/build/models', to: '/reference/typescript/Model' },
          { from: '/deploy/local-setup', to: '/deploy/local-docker' },
          { from: '/deploy/dashboard-configuration', to: '/deploy/get-api-key' },
          { from: '/get-started/get-api-key', to: '/deploy/get-api-key' },
          // Moved deployment tutorials.
          { from: '/tutorials/configuring-dashboard', to: '/deploy/get-api-key' },
          { from: '/tutorials/connect-aura-instance', to: '/deploy/connect-aura' },
          { from: '/tutorials/deployment', to: '/deploy/docker-remote' },
          { from: '/tutorials/local-setup', to: '/deploy/local-docker' },
          { from: '/tutorials/mcp-operator-quickstart', to: '/deploy/mcp-operator-quickstart' },
          { from: '/tutorials/self-hosted-project-setup', to: '/deploy/self-hosted-project-setup' },
          // The standalone MCP guide has been consolidated into Connect.
          { from: '/mcp-server/introduction', to: '/connect/mcp' },
          { from: '/mcp-server/quickstart', to: '/connect/mcp' },
          { from: '/mcp-server/configuration', to: '/connect/mcp' },
          { from: '/mcp-server/tools', to: '/connect/mcp' },
          { from: '/mcp-server/examples', to: '/connect/mcp' },
          { from: '/mcp-server/troubleshooting', to: '/connect/mcp' },
          // SDK guides are represented by Learn workflows and generated references.
          { from: '/typescript-sdk/introduction', to: '/reference/typescript/' },
          { from: '/typescript-sdk/labels', to: '/build/schema/labels-and-properties' },
          { from: '/typescript-sdk/properties', to: '/build/schema/labels-and-properties' },
          { from: '/typescript-sdk/models', to: '/reference/typescript/Model' },
          { from: '/typescript-sdk/raw-queries', to: '/build/raw-queries' },
          { from: '/typescript-sdk/relationships', to: '/build/graph/connect-records' },
          { from: '/typescript-sdk/transactions', to: '/build/reliability/transactions' },
          { from: '/typescript-sdk/records/create-records', to: '/build/data/store-records' },
          { from: '/typescript-sdk/records/get-records', to: '/build/data/find-and-query' },
          { from: '/typescript-sdk/records/update-records', to: '/build/data/store-records' },
          { from: '/typescript-sdk/records/delete-records', to: '/build/data/store-records' },
          { from: '/typescript-sdk/records/import-data', to: '/build/data/import-data' },
          { from: '/typescript-sdk/ai/overview', to: '/build/ai-search/semantic-search' },
          { from: '/typescript-sdk/ai/indexing', to: '/build/ai-search/manage-indexes' },
          { from: '/typescript-sdk/ai/advanced-indexing', to: '/build/ai-search/bring-your-own-vectors' },
          { from: '/typescript-sdk/ai/search', to: '/build/ai-search/semantic-search' },
          { from: '/typescript-sdk/ai/write-with-vectors', to: '/build/ai-search/write-with-vectors' },
          { from: '/typescript-sdk/typescript-reference/DBRecord', to: '/reference/typescript/DBRecord' },
          {
            from: '/typescript-sdk/typescript-reference/DBRecordInstance',
            to: '/reference/typescript/DBRecordInstance'
          },
          {
            from: '/typescript-sdk/typescript-reference/DBRecordTarget',
            to: '/reference/typescript/DBRecordTarget'
          },
          {
            from: '/typescript-sdk/typescript-reference/DBRecordsArrayInstance',
            to: '/reference/typescript/DBRecordsArrayInstance'
          },
          { from: '/typescript-sdk/typescript-reference/Model', to: '/reference/typescript/Model' },
          {
            from: '/typescript-sdk/typescript-reference/RelationTarget',
            to: '/reference/typescript/RelationTarget'
          },
          { from: '/typescript-sdk/typescript-reference/RushDB', to: '/reference/typescript/RushDB' },
          {
            from: '/typescript-sdk/typescript-reference/SearchQuery',
            to: '/reference/typescript/SearchQuery'
          },
          {
            from: '/typescript-sdk/typescript-reference/Transaction',
            to: '/reference/typescript/Transaction'
          },
          { from: '/python-sdk/introduction', to: '/reference/python/' },
          { from: '/python-sdk/labels', to: '/build/schema/labels-and-properties' },
          { from: '/python-sdk/properties', to: '/build/schema/labels-and-properties' },
          { from: '/python-sdk/raw-queries', to: '/build/raw-queries' },
          { from: '/python-sdk/relationships', to: '/build/graph/connect-records' },
          { from: '/python-sdk/transactions', to: '/build/reliability/transactions' },
          { from: '/python-sdk/records-api', to: '/build/data/store-records' },
          { from: '/python-sdk/records/create-records', to: '/build/data/store-records' },
          { from: '/python-sdk/records/get-records', to: '/build/data/find-and-query' },
          { from: '/python-sdk/records/update-records', to: '/build/data/store-records' },
          { from: '/python-sdk/records/delete-records', to: '/build/data/store-records' },
          { from: '/python-sdk/records/import-data', to: '/build/data/import-data' },
          { from: '/python-sdk/ai/overview', to: '/build/ai-search/semantic-search' },
          { from: '/python-sdk/ai/indexing', to: '/build/ai-search/manage-indexes' },
          { from: '/python-sdk/ai/advanced-indexing', to: '/build/ai-search/bring-your-own-vectors' },
          { from: '/python-sdk/ai/search', to: '/build/ai-search/semantic-search' },
          { from: '/python-sdk/ai/write-with-vectors', to: '/build/ai-search/write-with-vectors' },
          { from: '/python-sdk/python-reference/record', to: '/reference/python/record' },
          { from: '/python-sdk/python-reference/RushDB', to: '/reference/python/RushDB' },
          { from: '/python-sdk/python-reference/search-result', to: '/reference/python/search-result' },
          { from: '/python-sdk/python-reference/SearchQuery', to: '/reference/python/SearchQuery' },
          { from: '/python-sdk/python-reference/transaction', to: '/reference/python/transaction' }
        ]
      }
    ],
    ['@docusaurus/plugin-google-gtag', { trackingID: process.env.GTM_ID || 'GTM-XXXXX', anonymizeIP: true }],
    async function pluginLlmsTxt(context) {
      return {
        name: 'llms-txt-plugin',
        loadContent: async () => {
          const { siteDir } = context

          const contentDir = path.join(siteDir, 'docs')
          const allMdx: string[] = []
          const filePathToContent: Map<string, string> = new Map()

          // recursive function to get all mdx files
          const getMdxFiles = async (dir: string) => {
            const entries = await fs.promises.readdir(dir, { withFileTypes: true })

            for (const entry of entries) {
              const fullPath = path.join(dir, entry.name)
              if (entry.isDirectory()) {
                await getMdxFiles(fullPath)
              } else if (entry.name.endsWith('.mdx') || entry.name.endsWith('.md')) {
                let content = await fs.promises.readFile(fullPath, 'utf8')

                // Remove ALL front-matter (everything between --- and ---)
                content = content.replace(/^---\n[\s\S]*?\n---\n/, '')

                // Get relative path from docs directory for categorization
                const relativePath = path.relative(contentDir, fullPath)

                allMdx.push(content)
                filePathToContent.set(relativePath, content)
              }
            }
          }

          await getMdxFiles(contentDir)
          return { allMdx, filePathToContent }
        },
        postBuild: async ({ content, routes, outDir }) => {
          const { allMdx, filePathToContent } = content as {
            allMdx: string[]
            filePathToContent: Map<string, string>
          }

          // Create llms directory
          const llmsDir = path.join(outDir, '')
          await fs.promises.mkdir(llmsDir, { recursive: true })

          // Write concatenated MDX content
          const concatenatedPath = path.join(llmsDir, 'llms-full.txt')
          await fs.promises.writeFile(concatenatedPath, allMdx.join('\n\n---\n\n'))

          // we need to dig down several layers:
          // find PluginRouteConfig marked by plugin.name === "docusaurus-plugin-content-docs"
          const docsPluginRouteConfig = routes.filter(
            (route) => route.plugin.name === 'docusaurus-plugin-content-docs'
          )[0]

          // docsPluginRouteConfig has a routes property has a record with the path "/" that contains all docs routes.
          const allDocsRouteConfig = docsPluginRouteConfig.routes?.filter((route) => route.path === '/')[0]

          // A little type checking first
          if (!allDocsRouteConfig?.props?.version) {
            return
          }

          // this route config has a `props` property that contains the current documentation.
          const currentVersionDocsRoutes = (allDocsRouteConfig.props.version as Record<string, unknown>)
            .docs as Record<string, Record<string, unknown>>

          // Categorize docs by section
          const categories = {
            learn: [] as string[],
            connect: [] as string[],
            deploy: [] as string[],
            'rushdb-cloud': [] as string[],
            'get-started': [] as string[]
          }

          const docPermalinks = new Map<string, string>()
          try {
            const globalDataPath = path.join(context.generatedFilesDir, 'globalData.json')
            const globalData = JSON.parse(await fs.promises.readFile(globalDataPath, 'utf8')) as {
              'docusaurus-plugin-content-docs'?: {
                default?: {
                  versions?: Array<{
                    name?: string
                    docs?: Array<{ id?: string; path?: string }>
                  }>
                }
              }
            }
            const versions = globalData['docusaurus-plugin-content-docs']?.default?.versions ?? []
            const currentVersion = versions.find((version) => version.name === 'current') ?? versions[0]
            currentVersion?.docs?.forEach((doc) => {
              if (doc.id && doc.path) {
                docPermalinks.set(doc.id, doc.path)
              }
            })
          } catch (err) {
            console.warn('Unable to load Docusaurus permalinks for llms.txt; falling back to doc IDs.', err)
          }

          // Group all docs records by category and prepare content maps
          const contentByPath = new Map<string, string>()
          const docsRecords: string[] = []

          Object.entries(currentVersionDocsRoutes).forEach(([path, record]) => {
            const permalink = docPermalinks.get(path) ?? `/${path}`
            const docEntry = `- [${record.title}](https://docs.rushdb.com${permalink}): ${record.description}`
            docsRecords.push(docEntry)

            // Categorize by path prefix
            if (path.startsWith('learn/')) {
              categories.learn.push(docEntry)
            } else if (path.startsWith('connect/')) {
              categories.connect.push(docEntry)
            } else if (path.startsWith('deploy/')) {
              categories.deploy.push(docEntry)
            } else if (path.startsWith('rushdb-cloud/')) {
              categories['rushdb-cloud'].push(docEntry)
            } else if (path.startsWith('get-started/')) {
              categories['get-started'].push(docEntry)
            }
          })

          // Build main llms.txt file
          const llmsTxt = `# ${context.siteConfig.title}\n\n## Docs\n\n${docsRecords.join('\n')}`
          const llmsTxtPath = path.join(llmsDir, 'llms.txt')
          try {
            fs.writeFileSync(llmsTxtPath, llmsTxt)
          } catch (err) {
            throw err
          }

          // Generate isolated txt files for each category
          for (const [categoryName, categoryDocs] of Object.entries(categories)) {
            if (categoryDocs.length > 0) {
              const categoryTitle =
                categoryName === 'rushdb-cloud' ? 'RushDB Cloud' : (
                  `RushDB ${categoryName
                    .split('-')
                    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
                    .join(' ')}`
                )

              const categoryTxt = `# ${categoryTitle}\n\n${categoryDocs.join('\n')}`
              const categoryPath = path.join(llmsDir, `llms-${categoryName}.txt`)

              try {
                fs.writeFileSync(categoryPath, categoryTxt)
              } catch (err) {
                console.error(`Error writing ${categoryName} file:`, err)
              }
            }
          }

          // Generate isolated content files with full MDX content
          // We need to parse the full content and categorize by path
          await generateCategorizedContentFiles(llmsDir, allMdx, currentVersionDocsRoutes, filePathToContent)
        }
      }
    }
  ],

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.ts',
          sidebarCollapsed: false,
          sidebarCollapsible: false
        },
        blog: false,
        pages: false,
        theme: {
          customCss: './src/css/custom.css'
        }
      } satisfies Preset.Options
    ]
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    {
      docs: {
        sidebar: {
          hideable: false,
          autoCollapseCategories: false
        }
      },
      themeConfig: {
        colorMode: {
          defaultMode: 'dark'
        },
        mermaid: {
          theme: { light: 'neutral', dark: 'forest' }
        }
      },
      image: 'img/og.png',
      navbar: {
        title: 'docs',
        logo: {
          alt: 'RushDB Logo',
          src: 'img/logo.svg'
        },
        items: [
          {
            label: 'Learn',
            to: '/build/data/',
            position: 'left',
            activeBaseRegex: '/(build|connect|deploy|reference|rest-api|tutorials)/'
          },
          {
            href: 'https://github.com/rush-db/rushdb',
            // label: 'GitHub',
            position: 'right',
            className: 'github header-github-link',
            'aria-label': 'GitHub repository'
          },
          {
            href: 'https://rushdb.com',
            label: 'Website',
            position: 'right'
          },
          {
            href: 'https://app.rushdb.com',
            label: 'GET API KEY',
            position: 'right',
            className: 'cta-button'
          }
        ]
      },
      prism: {
        theme: lightCodeTheme,
        darkTheme: darkCodeTheme,
        fontFamily: 'JetBrains Mono',
        additionalLanguages: ['python', 'javascript', 'typescript', 'bash', 'json']
      }
    }
}

export default config
