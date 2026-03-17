// Copyright Collect Software, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import type { Schema } from 'jsonschema'

export type ToolName =
  | 'getOntologyMarkdown'
  | 'getOntology'
  | 'findLabels'
  | 'createRecord'
  | 'updateRecord'
  | 'deleteRecord'
  | 'findRecords'
  | 'getRecord'
  | 'getRecordsByIds'
  | 'attachRelation'
  | 'detachRelation'
  | 'findRelationships'
  | 'bulkCreateRecords'
  | 'bulkDeleteRecords'
  | 'exportRecords'
  | 'helpAddToClient'
  | 'getQueryBuilderPrompt'
  | 'getSearchQuerySpec'
  | 'setRecord'
  | 'findOneRecord'
  | 'findUniqRecord'
  | 'deleteRecordById'
  | 'propertyValues'
  | 'findProperties'
  | 'findPropertyById'
  | 'deleteProperty'

type SecurityScheme = { type: 'oauth2'; scopes: string[] } | { type: 'noauth' }

type ToolAnnotations = {
  /** True for tools that only retrieve or compute — never write/delete outside ChatGPT. */
  readOnlyHint: boolean
  /** False for tools that only affect RushDB (a bounded target). */
  openWorldHint: boolean
  /** True for tools that can delete, overwrite, or have irreversible side effects. */
  destructiveHint: boolean
}

type Tool = {
  name: ToolName
  description: string
  inputSchema: Schema
  annotations: ToolAnnotations
  securitySchemes?: SecurityScheme[]
}

// Shared annotation presets
const READ_ONLY: ToolAnnotations = { readOnlyHint: true, openWorldHint: false, destructiveHint: false }
const WRITE: ToolAnnotations = { readOnlyHint: false, openWorldHint: false, destructiveHint: false }
const DESTROY: ToolAnnotations = { readOnlyHint: false, openWorldHint: false, destructiveHint: true }

// Read-only tools advertise both noauth and oauth2 so ChatGPT can call them
// anonymously and also offer linking for user-specific data.
const READ_SCHEMES: SecurityScheme[] = [{ type: 'noauth' }, { type: 'oauth2', scopes: ['records:read'] }]
const WRITE_SCHEMES: SecurityScheme[] = [{ type: 'oauth2', scopes: ['records:write'] }]

export const tools: Tool[] = [
  {
    name: 'getOntologyMarkdown',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'STEP 0 — call this ONCE at the start of every conversation before constructing any query. ' +
      'Returns the complete graph ontology as compact Markdown: all labels with record counts, ' +
      'all properties per label with their type and value ranges (min/max for numbers/datetimes, ' +
      'sample values for strings/booleans), and all cross-label relationships with direction. ' +
      'This single call replaces the need for separate findLabels + findProperties + findRelationships ' +
      'discovery calls. Use the result to determine exact label names (case-sensitive), field names, ' +
      'field types, and relationship patterns before building any findRecords query. ' +
      'Optionally pass `labels` array to narrow the output to specific labels.',
    inputSchema: {
      type: 'object',
      properties: {
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Optional: scope ontology to specific labels only. Leave empty to get all labels.'
        }
      },
      required: []
    }
  },
  {
    name: 'getOntology',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Returns the same graph ontology as getOntologyMarkdown but as structured JSON. ' +
      'Each item has: label (string), count (number), properties (array with id, name, type, ' +
      'min/max for numbers/datetimes, values[] for strings/booleans), and relationships ' +
      '(array with label, type, direction: in|out). ' +
      'Use this when you need property `id` values to pass to propertyValues for deeper drill-down. ' +
      'For initial schema orientation, getOntologyMarkdown uses fewer tokens.',
    inputSchema: {
      type: 'object',
      properties: {
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Optional: scope ontology to specific labels only. Leave empty to get all labels.'
        }
      },
      required: []
    }
  },
  {
    name: 'findLabels',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'List or filter available record types (labels) and their counts. ' +
      'IMPORTANT: getOntologyMarkdown already returns ALL label names in STEP 0 — do NOT call findLabels as a substitute for it. ' +
      'Only call findLabels when: (a) you need to search/filter labels by name, or (b) getOntologyMarkdown was not called yet. ' +
      'Call with no arguments to list all labels. ' +
      'To search by name: where: { name: { $contains: "candidate" } }. ' +
      'Returns objects with name (case-sensitive — use exact casing in all subsequent calls) and count (number of records). ' +
      'Pick the best matching label by: exact match > starts-with > substring > semantic similarity, preferring higher count on ties. ' +
      'State your label assumption briefly ("using DEAL for \'deals\'") and proceed without asking.',
    inputSchema: {
      type: 'object',
      properties: {
        where: {
          type: 'object',
          description: 'Filter conditions. Use { name: { $contains: "..." } } to search by label name.'
        },
        limit: { type: 'number', description: 'Maximum number of labels to return' },
        skip: { type: 'number', description: 'Number of labels to skip' },
        orderBy: {
          type: 'object',
          description: 'Sorting configuration: key = field, value = asc|desc',
          additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
        }
      },
      required: []
    }
  },
  {
    name: 'createRecord',
    annotations: WRITE,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Create a new record. ' +
      'Use the exact label casing returned by findLabels. ' +
      'Set mergeStrategy + mergeBy in options to enable upsert semantics (merge existing vs. replace).',

    inputSchema: {
      type: 'object',
      properties: {
        label: { type: 'string', description: 'Label for the record' },
        data: { type: 'object', description: 'The record data to insert' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic creation' },
        options: {
          type: 'object',
          description: 'Optional creation/upsert options',
          properties: {
            mergeStrategy: {
              type: 'string',
              enum: ['append', 'rewrite'],
              description:
                'Upsert strategy when matching an existing record: append/merge keeps unspecified existing properties; rewrite replaces.'
            },
            mergeBy: {
              type: 'array',
              items: { type: 'string' },
              description:
                'Fields used to match an existing record. Empty array => all incoming keys. Presence (even empty) triggers upsert when combined with mergeStrategy or alone.'
            }
          },
          required: []
        }
      },
      required: ['label', 'data']
    }
  },
  {
    name: 'updateRecord',
    annotations: WRITE,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Partially update a record — only fields present in data are changed; all other existing fields are preserved. ' +
      'Use setRecord instead if you want to replace all fields. ' +
      'Requires recordId: retrieve it first with findRecords or getRecord if not already known.',

    inputSchema: {
      type: 'object',
      properties: {
        recordId: { type: 'string', description: 'ID of the record to update' },
        label: { type: 'string', description: 'Label for the record' },
        data: { type: 'object', description: 'The updated (partial) record data' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic update' }
      },
      required: ['recordId', 'label', 'data']
    }
  },
  {
    name: 'deleteRecord',
    annotations: DESTROY,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Delete a single record by ID. Irreversible. ' +
      'Always confirm with the user before calling. Use findRecords to preview the record first.',

    inputSchema: {
      type: 'object',
      properties: {
        recordId: { type: 'string', description: 'ID of the record to delete' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic deletion' }
      },
      required: ['recordId']
    }
  },
  {
    name: 'findRecords',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Search records with a structured SearchQuery. ' +
      '⚠ BEFORE building any query with dates, aggregation, groupBy, relationship traversal, or vector search — call getSearchQuerySpec to load the complete syntax reference. ' +
      'INTENT: aggregation request (count/total/sum/avg/breakdown/top N by metric) → MUST include aggregate + groupBy. NEVER fetch raw records to count/sum manually. ' +
      'RESPONSE: { data:[...records], total:N } — for simple "how many" read total directly; no count aggregate needed. ' +
      'HARD RULES: ' +
      '(1) NEVER set limit when aggregate is present — restricts the record scan and produces mathematically wrong results. Omit limit for all aggregation queries. ' +
      '(2) Every fn-based aggregate entry MUST include alias ("$record" for root fields; the $alias declared in where for related nodes).',

    inputSchema: {
      type: 'object',
      properties: {
        labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
        where: {
          type: 'object',
          description:
            'Filter conditions. Field names must match exactly what findProperties/getOntologyMarkdown returns. ' +
            'For the complete operator reference (string/number/boolean/datetime/vector/$exists/$type/logical/$alias/$relation/$id) call getSearchQuerySpec.'
        },
        limit: {
          type: 'number',
          description:
            'Max records for listing queries (default 10, max 1000). ' +
            'NEVER set when aggregate is present — restricts the scan and produces wrong results.',
          default: 10
        },
        skip: { type: 'number', description: 'Number of records to skip', default: 0 },
        orderBy: {
          type: 'object',
          description: 'Sorting configuration: key = field, value = asc|desc',
          additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
        },
        aggregate: {
          type: 'object',
          description:
            'Map of output-key → aggregation spec. fn: count|sum|avg|min|max|collect|timeBucket. ' +
            'alias required on every fn-based entry: "$record" for root fields; $alias from where for related nodes. ' +
            'Call getSearchQuerySpec for full aggregate/groupBy/collect/timeBucket reference.',
          additionalProperties: {
            type: 'object',
            properties: {
              fn: {
                type: 'string',
                enum: ['count', 'sum', 'avg', 'min', 'max', 'collect', 'timeBucket'],
                description: 'Aggregation function'
              },
              field: {
                type: 'string',
                description: 'Field to aggregate (required for all fns except count)'
              },
              alias: {
                type: 'string',
                description: '"$record" for root-label fields; the $alias value from where for related nodes'
              },
              precision: { type: 'number', description: 'Decimal places for avg results' },
              unique: { type: 'boolean', description: 'For collect: deduplicate (default true)' },
              granularity: {
                type: 'string',
                enum: ['day', 'week', 'month', 'quarter', 'year'],
                description: 'For timeBucket: time bucket size'
              }
            },
            required: ['fn', 'alias']
          }
        },
        groupBy: {
          type: 'array',
          items: { type: 'string' },
          description:
            'Two modes: (A) Dimensional — "$alias.propertyName" strings, one row per distinct value; ' +
            '(B) Self-group — aggregation key names, collapses to one row. Call getSearchQuerySpec for full reference.'
        }
      },
      required: []
    }
  },
  {
    name: 'getRecord',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Fetch a single record by its ID. Use when you already have the ID from a previous findRecords or findOneRecord call.',

    inputSchema: {
      type: 'object',
      properties: { recordId: { type: 'string', description: 'ID of the record to retrieve' } },
      required: ['recordId']
    }
  },
  {
    name: 'getRecordsByIds',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Fetch multiple records by their IDs in one call. Use after collecting IDs from a findRecords query.',

    inputSchema: {
      type: 'object',
      properties: {
        recordIds: {
          type: 'array',
          items: { type: 'string' },
          description: 'Array of record IDs to retrieve'
        }
      },
      required: ['recordIds']
    }
  },
  {
    name: 'attachRelation',
    annotations: WRITE,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Create a directed or bidirectional relationship between records. ' +
      'sourceId and targetId/targetIds must already exist — use findRecords to resolve records by name/attribute first.',

    inputSchema: {
      type: 'object',
      properties: {
        sourceId: { type: 'string', description: 'ID of the source record' },
        targetId: {
          type: 'string',
          description: 'ID of one target record (deprecated if targetIds provided)'
        },
        targetIds: {
          type: 'array',
          items: { type: 'string' },
          description: 'IDs of multiple target records'
        },
        relationType: { type: 'string', description: 'Type of the relationship' },
        direction: {
          type: 'string',
          enum: ['outgoing', 'incoming', 'bidirectional'],
          description: 'Direction of the relationship',
          default: 'outgoing'
        },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic relation creation' }
      },
      required: ['sourceId']
    }
  },
  {
    name: 'detachRelation',
    annotations: WRITE,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Remove a relationship between records. ' +
      'Use findRelationships to inspect existing relationships and confirm the correct type/direction before detaching.',

    inputSchema: {
      type: 'object',
      properties: {
        sourceId: { type: 'string', description: 'ID of the source record' },
        targetId: {
          type: 'string',
          description: 'ID of one target record (deprecated if targetIds provided)'
        },
        targetIds: {
          type: 'array',
          items: { type: 'string' },
          description: 'IDs of multiple target records'
        },
        relationType: { type: 'string', description: 'Type of the relationship to remove' },
        direction: {
          type: 'string',
          enum: ['outgoing', 'incoming', 'bidirectional'],
          description: 'Direction of the relationship',
          default: 'outgoing'
        },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic relation removal' }
      },
      required: ['sourceId']
    }
  },
  {
    name: 'findRelationships',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Discover and traverse relationships between records. Use this tool in two scenarios: ' +
      '(1) Multi-hop path discovery — fetch a sample record ID, then call findRelationships filtered by that ID to reveal which labels are adjacent; repeat to trace the full path before building a nested findRecords where clause. ' +
      '(2) Direction/type filtering — when the user specifies a relationship type or direction that cannot be expressed in a findRecords where block. ' +
      'Does NOT support aggregate or groupBy — use findRecords for aggregations across related labels.',

    inputSchema: {
      type: 'object',
      properties: {
        where: { type: 'object', description: 'Search conditions for finding relationships' },
        limit: { type: 'number', description: 'Maximum number of relationships to return', default: 10 },
        skip: { type: 'number', description: 'Number of relationships to skip', default: 0 },
        orderBy: {
          type: 'object',
          description: 'Sorting configuration: key = field, value = asc|desc',
          additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
        }
      },
      required: []
    }
  },
  {
    name: 'bulkCreateRecords',
    annotations: WRITE,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Insert multiple records of the same label in one call. ' +
      'Set mergeStrategy ("append" to keep existing unspecified fields, "rewrite" to replace) and mergeBy (fields to match on) in options to enable upsert semantics. ' +
      'Use the exact label casing returned by findLabels.',

    inputSchema: {
      type: 'object',
      properties: {
        label: { type: 'string', description: 'Label for all records' },
        data: { type: 'array', items: { type: 'object' }, description: 'Array of record data to insert' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic bulk creation' },
        options: {
          type: 'object',
          description: 'Optional bulk creation/upsert options',
          properties: {
            mergeStrategy: {
              type: 'string',
              enum: ['append', 'rewrite'],
              description:
                'Global upsert strategy for the batch: append/merge keeps existing unspecified properties; rewrite replaces.'
            },
            mergeBy: {
              type: 'array',
              items: { type: 'string' },
              description:
                'Global fields used to match existing records. Empty array => all keys of each record. Presence triggers upsert semantics.'
            },
            returnResult: {
              type: 'boolean',
              description: 'Return created/upserted records instead of just OK message'
            }
          },
          required: []
        }
      },
      required: ['label', 'data']
    }
  },
  {
    name: 'bulkDeleteRecords',
    annotations: DESTROY,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Delete all records matching a query. IRREVERSIBLE and potentially high-impact. ' +
      'REQUIRED: always call findRecords with the same labels+where first to show the user a preview, then ask for explicit confirmation before calling this tool.',

    inputSchema: {
      type: 'object',
      properties: {
        labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
        where: { type: 'object', description: 'Search conditions for records to delete' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic bulk deletion' }
      },
      required: ['where']
    }
  },
  {
    name: 'exportRecords',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Export matching records as a CSV file. ' +
      'Accepts the same labels/where/orderBy filters as findRecords. ' +
      'Call findProperties first to know available field names if constructing a where filter.',

    inputSchema: {
      type: 'object',
      properties: {
        labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
        where: { type: 'object', description: 'Search conditions for records to export' },
        limit: { type: 'number', description: 'Maximum number of records to export' },
        orderBy: {
          type: 'object',
          description: 'Sorting configuration for export',
          additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
        }
      },
      required: []
    }
  },
  {
    name: 'helpAddToClient',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description: 'Help the user add the RushDB MCP server to their MCP client',
    inputSchema: { type: 'object', properties: {}, required: [] }
  },
  {
    name: 'getQueryBuilderPrompt',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Return the RushDB system prompt. Use this if your MCP client does not support the Prompts API.',
    inputSchema: { type: 'object', properties: {}, required: [] }
  },
  {
    name: 'getSearchQuerySpec',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Returns the complete RushDB SearchQuery specification as a focused reference document. ' +
      'Covers: all WHERE operators (string/number/boolean/datetime component objects/vector/$exists/$type), ' +
      'relationship traversal syntax ($alias/$relation/$id), logical grouping ($and/$or/$not/$nor/$xor), ' +
      'all aggregate functions (count/sum/avg/min/max/collect/timeBucket), both groupBy modes (dimensional + self-group), ' +
      'late-ordering rules, COLLECT nesting, limit rules by query mode, multi-hop path discovery, ' +
      'enum normalization, validation checklist, and annotated query examples. ' +
      'CALL THIS before building any findRecords query that involves dates, aggregation, groupBy, relationship traversal, or vector search. ' +
      'Do not guess operator syntax — use this spec as the source of truth.',
    inputSchema: { type: 'object', properties: {}, required: [] }
  },
  {
    name: 'setRecord',
    annotations: WRITE,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Replace ALL fields of a record with the provided data object — any existing fields not in data are deleted. ' +
      'Use updateRecord instead for partial/merge updates that preserve unspecified fields.',

    inputSchema: {
      type: 'object',
      properties: {
        recordId: { type: 'string', description: 'ID of the record to set' },
        label: { type: 'string', description: 'Label for the record' },
        data: { type: 'object', description: 'The new record data to set' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic set' }
      },
      required: ['recordId', 'label', 'data']
    }
  },
  {
    name: 'findOneRecord',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Return the first record matching the query — useful for entity resolution probes. ' +
      'Call with where: { <nameField>: { $contains: "..." } } and a small limit to resolve a named entity to its ID before using it in a relationship filter. ' +
      'Prefer this over findRecords when you need exactly one representative match rather than a full list.',

    inputSchema: {
      type: 'object',
      properties: {
        labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
        where: { type: 'object', description: 'Search conditions for finding the record' }
      },
      required: []
    }
  },
  {
    name: 'findUniqRecord',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Return the single record that uniquely matches the query — throws if zero or more than one record matches. ' +
      'Use for unique-key lookups (email, code, slug) where exactly one result is expected. ' +
      'Use findOneRecord instead when you only want the first match and duplicates are acceptable.',

    inputSchema: {
      type: 'object',
      properties: {
        labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
        where: { type: 'object', description: 'Search conditions for finding the unique record' }
      },
      required: []
    }
  },
  {
    name: 'deleteRecordById',
    annotations: DESTROY,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Delete a single record by ID. Irreversible. ' +
      'Always confirm with the user before calling. Use getRecord to preview the record if needed.',

    inputSchema: {
      type: 'object',
      properties: {
        recordId: { type: 'string', description: 'ID of the record to delete' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic deletion' }
      },
      required: ['recordId']
    }
  },
  {
    name: 'propertyValues',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Get statistics or distinct values for a specific property, identified by the `id` field returned from findProperties. ' +
      'What this tool returns depends on the property type:\n' +
      '  • number / datetime → returns { min, max } — use this to answer range, min/max, or spread questions for numeric or date fields. No findRecords aggregation needed.\n' +
      '  • string / boolean  → returns a list of all distinct values — use this to canonicalize filter values before querying.\n' +
      'Workflow for range/min/max questions: (1) findLabels, (2) findProperties to find the field and get its id and type, (3) call this tool with that id if type is number or datetime.',
    inputSchema: {
      type: 'object',
      properties: {
        propertyId: {
          type: 'string',
          description: 'The `id` field from the property object returned by findProperties.'
        },
        query: { type: 'string', description: 'Optional text filter — only applies to string properties.' },
        orderBy: { type: 'string', enum: ['asc', 'desc'], description: 'Sort order for string value lists.' },
        limit: { type: 'number', description: 'Max number of string values to return.' },
        skip: { type: 'number', description: 'Number of string values to skip.' }
      },
      required: ['propertyId']
    }
  },
  {
    name: 'findProperties',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Discover the field names, types, and IDs available on a record label. ' +
      'Always call this before using field names in any query — never guess or invent field names. ' +
      'Filter by label using: where: { label: { $in: ["LABEL_NAME"] } }. ' +
      'Each returned property object has: id (string), name (string), type (string | number | boolean | datetime | vector | null). ' +
      'Use the `name` field as the field name in where/orderBy/groupBy clauses. ' +
      'Use the `id` field as the `propertyId` argument to propertyValues. ' +
      'After calling this tool, decide the next step based on the field type: ' +
      'number or datetime → call propertyValues(propertyId) to get { min, max } for range questions, OR use findRecords with aggregate fn:min/max; ' +
      'string or boolean → call propertyValues(propertyId) to get distinct values before filtering.',
    inputSchema: {
      type: 'object',
      properties: {
        where: { type: 'object', description: 'Search conditions for finding properties' },
        limit: { type: 'number', description: 'Maximum number of properties to return', default: 10 },
        skip: { type: 'number', description: 'Number of properties to skip', default: 0 },
        orderBy: {
          type: 'object',
          description: 'Sorting configuration: key = field, value = asc|desc',
          additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
        }
      },
      required: []
    }
  },
  {
    name: 'findPropertyById',
    annotations: READ_ONLY,
    securitySchemes: READ_SCHEMES,
    description:
      'Fetch the metadata (name, type, label) of a single property by its ID. ' +
      'Use when you already have a propertyId and need to re-confirm its type before calling propertyValues.',

    inputSchema: {
      type: 'object',
      properties: { propertyId: { type: 'string', description: 'ID of the property to retrieve' } },
      required: ['propertyId']
    }
  },
  {
    name: 'deleteProperty',
    annotations: DESTROY,
    securitySchemes: WRITE_SCHEMES,
    description:
      'Permanently delete a property and all its values from every record that has it. Irreversible. ' +
      'Confirm with the user before calling.',

    inputSchema: {
      type: 'object',
      properties: { propertyId: { type: 'string', description: 'ID of the property to delete' } },
      required: ['propertyId']
    }
  }
] as const satisfies Array<Tool>
