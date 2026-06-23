import type { SidebarsConfig } from '@docusaurus/plugin-content-docs'

const sidebars: SidebarsConfig = {
  docs: [
    {
      type: 'link',
      label: 'Quick Start',
      href: '/get-started/quick-tutorial',
      customProps: { icon: 'Rocket' }
    },
    {
      type: 'category',
      label: 'Deploy',
      collapsible: false,
      collapsed: false,
      customProps: { icon: 'Server' },
      items: [
        {
          type: 'category',
          label: 'Local Hosting',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Monitor' },
          link: { type: 'doc', id: 'deploy/local-hosting/index' },
          items: [
            'deploy/local-hosting/docker',
            'deploy/local-hosting/from-source',
            'deploy/local-hosting/helm'
          ]
        },
        {
          type: 'category',
          label: 'Remote Hosting',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Cloud' },
          link: { type: 'doc', id: 'deploy/remote-hosting/index' },
          items: [
            'deploy/remote-hosting/prerequisites',
            'deploy/remote-hosting/templates',
            'deploy/remote-hosting/aws-gcp-azure',
            'deploy/remote-hosting/self-hosting-rushdb'
          ]
        },
        {
          type: 'category',
          label: 'Configuration',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Settings2' },
          link: { type: 'doc', id: 'deploy/configuration/index' },
          items: [
            'deploy/configuration/environment-variables',
            'deploy/configuration/get-api-key',
            'deploy/configuration/security'
          ]
        },
        {
          type: 'category',
          label: 'Infrastructure',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Database' },
          link: { type: 'doc', id: 'deploy/infrastructure/index' },
          items: [
            'deploy/infrastructure/rushdb-platform',
            'deploy/infrastructure/neo4j-and-aura',
            'deploy/infrastructure/postgresql-sqlite'
          ]
        },
        {
          type: 'category',
          label: 'Operations',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'LifeBuoy' },
          items: ['deploy/operations/backup-restore', 'deploy/operations/troubleshooting']
        }
      ]
    },
    {
      type: 'category',
      label: 'Connect',
      collapsible: false,
      collapsed: false,
      customProps: { icon: 'Plug' },
      link: { type: 'doc', id: 'connect/index' },
      items: [
        {
          type: 'link',
          label: 'Skills',
          href: '/connect/skills',
          className: 'connect-sidebar-link connect-sidebar-link--skills',
          customProps: { icon: 'Sparkles' }
        },
        {
          type: 'link',
          label: 'MCP',
          href: '/connect/mcp',
          className: 'connect-sidebar-link connect-sidebar-link--mcp',
          customProps: { icon: 'Cpu' }
        },
        {
          type: 'link',
          label: 'Agents',
          href: '/connect/agents',
          className: 'connect-sidebar-link connect-sidebar-link--agents',
          customProps: { icon: 'Bot' }
        },
        {
          type: 'link',
          label: 'SDK & REST',
          href: '/connect/sdks',
          className: 'connect-sidebar-link connect-sidebar-link--sdks',
          customProps: { icon: 'Code2' }
        }
      ]
    },
    {
      type: 'category',
      label: 'Learn',
      collapsible: false,
      collapsed: false,
      customProps: { icon: 'Hammer' },
      items: [
        {
          type: 'category',
          label: 'Agent Memory',
          collapsible: true,
          collapsed: false,
          customProps: { icon: 'Brain' },
          link: { type: 'doc', id: 'learn/agent-memory/index' },
          items: ['learn/agent-memory/quickstart', 'learn/agent-memory/schema-self-awareness']
        },
        {
          type: 'category',
          label: 'Records & Queries',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Database' },
          link: { type: 'doc', id: 'learn/records-and-queries/index' },
          items: [
            'learn/records-and-queries/store-records',
            'learn/records-and-queries/import-data',
            'learn/records-and-queries/find-and-query',
            'learn/records-and-queries/raw-queries',
            'learn/records-and-queries/export-data',
            'learn/records-and-queries/labels-and-properties',
            'learn/records-and-queries/discover-your-schema',
            'learn/records-and-queries/labeled-meta-property-graph',
            'learn/records-and-queries/transactions'
          ]
        },
        {
          type: 'category',
          label: 'SearchQuery',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Filter' },
          link: { type: 'doc', id: 'learn/search-query/search-query' },
          items: [
            'learn/search-query/where-operators',
            'learn/search-query/select-expressions',
            'learn/search-query/group-by',
            'learn/search-query/pagination-order',
            'learn/search-query/search-labels'
          ]
        },
        {
          type: 'category',
          label: 'Relationships',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Network' },
          link: { type: 'doc', id: 'learn/relationships/index' },
          items: [
            'learn/relationships/connect-records',
            'learn/relationships/bulk-relationships',
            'learn/relationships/suggested-patterns'
          ]
        },
        {
          type: 'category',
          label: 'Semantic Search',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'Search' },
          link: { type: 'doc', id: 'learn/semantic-search/semantic-search' },
          items: [
            'learn/semantic-search/manage-indexes',
            'learn/semantic-search/write-with-vectors',
            'learn/semantic-search/bring-your-own-vectors'
          ]
        },
        {
          type: 'category',
          label: 'Tutorials',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'BookOpen' },
          link: { type: 'doc', id: 'learn/tutorials/index' },
          items: [
            {
              type: 'category',
              label: 'AI & RAG',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Cpu' },
              items: [
                'learn/tutorials/ai-and-rag/ai-semantic-search',
                'learn/tutorials/ai-and-rag/graphrag',
                'learn/tutorials/ai-and-rag/hybrid-retrieval',
                'learn/tutorials/ai-and-rag/rag-pipeline',
                'learn/tutorials/ai-and-rag/rag-evaluation',
                'learn/tutorials/ai-and-rag/rag-multi-source',
                'learn/tutorials/ai-and-rag/rag-reranking',
                'learn/tutorials/ai-and-rag/semantic-search-multitenant',
                'learn/tutorials/ai-and-rag/explainable-results',
                'learn/tutorials/ai-and-rag/byov-when-and-why',
                'learn/tutorials/ai-and-rag/byov-external-embeddings'
              ]
            },
            {
              type: 'category',
              label: 'Agent Memory',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Brain' },
              items: [
                'learn/tutorials/agent-memory/agent-safe-query-planning',
                'learn/tutorials/agent-memory/agent-skills-with-openclaw',
                'learn/tutorials/agent-memory/building-team-memory',
                'learn/tutorials/agent-memory/episodic-memory',
                'learn/tutorials/agent-memory/memory-layer',
                'learn/tutorials/agent-memory/research-knowledge-graph'
              ]
            },
            {
              type: 'category',
              label: 'Graph Modeling',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Network' },
              items: [
                'learn/tutorials/graph-modeling/thinking-in-graphs',
                'learn/tutorials/graph-modeling/choosing-relationship-types',
                'learn/tutorials/graph-modeling/modeling-hierarchies',
                'learn/tutorials/graph-modeling/temporal-graphs',
                'learn/tutorials/graph-modeling/data-lineage',
                'learn/tutorials/graph-modeling/versioning-records',
                'learn/tutorials/graph-modeling/customer-360'
              ]
            },
            {
              type: 'category',
              label: 'Working with Data',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Database' },
              items: [
                'learn/tutorials/working-with-data/importing-data',
                'learn/tutorials/working-with-data/importing-from-mongodb',
                'learn/tutorials/working-with-data/event-driven-ingestion',
                'learn/tutorials/working-with-data/third-party-webhook-ingestion',
                'learn/tutorials/working-with-data/graph-backed-api'
              ]
            },
            {
              type: 'category',
              label: 'Search & Queries',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Search' },
              items: [
                'learn/tutorials/search-and-queries/discovery-queries',
                'learn/tutorials/search-and-queries/reusable-search-query',
                'learn/tutorials/search-and-queries/searchquery-advanced-patterns',
                'learn/tutorials/search-and-queries/search-ux-patterns',
                'learn/tutorials/search-and-queries/query-optimization',
                'learn/tutorials/search-and-queries/testing-searchquery'
              ]
            },
            {
              type: 'category',
              label: 'Use Cases',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Globe' },
              items: [
                'learn/tutorials/use-cases/audit-trails',
                'learn/tutorials/use-cases/compliance-retention',
                'learn/tutorials/use-cases/incident-response',
                'learn/tutorials/use-cases/supply-chain-traceability',
                'learn/tutorials/use-cases/byoc-vs-managed',
                'learn/tutorials/use-cases/is-rushdb-right-for-me'
              ]
            }
          ]
        },
        {
          type: 'category',
          label: 'Reference',
          collapsible: true,
          collapsed: true,
          customProps: { icon: 'BookMarked' },
          items: [
            {
              type: 'category',
              label: 'TypeScript',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'FileCode' },
              link: { type: 'doc', id: 'learn/reference/typescript/index' },
              items: [
                'learn/reference/typescript/DBRecord',
                'learn/reference/typescript/DBRecordInstance',
                'learn/reference/typescript/DBRecordTarget',
                'learn/reference/typescript/DBRecordsArrayInstance',
                'learn/reference/typescript/Model',
                'learn/reference/typescript/relationship-patterns',
                'learn/reference/typescript/RelationTarget',
                'learn/reference/typescript/RushDB',
                'learn/reference/typescript/SearchQuery',
                'learn/reference/typescript/Transaction'
              ]
            },
            {
              type: 'category',
              label: 'Python',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'FileCode' },
              link: { type: 'doc', id: 'learn/reference/python/index' },
              items: [
                'learn/reference/python/record',
                'learn/reference/python/relationship-patterns',
                'learn/reference/python/RushDB',
                'learn/reference/python/search-result',
                'learn/reference/python/SearchQuery',
                'learn/reference/python/transaction'
              ]
            },
            {
              type: 'category',
              label: 'REST API',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Globe' },
              link: { type: 'doc', id: 'learn/reference/rest-api/introduction' },
              items: [
                {
                  type: 'category',
                  label: 'Records',
                  collapsible: true,
                  collapsed: true,
                  customProps: { icon: 'Database' },
                  items: [
                    'learn/reference/rest-api/records/create-records',
                    'learn/reference/rest-api/records/get-records',
                    'learn/reference/rest-api/records/update-records',
                    'learn/reference/rest-api/records/delete-records',
                    'learn/reference/rest-api/records/import-data',
                    'learn/reference/rest-api/records/export-data'
                  ]
                },
                {
                  type: 'category',
                  label: 'AI & Vectors',
                  collapsible: true,
                  collapsed: true,
                  customProps: { icon: 'Sparkles' },
                  link: { type: 'doc', id: 'learn/reference/rest-api/ai-and-vectors/overview' },
                  items: [
                    'learn/reference/rest-api/ai-and-vectors/indexing',
                    'learn/reference/rest-api/ai-and-vectors/advanced-indexing',
                    'learn/reference/rest-api/ai-and-vectors/search',
                    'learn/reference/rest-api/ai-and-vectors/write-with-vectors'
                  ]
                },
                'learn/reference/rest-api/labels',
                'learn/reference/rest-api/properties',
                'learn/reference/rest-api/relationships',
                'learn/reference/rest-api/transactions',
                'learn/reference/rest-api/raw-queries'
              ]
            },
            {
              type: 'category',
              label: 'MCP',
              collapsible: true,
              collapsed: true,
              customProps: { icon: 'Cpu' },
              link: { type: 'doc', id: 'learn/reference/mcp/index' },
              items: ['learn/reference/mcp/tools']
            }
          ]
        }
      ]
    },
    {
      type: 'category',
      label: 'RushDB Cloud',
      collapsible: true,
      collapsed: false,
      customProps: { icon: 'Cloud' },
      link: { type: 'doc', id: 'rushdb-cloud/index' },
      items: [
        'rushdb-cloud/billing-model',
        'rushdb-cloud/project-isolation',
        'rushdb-cloud/licensing-and-services',
        'rushdb-cloud/knowledge-units'
      ]
    }
  ]
}

export default sidebars
