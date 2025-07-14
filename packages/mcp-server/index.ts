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

import { Server } from '@modelcontextprotocol/sdk/server/index.js'
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js'
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError
} from '@modelcontextprotocol/sdk/types.js'
import { ToolName, tools } from './tools.js'
import { GetLabels } from './tools/GetLabels.js'
import { GetProperties } from './tools/GetProperties.js'
import { CreateRecord } from './tools/CreateRecord.js'
import { UpdateRecord } from './tools/UpdateRecord.js'
import { DeleteRecord } from './tools/DeleteRecord.js'
import { FindRecords } from './tools/FindRecords.js'
import { GetRecord } from './tools/GetRecord.js'
import { AttachRelation } from './tools/AttachRelation.js'
import { DetachRelation } from './tools/DetachRelation.js'
import { FindRelations } from './tools/FindRelations.js'
import { BulkCreateRecords } from './tools/BulkCreateRecords.js'
import { BulkDeleteRecords } from './tools/BulkDeleteRecords.js'
import { ExportRecords } from './tools/ExportRecords.js'
import { OpenBrowser } from './tools/OpenBrowser.js'
import { HelpAddToClient } from './tools/HelpAddToClient.js'
import { SetRecord } from './tools/SetRecord.js'
import { FindOneRecord } from './tools/FindOneRecord.js'
import { FindUniqRecord } from './tools/FindUniqRecord.js'
import { DeleteRecordById } from './tools/DeleteRecordById.js'
import { PropertyValues } from './tools/PropertyValues.js'
import { FindProperty } from './tools/FindProperty.js'
import { FindPropertyById } from './tools/FindPropertyById.js'
import { DeleteProperty } from './tools/DeleteProperty.js'
import { TransactionBegin } from './tools/TransactionBegin.js'
import { TransactionCommit } from './tools/TransactionCommit.js'
import { TransactionRollback } from './tools/TransactionRollback.js'
import { TransactionGet } from './tools/TransactionGet.js'
import { GetSettings } from './tools/GetSettings.js'

const server = new Server(
  {
    name: 'rushdb-mcp-server',
    version: '1.0.0'
  },
  {
    capabilities: {
      tools: {
        list: true,
        call: true
      }
    }
  }
)

server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools
  }
})

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const toolName = request.params.name as ToolName
  const args = request.params.arguments || {}

  try {
    switch (toolName) {
      case 'GetLabels':
        const labels = await GetLabels()
        return {
          content: [
            {
              type: 'text',
              text:
                labels.length > 0 ?
                  labels.map((l) => `${l.name}: ${l.count} records`).join('\n')
                : 'No labels found'
            }
          ]
        }

      case 'GetProperties':
        const properties = await GetProperties()
        return {
          content: [
            {
              type: 'text',
              text: properties.length > 0 ? JSON.stringify(properties, null, 2) : 'No properties found'
            }
          ]
        }

      case 'CreateRecord':
        const createResult = await CreateRecord({
          label: args.label as string,
          data: args.data as Record<string, any>
        })
        return {
          content: [
            {
              type: 'text',
              text: `${createResult.message}\nID: ${createResult.id}`
            }
          ]
        }

      case 'UpdateRecord':
        const updateResult = await UpdateRecord({
          recordId: args.recordId as string,
          label: args.label as string,
          data: args.data as Record<string, any>
        })
        return {
          content: [
            {
              type: 'text',
              text: updateResult.message
            }
          ]
        }

      case 'DeleteRecord':
        const deleteResult = await DeleteRecord({
          recordId: args.recordId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: deleteResult.message
            }
          ]
        }

      case 'FindRecords':
        const foundRecords = await FindRecords({
          labels: args.labels as string[] | undefined,
          where: args.where as Record<string, any> | undefined,
          limit: args.limit as number | undefined,
          skip: args.skip as number | undefined
        })

        return {
          content: [
            {
              type: 'text',
              text:
                foundRecords.length === 0 ?
                  'No matching records found.'
                : JSON.stringify(foundRecords, null, 2)
            }
          ]
        }

      case 'GetRecord':
        const record = await GetRecord({
          recordId: args.recordId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify(record, null, 2)
            }
          ]
        }

      case 'AttachRelation':
        const attachResult = await AttachRelation({
          sourceId: args.sourceId as string,
          targetId: args.targetId as string,
          relationType: args.relationType as string | undefined,
          direction: args.direction as 'outgoing' | 'incoming' | 'bidirectional' | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: attachResult.message
            }
          ]
        }

      case 'DetachRelation':
        const detachResult = await DetachRelation({
          sourceId: args.sourceId as string,
          targetId: args.targetId as string,
          relationType: args.relationType as string | undefined,
          direction: args.direction as 'outgoing' | 'incoming' | 'bidirectional' | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: detachResult.message
            }
          ]
        }

      case 'FindRelations':
        const relations = await FindRelations({
          where: args.where as Record<string, any> | undefined,
          limit: args.limit as number | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: relations.length > 0 ? JSON.stringify(relations, null, 2) : 'No relations found'
            }
          ]
        }

      case 'BulkCreateRecords':
        const bulkCreateResult = await BulkCreateRecords({
          label: args.label as string,
          data: args.data as Record<string, any>[]
        })
        return {
          content: [
            {
              type: 'text',
              text: `${bulkCreateResult.message}\nIDs: ${bulkCreateResult.ids.join(', ')}`
            }
          ]
        }

      case 'BulkDeleteRecords':
        const bulkDeleteResult = await BulkDeleteRecords({
          labels: args.labels as string[] | undefined,
          where: args.where as Record<string, any>
        })
        return {
          content: [
            {
              type: 'text',
              text: bulkDeleteResult.message
            }
          ]
        }

      case 'ExportRecords':
        const exportResult = await ExportRecords({
          labels: args.labels as string[] | undefined,
          where: args.where as Record<string, any> | undefined,
          limit: args.limit as number | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: `Export completed at ${exportResult.dateTime}\n\n${exportResult.csv}`
            }
          ]
        }

      case 'OpenBrowser':
        const openBrowserResult = await OpenBrowser({
          url: args.url as string
        })
        return {
          content: [
            {
              type: 'text',
              text: openBrowserResult.message
            }
          ]
        }

      case 'HelpAddToClient':
        const helpAddToClientResult = await HelpAddToClient()
        return {
          content: [
            {
              type: 'text',
              text: helpAddToClientResult.instructions
            }
          ]
        }

      case 'SetRecord':
        const setResult = await SetRecord({
          recordId: args.recordId as string,
          label: args.label as string,
          data: args.data as Record<string, any>
        })
        return {
          content: [
            {
              type: 'text',
              text: setResult.message
            }
          ]
        }

      case 'FindOneRecord':
        const foundOneRecord = await FindOneRecord({
          labels: args.labels as string[] | undefined,
          where: args.where as Record<string, any> | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: foundOneRecord ? JSON.stringify(foundOneRecord, null, 2) : 'No matching record found.'
            }
          ]
        }

      case 'FindUniqRecord':
        const foundUniqRecord = await FindUniqRecord({
          labels: args.labels as string[] | undefined,
          where: args.where as Record<string, any> | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: foundUniqRecord ? JSON.stringify(foundUniqRecord, null, 2) : 'No unique record found.'
            }
          ]
        }

      case 'DeleteRecordById':
        const deleteByIdResult = await DeleteRecordById({
          recordId: args.recordId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: deleteByIdResult.message
            }
          ]
        }

      case 'PropertyValues':
        const propertyValues = await PropertyValues({
          propertyId: args.propertyId as string,
          query: args.query as string | undefined,
          orderBy: args.orderBy as 'asc' | 'desc' | undefined,
          limit: args.limit as number | undefined,
          skip: args.skip as number | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: propertyValues ? JSON.stringify(propertyValues, null, 2) : 'No property values found'
            }
          ]
        }

      case 'FindProperty':
        const foundProperties = await FindProperty({
          where: args.where as Record<string, any> | undefined,
          limit: args.limit as number | undefined,
          skip: args.skip as number | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text:
                foundProperties.length > 0 ? JSON.stringify(foundProperties, null, 2) : 'No properties found'
            }
          ]
        }

      case 'FindPropertyById':
        const foundProperty = await FindPropertyById({
          propertyId: args.propertyId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: foundProperty ? JSON.stringify(foundProperty, null, 2) : 'Property not found'
            }
          ]
        }

      case 'DeleteProperty':
        const deletePropertyResult = await DeleteProperty({
          propertyId: args.propertyId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: deletePropertyResult.message
            }
          ]
        }

      case 'TransactionBegin':
        const beginResult = await TransactionBegin({
          ttl: args.ttl as number | undefined
        })
        return {
          content: [
            {
              type: 'text',
              text: `${beginResult.message}\nTransaction ID: ${beginResult.transactionId}`
            }
          ]
        }

      case 'TransactionCommit':
        const commitResult = await TransactionCommit({
          transactionId: args.transactionId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: commitResult.message
            }
          ]
        }

      case 'TransactionRollback':
        const rollbackResult = await TransactionRollback({
          transactionId: args.transactionId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: rollbackResult.message
            }
          ]
        }

      case 'TransactionGet':
        const transactionInfo = await TransactionGet({
          transactionId: args.transactionId as string
        })
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify(transactionInfo, null, 2)
            }
          ]
        }

      case 'GetSettings':
        const settings = await GetSettings()
        return {
          content: [
            {
              type: 'text',
              text: JSON.stringify(settings, null, 2)
            }
          ]
        }

      default:
        throw new McpError(ErrorCode.MethodNotFound, 'Tool not found')
    }
  } catch (error) {
    console.error('Error executing tool:', error)

    // Check if error is related to API endpoint or missing env vars
    if (
      error instanceof Error &&
      (error.message.includes('RUSHDB_API_KEY') ||
        error.message.includes('Invalid URL') ||
        error.message.includes('Failed to fetch') ||
        error.message.includes('Network error') ||
        !process.env.RUSHDB_API_KEY)
    ) {
      // Open browser for configuration
      return {
        content: [
          {
            type: 'text',
            text: "It seems like you haven't configured your RushDB credentials. Would you like me to open the RushDB dashboard for you so you can sign up and get your credentials?"
          }
        ]
      }
    }

    // For other errors, return the error message
    return {
      content: [
        {
          type: 'text',
          text: `Error: ${error instanceof Error ? error.message : String(error)}`
        }
      ]
    }
  }
})

const transport = new StdioServerTransport()
await server.connect(transport)
