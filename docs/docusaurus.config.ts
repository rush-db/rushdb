import type { Config } from '@docusaurus/types'
import type * as Preset from '@docusaurus/preset-classic'
import { themes } from 'prism-react-renderer'
import tailwindPlugin from './plugins/tailwind-config.cjs'
import { version } from './package.json'
import * as path from 'node:path'
import * as fs from 'node:fs'

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

          // recursive function to get all mdx files
          const getMdxFiles = async (dir: string) => {
            const entries = await fs.promises.readdir(dir, { withFileTypes: true })

            for (const entry of entries) {
              const fullPath = path.join(dir, entry.name)
              if (entry.isDirectory()) {
                await getMdxFiles(fullPath)
              } else if (entry.name.endsWith('.mdx') || entry.name.endsWith('.md')) {
                let content = await fs.promises.readFile(fullPath, 'utf8')

                content = content.replace(/^---\n(?:.*\n)*?sidebar_position:\s*\d+\n(?:.*\n)*?---\n/, '')

                allMdx.push(content)
              }
            }
          }

          await getMdxFiles(contentDir)
          return { allMdx }
        },
        postBuild: async ({ content, routes, outDir }) => {
          const { allMdx } = content as { allMdx: string[] }

          // Write concatenated MDX content
          const concatenatedPath = path.join(outDir, 'llms-full.txt')
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

          // for every single docs route we now parse a path (which is the key) and a title
          const docsRecords = Object.entries(currentVersionDocsRoutes).map(([path, record]) => {
            return `- [${record.title}](https://docs.rushdb.com/${path}): ${record.description}`
          })

          // Build up llms.txt file
          const llmsTxt = `# ${context.siteConfig.title}\n\n## Docs\n\n${docsRecords.join('\n')}`

          // Write llms.txt file
          const llmsTxtPath = path.join(outDir, 'llms.txt')
          try {
            fs.writeFileSync(llmsTxtPath, llmsTxt)
          } catch (err) {
            throw err
          }
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
