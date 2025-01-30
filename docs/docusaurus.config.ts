import type { Config } from '@docusaurus/types'
import type * as Preset from '@docusaurus/preset-classic'
import { themes } from 'prism-react-renderer'
import tailwindPlugin from './plugins/tailwind-config.cjs'
import { version } from './package.json'

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
  title: 'RushDB',
  organizationName: 'Collect Software Inc',
  projectName: 'RushDB Docs',
  tagline: 'Instant Database for Modern Apps',
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

  plugins: [tailwindPlugin],

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      {
        docs: {
          routeBasePath: '/',
          sidebarPath: './sidebars.ts'
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
          hideable: true
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
        title: 'RushDB Docs',
        logo: {
          alt: 'RushDB Logo',
          src: 'img/logo.svg'
        },
        items: [
          {
            href: 'https://github.com/rush-db/rushdb',
            label: 'GitHub',
            position: 'right'
          },
          {
            href: 'https://www.npmjs.com/package/@rushdb/javascript-sdk',
            label: `NPM v${version}`,
            position: 'right'
          }
        ]
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Docs',
            items: [
              {
                label: 'Quick start',
                to: '/quick-start/installation'
              },
              {
                label: 'Working with RushDB SDK',
                to: '/working-with-rushdb-sdk/rushdb-sdk-intro'
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
              }
            ]
          },
          {
            title: 'More',
            items: [
              {
                label: 'GitHub',
                href: 'https://github.com/rush-db/rushdb'
              }
            ]
          }
        ],
        copyright: `© ${new Date().getFullYear()}, Collect Software Inc.`
      },
      prism: {
        theme: themes.oneLight,
        darkTheme: atomTheme,
        fontStyle: 'italic'
      }
    }
}

export default config
