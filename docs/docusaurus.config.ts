import type { Config } from '@docusaurus/types'
import type * as Preset from '@docusaurus/preset-classic'
import { themes } from 'prism-react-renderer'
import tailwindPlugin from './plugins/tailwind-config.cjs'
import * as path from 'node:path'
import * as fs from 'node:fs'

// Helper function to generate categorized content files
async function generateCategorizedContentFiles(
  llmsDir: string,
  allMdx: string[],
  currentVersionDocsRoutes: Record<string, Record<string, unknown>>,
  filePathToContent: Map<string, string>
) {
  // Create mapping from file content to routes to categorize the full content
  const categories = {
    concepts: [] as string[],
    'typescript-sdk': [] as string[],
    'python-sdk': [] as string[],
    'rest-api': [] as string[],
    tutorials: [] as string[],
    'get-started': [] as string[]
  }

  // Group content by file paths directly
  for (const [filePath, content] of filePathToContent.entries()) {
    // Categorize by file path prefix
    if (filePath.startsWith('concepts/')) {
      categories.concepts.push(content)
    } else if (filePath.startsWith('typescript-sdk/')) {
      categories['typescript-sdk'].push(content)
    } else if (filePath.startsWith('python-sdk/')) {
      categories['python-sdk'].push(content)
    } else if (filePath.startsWith('rest-api/')) {
      categories['rest-api'].push(content)
    } else if (filePath.startsWith('tutorials/')) {
      categories.tutorials.push(content)
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

const atomTheme = {
  plain: {
    color: '#F8F8F2',
    backgroundColor: '#231919'
  },
  styles: [
    {
      types: ['prolog', 'constant', 'builtin'],
      style: {
        color: '#BD93F9'
      }
    },
    {
      types: ['inserted', 'function'],
      style: {
        color: '#50FA7B'
      }
    },
    {
      types: ['deleted'],
      style: {
        color: '#FF5555'
      }
    },
    {
      types: ['changed'],
      style: {
        color: '#FFB86C'
      }
    },
    {
      types: ['punctuation', 'symbol'],
      style: {
        color: '#F8F8F2'
      }
    },
    {
      types: ['string', 'char', 'tag', 'selector'],
      style: {
        color: '#FF79C6'
      }
    },
    {
      types: ['keyword', 'variable'],
      style: {
        color: '#BD93F9'
      }
    },
    {
      types: ['comment'],
      style: {
        color: '#6272A4'
      }
    },
    {
      types: ['attr-name'],
      style: {
        color: '#F1FA8C'
      }
    }
  ]
}

const config: Config = {
  markdown: {
    mermaid: true
  },
  themes: ['@docusaurus/theme-mermaid'],
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
          const llmsDir = path.join(outDir, 'llms')
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
            concepts: [] as string[],
            'typescript-sdk': [] as string[],
            'python-sdk': [] as string[],
            'rest-api': [] as string[],
            tutorials: [] as string[],
            'get-started': [] as string[]
          }

          // Group all docs records by category and prepare content maps
          const contentByPath = new Map<string, string>()
          const docsRecords: string[] = []

          Object.entries(currentVersionDocsRoutes).forEach(([path, record]) => {
            const docEntry = `- [${record.title}](https://docs.rushdb.com/${path}): ${record.description}`
            docsRecords.push(docEntry)

            // Categorize by path prefix
            if (path.startsWith('concepts/')) {
              categories.concepts.push(docEntry)
            } else if (path.startsWith('typescript-sdk/')) {
              categories['typescript-sdk'].push(docEntry)
            } else if (path.startsWith('python-sdk/')) {
              categories['python-sdk'].push(docEntry)
            } else if (path.startsWith('rest-api/')) {
              categories['rest-api'].push(docEntry)
            } else if (path.startsWith('tutorials/')) {
              categories.tutorials.push(docEntry)
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
              const categoryTitle = categoryName
                .split('-')
                .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
                .join(' ')

              const categoryTxt = `# RushDB ${categoryTitle}\n\n${categoryDocs.join('\n')}`
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
        title: '',
        logo: {
          alt: 'RushDB Logo',
          src: 'img/logo.svg'
        },
        items: [
          {
            href: '/',
            label: 'RushDB Docs',
            position: 'left'
          },
          {
            label: 'Python SDK',
            href: '/python-sdk/introduction',
            className: 'python-sdk',
            position: 'left',
            activeBaseRegex: '/python-sdk/'
          },
          {
            label: 'TypeScript SDK',
            href: '/typescript-sdk/introduction',
            className: 'typescript-sdk',
            position: 'left',
            activeBaseRegex: '/typescript-sdk/'
          },
          {
            label: 'REST API',
            href: '/rest-api/introduction',
            className: 'rest-api',
            position: 'left',
            activeBaseRegex: '/rest-api/'
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
            label: 'Dashboard',
            position: 'right',
            className: 'cta-button'
          }
        ]
      },
      footer: {
        links: [
          {
            title: 'Documentation',
            items: [
              {
                label: 'Getting Started',
                to: '/get-started/quick-tutorial'
              },
              {
                label: 'Basic Concepts',
                to: '/concepts/records'
              }
            ]
          },
          {
            title: 'SDKs',
            items: [
              {
                label: 'REST API',
                to: '/rest-api/introduction'
              },
              {
                label: 'Python SDK',
                to: '/python-sdk/introduction'
              },
              {
                label: 'TypeScript SDK',
                to: '/typescript-sdk/introduction'
              }
            ]
          },
          {
            title: 'Community',
            items: [
              {
                label: 'X (Twitter)',
                href: 'https://x.com/RushDatabase'
              },
              {
                label: 'LinkedIn',
                href: 'https://www.linkedin.com/company/rushdb'
              },
              {
                label: 'GitHub',
                href: 'https://github.com/rush-db/rushdb'
              }
            ],
            className: 'text-right'
          }
        ],
        copyright: `© ${new Date().getFullYear()}, Collect Software Inc.`
      },
      prism: {
        theme: themes.oneLight,
        darkTheme: atomTheme,
        fontFamily: 'JetBrains Mono',
        additionalLanguages: ['python', 'javascript', 'typescript', 'bash', 'json']
      }
    }
}

export default config
