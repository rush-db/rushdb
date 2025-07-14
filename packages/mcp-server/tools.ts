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
  | 'GetLabels'
  | 'GetProperties'
  | 'CreateRecord'
  | 'UpdateRecord'
  | 'DeleteRecord'
  | 'FindRecords'
  | 'GetRecord'
  | 'AttachRelation'
  | 'DetachRelation'
  | 'FindRelations'
  | 'BulkCreateRecords'
  | 'BulkDeleteRecords'
  | 'ExportRecords'
  | 'OpenBrowser'
  | 'HelpAddToClient'
  | 'SetRecord'
  | 'FindOneRecord'
  | 'FindUniqRecord'
  | 'DeleteRecordById'
  | 'PropertyValues'
  | 'FindProperty'
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
    name: 'GetLabels',
    description: 'Get all record labels in the RushDB database',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'GetProperties',
    description: 'Get all properties in the RushDB database',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'CreateRecord',
    description: 'Create a new record in the database',
    inputSchema: {
      type: 'object',
      properties: {
        label: {
          type: 'string',
          description: 'Label for the record'
        },
        data: {
          type: 'object',
          description: 'The record data to insert'
        }
      },
      required: ['label', 'data']
    }
  },
  {
    name: 'UpdateRecord',
    description: 'Update an existing record in the database',
    inputSchema: {
      type: 'object',
      properties: {
        recordId: {
          type: 'string',
          description: 'ID of the record to update'
        },
        label: {
          type: 'string',
          description: 'Label for the record'
        },
        data: {
          type: 'object',
          description: 'The updated record data'
        }
      },
      required: ['recordId', 'label', 'data']
    }
  },
  {
    name: 'DeleteRecord',
    description: 'Delete a record from the database',
    inputSchema: {
      type: 'object',
      properties: {
        recordId: {
          type: 'string',
          description: 'ID of the record to delete'
        }
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
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter by record labels'
        },
        where: {
          type: 'object',
          description: 'Search conditions for finding records'
        },
        limit: {
          type: 'number',
          description: 'Maximum number of records to return',
          default: 10
        },
        skip: {
          type: 'number',
          description: 'Number of records to skip',
          default: 0
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
      properties: {
        recordId: {
          type: 'string',
          description: 'ID of the record to retrieve'
        }
      },
      required: ['recordId']
    }
  },
  {
    name: 'AttachRelation',
    description: 'Create a relationship between two records',
    inputSchema: {
      type: 'object',
      properties: {
        sourceId: {
          type: 'string',
          description: 'ID of the source record'
        },
        targetId: {
          type: 'string',
          description: 'ID of the target record'
        },
        relationType: {
          type: 'string',
          description: 'Type of the relationship'
        },
        direction: {
          type: 'string',
          enum: ['outgoing', 'incoming', 'bidirectional'],
          description: 'Direction of the relationship',
          default: 'outgoing'
        }
      },
      required: ['sourceId', 'targetId']
    }
  },
  {
    name: 'DetachRelation',
    description: 'Remove a relationship between two records',
    inputSchema: {
      type: 'object',
      properties: {
        sourceId: {
          type: 'string',
          description: 'ID of the source record'
        },
        targetId: {
          type: 'string',
          description: 'ID of the target record'
        },
        relationType: {
          type: 'string',
          description: 'Type of the relationship to remove'
        },
        direction: {
          type: 'string',
          enum: ['outgoing', 'incoming', 'bidirectional'],
          description: 'Direction of the relationship',
          default: 'outgoing'
        }
      },
      required: ['sourceId', 'targetId']
    }
  },
  {
    name: 'FindRelations',
    description: 'Find relationships in the database',
    inputSchema: {
      type: 'object',
      properties: {
        where: {
          type: 'object',
          description: 'Search conditions for finding relationships'
        },
        limit: {
          type: 'number',
          description: 'Maximum number of relationships to return',
          default: 10
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
        label: {
          type: 'string',
          description: 'Label for all records'
        },
        data: {
          type: 'array',
          description: 'Array of record data to insert',
          items: {
            type: 'object'
          }
        }
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
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter by record labels'
        },
        where: {
          type: 'object',
          description: 'Search conditions for records to delete'
        }
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
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter by record labels'
        },
        where: {
          type: 'object',
          description: 'Search conditions for records to export'
        },
        limit: {
          type: 'number',
          description: 'Maximum number of records to export'
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
      properties: {
        url: {
          type: 'string',
          description: 'The URL to open in the browser'
        }
      },
      required: ['url']
    }
  },
  {
    name: 'HelpAddToClient',
    description: 'Help the user add the RushDB MCP server to their MCP client',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  },
  {
    name: 'SetRecord',
    description: 'Set all fields of a record to the provided values (replaces existing values)',
    inputSchema: {
      type: 'object',
      properties: {
        recordId: {
          type: 'string',
          description: 'ID of the record to set'
        },
        label: {
          type: 'string',
          description: 'Label for the record'
        },
        data: {
          type: 'object',
          description: 'The new record data to set'
        }
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
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter by record labels'
        },
        where: {
          type: 'object',
          description: 'Search conditions for finding the record'
        }
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
        labels: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter by record labels'
        },
        where: {
          type: 'object',
          description: 'Search conditions for finding the unique record'
        }
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
        recordId: {
          type: 'string',
          description: 'ID of the record to delete'
        }
      },
      required: ['recordId']
    }
  },
  {
    name: 'PropertyValues',
    description: 'Get all values for a specific property',
    inputSchema: {
      type: 'object',
      properties: {
        propertyId: {
          type: 'string',
          description: 'ID of the property to get values for'
        },
        query: {
          type: 'string',
          description: 'Optional search query for filtering values'
        },
        orderBy: {
          type: 'string',
          enum: ['asc', 'desc'],
          description: 'Order direction for the values'
        },
        limit: {
          type: 'number',
          description: 'Maximum number of values to return'
        },
        skip: {
          type: 'number',
          description: 'Number of values to skip'
        }
      },
      required: ['propertyId']
    }
  },
  {
    name: 'FindProperty',
    description: 'Find properties in the database using a search query',
    inputSchema: {
      type: 'object',
      properties: {
        where: {
          type: 'object',
          description: 'Search conditions for finding properties'
        },
        limit: {
          type: 'number',
          description: 'Maximum number of properties to return',
          default: 10
        },
        skip: {
          type: 'number',
          description: 'Number of properties to skip',
          default: 0
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
      properties: {
        propertyId: {
          type: 'string',
          description: 'ID of the property to retrieve'
        }
      },
      required: ['propertyId']
    }
  },
  {
    name: 'DeleteProperty',
    description: 'Delete a property from the database',
    inputSchema: {
      type: 'object',
      properties: {
        propertyId: {
          type: 'string',
          description: 'ID of the property to delete'
        }
      },
      required: ['propertyId']
    }
  },
  {
    name: 'TransactionBegin',
    description: 'Begin a new database transaction',
    inputSchema: {
      type: 'object',
      properties: {
        ttl: {
          type: 'number',
          description: 'Time to live for the transaction in seconds'
        }
      },
      required: []
    }
  },
  {
    name: 'TransactionCommit',
    description: 'Commit a database transaction',
    inputSchema: {
      type: 'object',
      properties: {
        transactionId: {
          type: 'string',
          description: 'ID of the transaction to commit'
        }
      },
      required: ['transactionId']
    }
  },
  {
    name: 'TransactionRollback',
    description: 'Rollback a database transaction',
    inputSchema: {
      type: 'object',
      properties: {
        transactionId: {
          type: 'string',
          description: 'ID of the transaction to rollback'
        }
      },
      required: ['transactionId']
    }
  },
  {
    name: 'TransactionGet',
    description: 'Get information about a database transaction',
    inputSchema: {
      type: 'object',
      properties: {
        transactionId: {
          type: 'string',
          description: 'ID of the transaction to get information for'
        }
      },
      required: ['transactionId']
    }
  },
  {
    name: 'GetSettings',
    description: 'Get the current database settings and configuration',
    inputSchema: {
      type: 'object',
      properties: {},
      required: []
    }
  }
] as const satisfies {
  name: ToolName
  description: string
  inputSchema: Schema
}[]
