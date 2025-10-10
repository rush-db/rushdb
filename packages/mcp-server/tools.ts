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
  | 'FindLabels'
  | 'CreateRecord'
  | 'UpdateRecord'
  | 'DeleteRecord'
  | 'FindRecords'
  | 'GetRecord'
  | 'GetRecordsByIds'
  | 'AttachRelation'
  | 'DetachRelation'
  | 'FindRelationships'
  | 'BulkCreateRecords'
  | 'BulkDeleteRecords'
  | 'ExportRecords'
  | 'OpenBrowser'
  | 'HelpAddToClient'
  | 'GetQueryBuilderPrompt'
  | 'SetRecord'
  | 'FindOneRecord'
  | 'FindUniqRecord'
  | 'DeleteRecordById'
  | 'PropertyValues'
  | 'FindProperties'
  | 'FindPropertyById'
  | 'DeleteProperty'
  | 'TransactionBegin'
  | 'TransactionCommit'
  | 'TransactionRollback'
  | 'TransactionGet'
  | 'GetSettings'

type Tool = {
  name: ToolName
  description: string
  inputSchema: Schema
}

export const tools: Tool[] = [
  {
    name: 'FindLabels',
    description: 'Find / filter record labels (supports where, limit, skip, orderBy). Superset of GetLabels.',
    inputSchema: {
      type: 'object',
      properties: {
        where: {
          type: 'object',
          description: 'Filter conditions for labels (e.g., by activity flags, counts)'
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
    name: 'CreateRecord',
    description: 'Create a new record in the database',
    inputSchema: {
      type: 'object',
      properties: {
        label: { type: 'string', description: 'Label for the record' },
        data: { type: 'object', description: 'The record data to insert' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic creation' }
      },
      required: ['label', 'data']
    }
  },
  {
    name: 'UpdateRecord',
    description: 'Update an existing record (partial update)',
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
    name: 'DeleteRecord',
    description: 'Delete a record from the database (alias of DeleteRecordById)',
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
    name: 'FindRecords',
    description: 'Find records in the database using a search query',
    inputSchema: {
      type: 'object',
      properties: {
        labels: { type: 'array', items: { type: 'string' }, description: 'Filter by record labels' },
        where: { type: 'object', description: 'Search conditions for finding records' },
        limit: { type: 'number', description: 'Maximum number of records to return', default: 10 },
        skip: { type: 'number', description: 'Number of records to skip', default: 0 },
        orderBy: {
          type: 'object',
          description: 'Sorting configuration: key = field, value = asc|desc',
          additionalProperties: { type: 'string', enum: ['asc', 'desc'] }
        },
        aggregate: {
          type: 'object',
          description: 'Aggregation definitions (records only)',
          additionalProperties: {
            type: 'object',
            properties: {
              fn: {
                type: 'string',
                description: 'Aggregation function (count,sum,avg,min,max,timeBucket)'
              },
              field: { type: 'string', description: 'Field to aggregate' },
              alias: { type: 'string', description: 'Optional alias override' },
              granularity: {
                type: 'string',
                description: 'For timeBucket, the time granularity (e.g., day, week, month, quarter, year)'
              }
            },
            required: ['fn']
          }
        },
        groupBy: {
          type: 'array',
          items: { type: 'string' },
          description: 'Fields to group by (records only)'
        }
      },
      required: []
    }
  },
  {
    name: 'GetRecord',
    description: 'Get a specific record by ID',
    inputSchema: {
      type: 'object',
      properties: { recordId: { type: 'string', description: 'ID of the record to retrieve' } },
      required: ['recordId']
    }
  },
  {
    name: 'GetRecordsByIds',
    description: 'Get multiple records by their IDs',
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
    name: 'AttachRelation',
    description: 'Create a relationship between records (single or multiple targets)',
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
    name: 'DetachRelation',
    description: 'Remove a relationship between records (single or multiple targets)',
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
    name: 'FindRelationships',
    description: 'Find relationships in the database',
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
    name: 'BulkCreateRecords',
    description: 'Create multiple records in a single operation',
    inputSchema: {
      type: 'object',
      properties: {
        label: { type: 'string', description: 'Label for all records' },
        data: { type: 'array', items: { type: 'object' }, description: 'Array of record data to insert' },
        transactionId: { type: 'string', description: 'Optional transaction ID for atomic bulk creation' }
      },
      required: ['label', 'data']
    }
  },
  {
    name: 'BulkDeleteRecords',
    description: 'Delete multiple records matching a query',
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
    name: 'ExportRecords',
    description: 'Export records to CSV format',
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
    name: 'OpenBrowser',
    description: 'Open a web browser to a specific URL',
    inputSchema: {
      type: 'object',
      properties: { url: { type: 'string', description: 'The URL to open' } },
      required: ['url']
    }
  },
  {
    name: 'HelpAddToClient',
    description: 'Help the user add the RushDB MCP server to their MCP client',
    inputSchema: { type: 'object', properties: {}, required: [] }
  },
  {
    name: 'GetQueryBuilderPrompt',
    description:
      'Return the RushDB Query Builder system prompt. Use this if your MCP client does not support Prompts API.',
    inputSchema: { type: 'object', properties: {}, required: [] }
  },
  {
    name: 'SetRecord',
    description: 'Replace all fields of a record with provided values',
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
    name: 'FindOneRecord',
    description: 'Find a single record that matches the given search criteria',
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
    name: 'FindUniqRecord',
    description: 'Find a unique record that matches the given search criteria',
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
    name: 'DeleteRecordById',
    description: 'Delete a record by its ID',
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
    name: 'PropertyValues',
    description: 'Get values for a specific property',
    inputSchema: {
      type: 'object',
      properties: {
        propertyId: { type: 'string', description: 'ID of the property to get values for' },
        query: { type: 'string', description: 'Optional search query for filtering values' },
        orderBy: { type: 'string', enum: ['asc', 'desc'], description: 'Ordering for value results' },
        limit: { type: 'number', description: 'Max number of values to return' },
        skip: { type: 'number', description: 'Number of values to skip' }
      },
      required: ['propertyId']
    }
  },
  {
    name: 'FindProperties',
    description: 'Find properties in the database using a search query',
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
    name: 'FindPropertyById',
    description: 'Find a specific property by ID',
    inputSchema: {
      type: 'object',
      properties: { propertyId: { type: 'string', description: 'ID of the property to retrieve' } },
      required: ['propertyId']
    }
  },
  {
    name: 'DeleteProperty',
    description: 'Delete a property from the database',
    inputSchema: {
      type: 'object',
      properties: { propertyId: { type: 'string', description: 'ID of the property to delete' } },
      required: ['propertyId']
    }
  },
  {
    name: 'TransactionBegin',
    description: 'Begin a new database transaction',
    inputSchema: {
      type: 'object',
      properties: { ttl: { type: 'number', description: 'TTL in milliseconds' } },
      required: []
    }
  },
  {
    name: 'TransactionCommit',
    description: 'Commit a database transaction',
    inputSchema: {
      type: 'object',
      properties: { transactionId: { type: 'string', description: 'Transaction ID' } },
      required: ['transactionId']
    }
  },
  {
    name: 'TransactionRollback',
    description: 'Rollback a database transaction',
    inputSchema: {
      type: 'object',
      properties: { transactionId: { type: 'string', description: 'Transaction ID' } },
      required: ['transactionId']
    }
  },
  {
    name: 'TransactionGet',
    description: 'Get information about a transaction',
    inputSchema: {
      type: 'object',
      properties: { transactionId: { type: 'string', description: 'Transaction ID' } },
      required: ['transactionId']
    }
  },
  {
    name: 'GetSettings',
    description: 'Get the current database settings and configuration',
    inputSchema: { type: 'object', properties: {}, required: [] }
  }
] as const satisfies { name: ToolName; description: string; inputSchema: Schema }[]
