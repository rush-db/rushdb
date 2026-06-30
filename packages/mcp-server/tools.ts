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
  | 'getSchemaMarkdown'
  | 'getSchema'
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
  | 'listRelationshipPatterns'
  | 'analyzeRelationshipPatterns'
  | 'approveRelationshipPattern'
  | 'ignoreRelationshipPattern'
  | 'deleteRelationshipPattern'
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
  | 'findEmbeddingIndexes'
  | 'createEmbeddingIndex'
  | 'deleteEmbeddingIndex'
  | 'getEmbeddingIndexStats'
  | 'upsertEmbeddingVectors'
  | 'semanticSearch'

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
  outputSchema: Schema
  annotations: ToolAnnotations
  securitySchemes?: SecurityScheme[]
}

// Shared annotation presets
const READ_ONLY: ToolAnnotations = { readOnlyHint: true, openWorldHint: false, destructiveHint: false }
const WRITE: ToolAnnotations = { readOnlyHint: false, openWorldHint: false, destructiveHint: false }
const DESTROY: ToolAnnotations = { readOnlyHint: false, openWorldHint: false, destructiveHint: true }

const RECORD_SCHEMA: Schema = {
  type: 'object',
  description: 'A RushDB record containing system fields and user-defined properties.',
  properties: {
    __id: { type: 'string' },
    __label: { type: 'string' },
    __score: { type: 'number' },
    __proptypes: { type: 'object', additionalProperties: true }
  },
  additionalProperties: true
}

const NULLABLE_RECORD_SCHEMA: Schema = { anyOf: [RECORD_SCHEMA, { type: 'null' }] }

const PROPERTY_SCHEMA: Schema = {
  type: 'object',
  properties: {
    id: { type: 'string' },
    name: { type: 'string' },
    type: { type: ['string', 'null'] },
    metadata: { type: 'object', additionalProperties: true },
    values: { type: 'array', items: {} }
  },
  additionalProperties: true
}

const EMBEDDING_INDEX_SCHEMA: Schema = {
  type: 'object',
  properties: {
    id: { type: 'string' },
    projectId: { type: 'string' },
    label: { type: 'string' },
    propertyName: { type: 'string' },
    modelKey: { type: 'string' },
    sourceType: { type: 'string', enum: ['managed', 'external'] },
    similarityFunction: { type: 'string', enum: ['cosine', 'euclidean'] },
    dimensions: { type: 'number' },
    vectorPropertyName: { type: 'string' },
    enabled: { type: 'boolean' },
    status: { type: 'string' },
    createdAt: { type: 'string' },
    updatedAt: { type: 'string' }
  },
  additionalProperties: true
}

const RELATIONSHIP_PATTERN_ENDPOINT_SCHEMA: Schema = {
  type: 'object',
  properties: {
    label: { type: 'string' },
    key: { type: 'string' },
    where: { type: 'object', additionalProperties: true }
  },
  required: ['label'],
  additionalProperties: false
}

const RELATIONSHIP_PATTERN_SCHEMA: Schema = {
  type: 'object',
  properties: {
    id: { type: 'string' },
    status: { type: 'string', enum: ['suggested', 'approved', 'ignored', 'error'] },
    origin: { type: 'string', enum: ['llm', 'manual'] },
    source: RELATIONSHIP_PATTERN_ENDPOINT_SCHEMA,
    target: RELATIONSHIP_PATTERN_ENDPOINT_SCHEMA,
    direction: { type: 'string', enum: ['in', 'out'] },
    type: { type: 'string' },
    mode: { type: 'string', enum: ['join_pattern', 'retype_existing_relationship'] },
    confidence: { type: 'number' },
    rationale: { type: 'string' },
    sampleMatchCount: { type: 'number' },
    lastAppliedAt: { type: 'string' },
    lastAnalyzedAt: { type: 'string' },
    lastError: { type: 'string' },
    createdAt: { type: 'string' },
    updatedAt: { type: 'string' }
  },
  required: [
    'id',
    'status',
    'origin',
    'source',
    'target',
    'direction',
    'type',
    'mode',
    'confidence',
    'createdAt',
    'updatedAt'
  ],
  additionalProperties: false
}

const RELATIONSHIP_PATTERN_LIST_SCHEMA: Schema = {
  type: 'object',
  properties: {
    patterns: { type: 'array', items: RELATIONSHIP_PATTERN_SCHEMA },
    relationships: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          label: { type: 'string' },
          relationships: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                label: { type: 'string' },
                type: { type: 'string' },
                direction: { type: 'string' }
              },
              required: ['label', 'type', 'direction'],
              additionalProperties: false
            }
          }
        },
        required: ['label', 'relationships'],
        additionalProperties: false
      }
    },
    analysis: {
      type: 'object',
      properties: {
        status: { type: 'string' },
        requestedAt: { type: 'string' },
        notBefore: { type: 'string' },
        lastRunAt: { type: 'string' },
        lastError: { type: 'string' }
      },
      required: ['status'],
      additionalProperties: false
    }
  },
  required: ['patterns', 'relationships'],
  additionalProperties: false
}

const MESSAGE_SCHEMA: Schema = {
  type: 'object',
  properties: {
    success: { type: 'boolean' },
    message: { type: 'string' }
  },
  required: ['success', 'message'],
  additionalProperties: false
}

const CREATE_RECORD_OUTPUT_SCHEMA: Schema = {
  type: 'object',
  properties: {
    success: { type: 'boolean' },
    id: { type: 'string' },
    message: { type: 'string' },
    record: RECORD_SCHEMA
  },
  required: ['success', 'id', 'message', 'record'],
  additionalProperties: false
}

const WRITE_RECORD_OUTPUT_SCHEMA: Schema = {
  type: 'object',
  properties: {
    success: { type: 'boolean' },
    message: { type: 'string' },
    record: RECORD_SCHEMA
  },
  required: ['success', 'message', 'record'],
  additionalProperties: false
}

const RECORDS_OUTPUT_SCHEMA: Schema = {
  type: 'object',
  properties: {
    data: { type: 'array', items: RECORD_SCHEMA },
    total: { type: 'number' }
  },
  required: ['data', 'total'],
  additionalProperties: true
}

const outputSchemas = {
  getSchemaMarkdown: {
    type: 'object',
    properties: { markdown: { type: 'string' } },
    required: ['markdown'],
    additionalProperties: false
  },
  getSchema: {
    type: 'object',
    properties: { schema: { type: 'array', items: { type: 'object', additionalProperties: true } } },
    required: ['schema'],
    additionalProperties: false
  },
  findLabels: {
    type: 'object',
    properties: {
      labels: {
        type: 'array',
        items: {
          type: 'object',
          properties: { name: { type: 'string' }, count: { type: 'number' } },
          required: ['name', 'count'],
          additionalProperties: false
        }
      }
    },
    required: ['labels'],
    additionalProperties: false
  },
  createRecord: CREATE_RECORD_OUTPUT_SCHEMA,
  updateRecord: WRITE_RECORD_OUTPUT_SCHEMA,
  deleteRecord: MESSAGE_SCHEMA,
  findRecords: RECORDS_OUTPUT_SCHEMA,
  getRecord: { type: 'object', properties: { record: NULLABLE_RECORD_SCHEMA }, required: ['record'] },
  getRecordsByIds: RECORDS_OUTPUT_SCHEMA,
  attachRelation: MESSAGE_SCHEMA,
  detachRelation: MESSAGE_SCHEMA,
  findRelationships: {
    type: 'object',
    properties: {
      relationships: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            sourceId: { type: 'string' },
            sourceLabel: { type: 'string' },
            targetId: { type: 'string' },
            targetLabel: { type: 'string' },
            type: { type: 'string' },
            direction: { type: 'string' },
            properties: { type: 'object', additionalProperties: true }
          },
          additionalProperties: true
        }
      }
    },
    required: ['relationships'],
    additionalProperties: false
  },
  listRelationshipPatterns: RELATIONSHIP_PATTERN_LIST_SCHEMA,
  analyzeRelationshipPatterns: {
    type: 'object',
    properties: { queued: { type: 'boolean' } },
    required: ['queued'],
    additionalProperties: false
  },
  approveRelationshipPattern: {
    type: 'object',
    properties: { pattern: { anyOf: [RELATIONSHIP_PATTERN_SCHEMA, { type: 'null' }] } },
    required: ['pattern'],
    additionalProperties: false
  },
  ignoreRelationshipPattern: {
    type: 'object',
    properties: { pattern: { anyOf: [RELATIONSHIP_PATTERN_SCHEMA, { type: 'null' }] } },
    required: ['pattern'],
    additionalProperties: false
  },
  deleteRelationshipPattern: {
    type: 'object',
    properties: { deleted: { type: 'boolean' } },
    required: ['deleted'],
    additionalProperties: false
  },
  bulkCreateRecords: {
    type: 'object',
    properties: {
      message: { type: 'string' },
      ids: { type: 'array', items: { type: 'string' } },
      data: { type: 'array', items: RECORD_SCHEMA },
      total: { type: 'number' }
    },
    required: ['message', 'ids', 'data', 'total'],
    additionalProperties: false
  },
  bulkDeleteRecords: MESSAGE_SCHEMA,
  exportRecords: {
    type: 'object',
    properties: {
      csv: { type: 'string' },
      dateTime: { type: 'string' },
      message: { type: 'string' }
    },
    required: ['csv', 'dateTime', 'message'],
    additionalProperties: false
  },
  helpAddToClient: {
    type: 'object',
    properties: { instructions: { type: 'string' } },
    required: ['instructions'],
    additionalProperties: false
  },
  getQueryBuilderPrompt: {
    type: 'object',
    properties: { prompt: { type: 'string' } },
    required: ['prompt'],
    additionalProperties: false
  },
  getSearchQuerySpec: {
    type: 'object',
    properties: { spec: { type: 'string' } },
    required: ['spec'],
    additionalProperties: false
  },
  setRecord: WRITE_RECORD_OUTPUT_SCHEMA,
  findOneRecord: { type: 'object', properties: { record: NULLABLE_RECORD_SCHEMA }, required: ['record'] },
  findUniqRecord: { type: 'object', properties: { record: NULLABLE_RECORD_SCHEMA }, required: ['record'] },
  deleteRecordById: MESSAGE_SCHEMA,
  propertyValues: {
    type: 'object',
    properties: { propertyValues: { anyOf: [PROPERTY_SCHEMA, { type: 'null' }] } },
    required: ['propertyValues'],
    additionalProperties: false
  },
  findProperties: {
    type: 'object',
    properties: { properties: { type: 'array', items: PROPERTY_SCHEMA } },
    required: ['properties'],
    additionalProperties: false
  },
  findPropertyById: {
    type: 'object',
    properties: { property: { anyOf: [PROPERTY_SCHEMA, { type: 'null' }] } },
    required: ['property'],
    additionalProperties: false
  },
  deleteProperty: {
    type: 'object',
    properties: {
      success: { type: 'boolean' },
      message: { type: 'string' },
      property: PROPERTY_SCHEMA
    },
    required: ['success', 'message', 'property'],
    additionalProperties: false
  },
  findEmbeddingIndexes: {
    type: 'object',
    properties: { indexes: { type: 'array', items: EMBEDDING_INDEX_SCHEMA } },
    required: ['indexes'],
    additionalProperties: false
  },
  createEmbeddingIndex: {
    type: 'object',
    properties: { index: { anyOf: [EMBEDDING_INDEX_SCHEMA, { type: 'null' }] } },
    required: ['index'],
    additionalProperties: false
  },
  upsertEmbeddingVectors: {
    type: 'object',
    properties: {
      result: {
        anyOf: [
          {
            type: 'object',
            properties: {
              updated: { type: 'number' },
              requested: { type: 'number' }
            },
            required: ['updated', 'requested'],
            additionalProperties: true
          },
          { type: 'null' }
        ]
      }
    },
    required: ['result'],
    additionalProperties: false
  },
  deleteEmbeddingIndex: {
    type: 'object',
    properties: {
      result: {
        anyOf: [
          {
            type: 'object',
            properties: { deleted: { type: 'boolean' } },
            required: ['deleted'],
            additionalProperties: true
          },
          { type: 'null' }
        ]
      }
    },
    required: ['result'],
    additionalProperties: false
  },
  getEmbeddingIndexStats: {
    type: 'object',
    properties: {
      stats: {
        anyOf: [
          {
            type: 'object',
            properties: { totalRecords: { type: 'number' }, indexedRecords: { type: 'number' } },
            additionalProperties: true
          },
          { type: 'null' }
        ]
      }
    },
    required: ['stats'],
    additionalProperties: false
  },
  semanticSearch: RECORDS_OUTPUT_SCHEMA
} satisfies Record<ToolName, Schema>

// Read-only tools advertise both noauth and oauth2 so ChatGPT can call them
// anonymously and also offer linking for user-specific data.
const READ_SCHEMES: SecurityScheme[] = [{ type: 'noauth' }, { type: 'oauth2', scopes: ['records:read'] }]
const WRITE_SCHEMES: SecurityScheme[] = [{ type: 'oauth2', scopes: ['records:write'] }]

export const tools: Tool[] = (
  [
    {
      name: 'getSchemaMarkdown',
      annotations: READ_ONLY,
      securitySchemes: READ_SCHEMES,
      description:
        'STEP 0 — call this ONCE at the start of every conversation before constructing any query. ' +
        'Returns the complete graph schema as compact Markdown: all labels with record counts, ' +
        'all properties per label with their type and value ranges (min/max for numbers/datetimes, ' +
        'sample values for strings/booleans; array properties render as type[]), and all cross-label relationships with direction and edge property summaries. ' +
        'The Properties table includes a "Semantic Search" column: properties with an embedding index show ' +
        '`sourceType similarityFunction dimensionsd [status]` (e.g. `managed cosine 1536d [ready]`); ' +
        'others show `—`. A non-`—` value means the property is queryable with aiSemanticSearch. ' +
        'This single call replaces the need for separate findLabels + findProperties + findRelationships ' +
        'discovery calls. Use the result to determine exact label names (case-sensitive), field names, ' +
        'field types, and relationship patterns before building any findRecords query. ' +
        'Optionally pass `labels` array to narrow the output to specific labels. ' +
        'Pass `force: true` to bypass the 1-hour schema cache and force a fresh recalculation.',
      inputSchema: {
        type: 'object',
        properties: {
          labels: {
            type: 'array',
            items: { type: 'string' },
            description: 'Optional: scope schema to specific labels only. Leave empty to get all labels.'
          },
          force: {
            type: 'boolean',
            description: 'Pass true to bypass the 1-hour schema cache and force a fresh recalculation.'
          }
        },
        required: []
      }
    },
    {
      name: 'getSchema',
      annotations: READ_ONLY,
      securitySchemes: READ_SCHEMES,
      description:
        'Returns the same graph schema as getSchemaMarkdown but as structured JSON. ' +
        'Each item has: label (string), count (number), properties (array with id, name, type, optional isArray, ' +
        'recordsCount (number of records that carry this property), ' +
        'min/max for numbers/datetimes, values[] for strings/booleans, and an optional vectorIndexes array), ' +
        'and relationships (array with label, type, direction: in|out, count, and optional edge properties). ' +
        'vectorIndexes is non-empty when one or more embedding indexes exist for that property; ' +
        'each entry has: id, sourceType (managed|external), similarityFunction (cosine|euclidean), ' +
        'dimensions (number), status (pending|indexing|ready|error), modelKey. ' +
        'A non-empty vectorIndexes means the property is queryable with aiSemanticSearch. ' +
        'Use this when you need property `id` values to pass to propertyValues for deeper drill-down. ' +
        'For initial schema orientation, getSchemaMarkdown uses fewer tokens. ' +
        'Pass `force: true` to bypass the 1-hour schema cache and force a fresh recalculation.',
      inputSchema: {
        type: 'object',
        properties: {
          labels: {
            type: 'array',
            items: { type: 'string' },
            description: 'Optional: scope schema to specific labels only. Leave empty to get all labels.'
          },
          force: {
            type: 'boolean',
            description: 'Pass true to bypass the 1-hour schema cache and force a fresh recalculation.'
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
        'IMPORTANT: getSchemaMarkdown already returns ALL label names in STEP 0 — do NOT call findLabels as a substitute for it. ' +
        'Only call findLabels when you need label counts after applying record-scoped predicates, or getSchemaMarkdown was not called yet. ' +
        'Call with no arguments to list all labels. ' +
        '`where` is applied to Records, not label metadata; use getSchemaMarkdown to search label names. ' +
        'Returns objects with name (case-sensitive — use exact casing in all subsequent calls) and count (number of matching records). ' +
        'Pick the best matching label by: exact match > starts-with > substring > semantic similarity, preferring higher count on ties. ' +
        'State your label assumption briefly ("using DEAL for \'deals\'") and proceed without asking.',
      inputSchema: {
        type: 'object',
        properties: {
          where: {
            type: 'object',
            description: 'Record-scoped filter conditions applied before label counts are returned.'
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
        'Primary read/query/list/search tool for records. Use findRecords to find records, search records, list records, retrieve matching records, filter records, or answer questions from data. ' +
        'Use this instead of exportRecords unless the user explicitly asks for CSV/export/download. ' +
        'Search records with a structured SearchQuery. ' +
        'BEFORE building any query with dates, metrics, groupBy, relationship traversal, or vector search — call getSearchQuerySpec to load the complete syntax reference. ' +
        'INTENT: metrics/analytics request (count/total/sum/avg/breakdown/top N by metric) → MUST include select + groupBy. NEVER fetch raw records to count/sum manually. ' +
        'RESPONSE: { data:[...records], total:N } — for simple "how many" read total directly; no count select needed. ' +
        'HARD RULES: ' +
        '(1) NEVER set limit when select is present — restricts the record scan and produces mathematically wrong results. Omit limit for all metrics queries. ' +
        '(2) labels contains root records only; put related labels inside where traversal blocks with $alias when referenced. ' +
        '(3) groupBy never accepts alias-only values like "$record"; use "$record.name" or a select key. ' +
        '(4) Ambiguous/incomplete named references should use $contains on likely display fields, not exact equality. ' +
        '(5) Related-count rankings keep the requested parent/entity as root; count the related alias and order desc for most/more or asc for least/less/fewer.',

      inputSchema: {
        type: 'object',
        properties: {
          labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
          where: {
            type: 'object',
            description:
              'Filter conditions. Field names must match exactly what findProperties/getSchemaMarkdown returns. ' +
              'Exact match can be field: value or field: { $eq: value }. ' +
              'For incomplete named references, prefer $contains on display fields instead of exact equality. ' +
              'For the complete operator reference (string/number/boolean/datetime/vector/$eq/$exists/$type/logical/$alias/$relation/$id) call getSearchQuerySpec.'
          },
          limit: {
            type: 'number',
            description:
              'Max records for listing queries (default 10, max 1000). ' +
              'NEVER set when select is present — restricts the scan and produces wrong results.',
            default: 10
          },
          skip: { type: 'number', description: 'Number of records to skip', default: 0 },
          orderBy: {
            type: 'object',
            description: 'Sorting configuration: key = field, value = asc|desc',
            additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
          },
          select: {
            type: 'object',
            description:
              'Map of output-key → select expression. Supports: $sum, $avg, $min, $max, $count, $collect, $timeBucket, math, and references. ' +
              'See getSearchQuerySpec for full select/groupBy/collect/timeBucket reference.'
          },
          groupBy: {
            type: 'array',
            items: { type: 'string' },
            description:
              'Two modes: (A) Dimensional — "$alias.propertyName" strings, one row per distinct value; ' +
              '(B) Self-group — select key names, collapses to one row. ' +
              'Do not use alias-only values such as "$record" or "$related". Call getSearchQuerySpec for full reference.'
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
          properties: {
            type: 'object',
            description:
              'Optional user-defined properties to store on the relationship edge. Reserved RushDB/internal keys are rejected.',
            additionalProperties: true
          },
          transactionId: {
            type: 'string',
            description: 'Optional transaction ID for atomic relation creation'
          }
        },
        required: ['sourceId']
      }
    },
    {
      name: 'detachRelation',
      annotations: DESTROY,
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
          transactionId: {
            type: 'string',
            description: 'Optional transaction ID for atomic relation removal'
          }
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
        '(1) Multi-hop path discovery — fetch a sample record ID, then call findRelationships with source.where or target.where containing that ID to reveal adjacent labels; repeat to trace the full path before building a nested findRecords where clause. ' +
        '(2) Edge filtering — where applies to relationship type/properties; use source/target for endpoint record predicates. ' +
        'Does NOT support select or groupBy — use findRecords for metrics/analytics across related labels.',

      inputSchema: {
        type: 'object',
        properties: {
          where: {
            type: 'object',
            description:
              'Relationship-edge predicates. `type` filters the relationship type; other fields filter relationship properties.'
          },
          source: {
            type: 'object',
            description: 'Optional source endpoint filter: labels and/or record-scoped where predicates.',
            properties: {
              labels: { type: 'array', items: { type: 'string' } },
              where: { type: 'object', additionalProperties: true }
            },
            additionalProperties: false
          },
          target: {
            type: 'object',
            description: 'Optional target endpoint filter: labels and/or record-scoped where predicates.',
            properties: {
              labels: { type: 'array', items: { type: 'string' } },
              where: { type: 'object', additionalProperties: true }
            },
            additionalProperties: false
          },
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
      name: 'listRelationshipPatterns',
      annotations: READ_ONLY,
      securitySchemes: READ_SCHEMES,
      description:
        'List inferred relationship patterns for the current project. ' +
        'Returns suggestions and their lifecycle status, current schema relationship summaries, and the latest analysis status. ' +
        'Use this before approving, ignoring, or deleting a relationship pattern.',
      inputSchema: { type: 'object', properties: {}, required: [] }
    },
    {
      name: 'analyzeRelationshipPatterns',
      annotations: WRITE,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Queue schema analysis to generate relationship pattern suggestions for the current project. ' +
        'This may invoke the configured LLM. Poll listRelationshipPatterns to inspect completion status and suggestions.',
      inputSchema: { type: 'object', properties: {}, required: [] }
    },
    {
      name: 'approveRelationshipPattern',
      annotations: WRITE,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Approve and apply a suggested relationship pattern. ' +
        'Call listRelationshipPatterns first so the user can review the inferred source, target, direction, type, mode, and confidence.',
      inputSchema: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'ID of the suggested relationship pattern to approve.' }
        },
        required: ['id']
      }
    },
    {
      name: 'ignoreRelationshipPattern',
      annotations: WRITE,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Ignore a suggested relationship pattern without applying it. ' +
        'Call listRelationshipPatterns first so the user can review the inferred pattern.',
      inputSchema: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'ID of the suggested relationship pattern to ignore.' }
        },
        required: ['id']
      }
    },
    {
      name: 'deleteRelationshipPattern',
      annotations: DESTROY,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Delete a saved relationship pattern. Irreversible. Confirm with the user before calling. ' +
        'When deleteExisting is true, relationships previously materialized by this pattern are also removed.',
      inputSchema: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'ID of the relationship pattern to delete.' },
          deleteExisting: {
            type: 'boolean',
            description: 'Also remove relationships previously materialized by this pattern.'
          }
        },
        required: ['id']
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
              },
              skipEmptyValues: {
                type: 'boolean',
                description:
                  'When true, empty strings ("") and empty arrays ([]) are treated as unset and not stored. 0 and false are kept. Useful for imports where empty cells mean "no data". Defaults to false.'
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
        'Export matching records as a CSV file only when the user explicitly asks to export/download CSV. ' +
        'For normal reading, answering, listing, searching, or inspecting records, use findRecords instead. ' +
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
        'all select functions ($sum/$avg/$min/$max/$count/$collect/$timeBucket), both groupBy modes (dimensional + self-group), ' +
        'late-ordering rules, root-label vs related-label traversal rules, COLLECT nesting, limit rules by query mode, multi-hop path discovery, ' +
        'enum normalization, validation checklist, and annotated query examples. ' +
        'CALL THIS before building any findRecords query that involves dates, metrics, groupBy, relationship traversal, or vector search. ' +
        'Do not guess operator syntax — use this spec as the source of truth.',
      inputSchema: { type: 'object', properties: {}, required: [] }
    },
    {
      name: 'setRecord',
      annotations: DESTROY,
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
          orderBy: {
            type: 'string',
            enum: ['asc', 'desc'],
            description: 'Sort order for string value lists.'
          },
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
        'Filter by label using labels: ["LABEL_NAME"]. `where` is applied to Records, not property metadata. ' +
        'Each returned property object has: id (string), name (string), type (string | number | boolean | datetime), recordsCount (number of records that carry this property). ' +
        'Use the `name` field as the field name in where/orderBy/groupBy clauses. ' +
        'Use the `id` field as the `propertyId` argument to propertyValues. ' +
        'After calling this tool, decide the next step based on the field type: ' +
        'number or datetime → call propertyValues(propertyId) to get { min, max } for range questions, OR use findRecords with select: { min: ..., max: ... }; ' +
        'string or boolean → call propertyValues(propertyId) to get distinct values before filtering.',
      inputSchema: {
        type: 'object',
        properties: {
          where: {
            type: 'object',
            description: 'Record-scoped filter conditions applied before properties are returned.'
          },
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
        'Fetch the metadata (name, type, label, recordsCount) of a single property by its ID. ' +
        'Use when you already have a propertyId and need to re-confirm its type or cardinality before calling propertyValues.',

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
    },
    {
      name: 'findEmbeddingIndexes',
      annotations: READ_ONLY,
      securitySchemes: READ_SCHEMES,
      description:
        'List all embedding index policies configured for the current project. ' +
        'Each index entry contains: id, label, propertyName, modelKey, dimensions, enabled, status (pending|indexing|ready|error), createdAt, updatedAt. ' +
        'Call this before creating a new index to check if one already exists for the same label+propertyName.',
      inputSchema: { type: 'object', properties: {}, required: [] }
    },
    {
      name: 'createEmbeddingIndex',
      annotations: WRITE,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Create a new embedding index policy for a string property. ' +
        'For managed indexes (default), RushDB asynchronously embeds every existing value and keeps new values embedded on write. ' +
        'For external indexes (sourceType: "external"), the client supplies vectors via upsertEmbeddingVectors. ' +
        'Once the index status becomes "ready" (check with getEmbeddingIndexStats), use semanticSearch to query.',
      inputSchema: {
        type: 'object',
        properties: {
          label: { type: 'string', description: 'Record label to scope the index to (e.g. "Book", "Task").' },
          propertyName: {
            type: 'string',
            description: 'Name of the string property whose values will be embedded.'
          },
          sourceType: {
            type: 'string',
            enum: ['managed', 'external'],
            description:
              'Whether RushDB generates embeddings ("managed") or the client provides them ("external"). Defaults to "managed".'
          },
          similarityFunction: {
            type: 'string',
            enum: ['cosine', 'euclidean'],
            description: 'Similarity function for the vector index. Defaults to "cosine".'
          },
          dimensions: {
            type: 'number',
            description:
              'Vector dimensionality. Required for external indexes. For managed indexes, defaults to the server-configured embedding model dimensions.'
          }
        },
        required: ['label', 'propertyName']
      }
    },
    {
      name: 'upsertEmbeddingVectors',
      annotations: WRITE,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Write pre-computed embedding vectors to an external vector index for a set of records. ' +
        'Only valid for indexes with sourceType "external". ' +
        'Each vector must contain exactly as many dimensions as the index was created with. ' +
        'After upserting, call getEmbeddingIndexStats to check if all records are indexed (status becomes "ready").',
      inputSchema: {
        type: 'object',
        properties: {
          indexId: {
            type: 'string',
            description: 'ID of the external embedding index (from findEmbeddingIndexes).'
          },
          items: {
            type: 'array',
            description: 'Array of record-vector pairs to write.',
            items: {
              type: 'object',
              properties: {
                recordId: { type: 'string', description: 'The __id of the target record.' },
                vector: {
                  type: 'array',
                  items: { type: 'number' },
                  description: 'Pre-computed embedding vector. Length must match index dimensions.'
                }
              },
              required: ['recordId', 'vector']
            }
          }
        },
        required: ['indexId', 'items']
      }
    },
    {
      name: 'deleteEmbeddingIndex',
      annotations: DESTROY,
      securitySchemes: WRITE_SCHEMES,
      description:
        'Delete an embedding index policy by its ID and strip all stored embedding vectors for that index. Irreversible. ' +
        'Confirm with the user before calling. Use findEmbeddingIndexes to get the indexId.',
      inputSchema: {
        type: 'object',
        properties: {
          indexId: { type: 'string', description: 'ID of the embedding index to delete.' }
        },
        required: ['indexId']
      }
    },
    {
      name: 'getEmbeddingIndexStats',
      annotations: READ_ONLY,
      securitySchemes: READ_SCHEMES,
      description:
        'Return Neo4j-level statistics for an embedding index: totalRecords and indexedRecords. ' +
        'Use this to monitor backfill progress after creating an index — when indexedRecords === totalRecords the index is fully ready.',
      inputSchema: {
        type: 'object',
        properties: {
          indexId: { type: 'string', description: 'ID of the embedding index (from findEmbeddingIndexes).' }
        },
        required: ['indexId']
      }
    },
    {
      name: 'semanticSearch',
      annotations: READ_ONLY,
      securitySchemes: READ_SCHEMES,
      description:
        'Perform semantic (vector) similarity search over records whose `propertyName` has been indexed with createEmbeddingIndex. ' +
        'For managed indexes: provide a free-text `query` — RushDB embeds it and returns the most similar records ranked by similarity (__score). ' +
        'For external indexes: provide a `queryVector` (pre-computed number[]) instead of query text. ' +
        'Direct vector-index mode (fast, default): used when no `where` filter is supplied. ' +
        'Prefilter mode (exact, slower): activated when a `where` filter is supplied — candidates are first narrowed then ranked. ' +
        'Requires an embedding index in "ready" status for the given label+propertyName.',
      inputSchema: {
        type: 'object',
        properties: {
          propertyName: { type: 'string', description: 'Name of the indexed property to search against.' },
          query: {
            type: 'string',
            description:
              'Free-text query to embed and compare against stored vectors (managed indexes only). Mutually exclusive with queryVector.'
          },
          queryVector: {
            type: 'array',
            items: { type: 'number' },
            description:
              'Pre-computed embedding vector to search with (external indexes). Mutually exclusive with query. Length must match index dimensions.'
          },
          labels: {
            type: 'array',
            items: { type: 'string' },
            description:
              'One or more record labels to scope the search. The first label is used to resolve the embedding index.'
          },
          sourceType: {
            type: 'string',
            enum: ['managed', 'external'],
            description:
              'Select which index type to search. Required when both managed and external indexes exist for the same label+property.'
          },
          similarityFunction: {
            type: 'string',
            enum: ['cosine', 'euclidean'],
            description: 'Disambiguates when multiple indexes share the same label+property+sourceType.'
          },
          where: {
            type: 'object',
            description: 'Optional filter applied before scoring (activates prefilter mode).'
          },
          topK: {
            type: 'number',
            description:
              'Max candidates to fetch from the vector index (default 20, direct vector-index mode only).'
          },
          limit: { type: 'number', description: 'Maximum number of results to return (default 20).' },
          skip: { type: 'number', description: 'Number of results to skip for pagination (default 0).' }
        },
        required: ['propertyName', 'labels']
      }
    }
  ] as const satisfies Array<Omit<Tool, 'outputSchema'>>
).map((tool) => ({ ...tool, outputSchema: outputSchemas[tool.name] }))
