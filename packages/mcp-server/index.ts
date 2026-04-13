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
import { WebStandardStreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/webStandardStreamableHttp.js'
import {
  CallToolRequestSchema,
  ErrorCode,
  ListToolsRequestSchema,
  McpError,
  ListPromptsRequestSchema,
  GetPromptRequestSchema
} from '@modelcontextprotocol/sdk/types.js'
import { ToolName, tools } from './tools.js'
import { findLabels } from './tools/findLabels.js'
import { createRecord } from './tools/createRecord.js'
import { updateRecord } from './tools/updateRecord.js'
import { deleteRecord } from './tools/deleteRecord.js'
import { findRecords } from './tools/findRecords.js'
import { getRecord } from './tools/getRecord.js'
import { attachRelation } from './tools/attachRelation.js'
import { detachRelation } from './tools/detachRelation.js'
import { findRelationships } from './tools/findRelationships.js'
import { bulkCreateRecords } from './tools/bulkCreateRecords.js'
import { bulkDeleteRecords } from './tools/bulkDeleteRecords.js'
import { exportRecords } from './tools/exportRecords.js'
import { helpAddToClient } from './tools/helpAddToClient.js'
import { setRecord } from './tools/setRecord.js'
import { findOneRecord } from './tools/findOneRecord.js'
import { findUniqRecord } from './tools/findUniqRecord.js'
import { deleteRecordById } from './tools/deleteRecordById.js'
import { propertyValues } from './tools/propertyValues.js'
import { findProperties } from './tools/findProperties.js'
import { findPropertyById } from './tools/findPropertyById.js'
import { deleteProperty } from './tools/deleteProperty.js'
import { getRecordsByIds } from './tools/getRecordsByIds.js'
import { findEmbeddingIndexes } from './tools/findEmbeddingIndexes.js'
import { createEmbeddingIndex } from './tools/createEmbeddingIndex.js'
import { deleteEmbeddingIndex } from './tools/deleteEmbeddingIndex.js'
import { getEmbeddingIndexStats } from './tools/getEmbeddingIndexStats.js'
import { upsertEmbeddingVectors } from './tools/upsertEmbeddingVectors.js'
import { semanticSearch } from './tools/semanticSearch.js'
import { getOntology } from './tools/getOntology.js'
import { getOntologyMarkdown } from './tools/getOntologyMarkdown.js'
import SYSTEM_PROMPT from './systemPrompt.js'
import { getSearchQuerySpec } from './tools/getSearchQuerySpec.js'
import { requestContext, RequestContext } from './util/db.js'
import { resolveRequestContext, makeMcpAuthError } from './util/auth.js'

// ─── MCP Server factory ───────────────────────────────────────────────────────
// Each call creates an independent MCP Server instance with all handlers wired.
// In STDIO mode a single server is created once.
// In HTTP mode a new server is created per request (required because Server
// supports only one transport at a time).

function createMcpServer(): Server {
  const server = new Server(
    {
      name: 'rushdb-mcp-server',
      version: '1.0.0'
    },
    {
      // `instructions` is sent in the MCP initialize response.
      // Conforming clients (ChatGPT, Claude Desktop, etc.) inject this as
      // system-level context before any tool call — it is the canonical way
      // to deliver server-wide behavioural rules to the model automatically.
      instructions: SYSTEM_PROMPT,
      capabilities: {
        tools: {},
        prompts: {}
      }
    }
  )

  server.setRequestHandler(ListToolsRequestSchema, async () => {
    return {
      tools
    }
  })

  // Expose RushDB system prompt via MCP Prompts so clients can fetch and inject it
  server.setRequestHandler(ListPromptsRequestSchema, async () => {
    return {
      prompts: [
        {
          name: 'rushdb.queryBuilder',
          description:
            'RushDB Query Builder system prompt: guides the model to discover labels/properties first and construct validated SearchQuery objects before calling find-related tools.',
          arguments: []
        }
      ]
    }
  })

  server.setRequestHandler(GetPromptRequestSchema, async (request: any) => {
    const name = request.params.name as string
    if (name !== 'rushdb.queryBuilder') {
      throw new McpError(ErrorCode.InvalidRequest, `Unknown prompt: ${name}`)
    }
    return {
      description:
        'RushDB Query Builder system prompt to enable discovery-first, schema-safe SearchQuery construction before find-related tool calls.',
      messages: [
        {
          role: 'user',
          content: {
            type: 'text',
            text: SYSTEM_PROMPT
          }
        }
      ]
    }
  })

  server.setRequestHandler(CallToolRequestSchema, async (request: any) => {
    const toolName = request.params.name as ToolName
    const args = request.params.arguments || {}

    try {
      switch (toolName) {
        case 'getOntologyMarkdown': {
          const md = await getOntologyMarkdown({
            labels: args.labels as string[] | undefined,
            force: args.force as boolean | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: md ?? 'No ontology data found.'
              }
            ]
          }
        }

        case 'getOntology': {
          const ontology = await getOntology({
            labels: args.labels as string[] | undefined,
            force: args.force as boolean | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: ontology ? JSON.stringify(ontology, null, 2) : 'No ontology data found.'
              }
            ]
          }
        }

        case 'findLabels':
          const foundLabels = await findLabels({
            where: args.where as Record<string, any> | undefined,
            limit: args.limit as number | undefined,
            skip: args.skip as number | undefined,
            orderBy: args.orderBy as Record<string, 'asc' | 'desc'> | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text:
                  foundLabels.length > 0 ?
                    foundLabels.map((l: any) => `${l.name}: ${l.count} records`).join('\n')
                  : 'No labels found'
              }
            ]
          }

        case 'createRecord':
          const createResult = await createRecord({
            label: args.label as string,
            data: args.data as Record<string, any>,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: `${createResult.message}\nID: ${createResult.id}`
              }
            ]
          }

        case 'updateRecord':
          const updateResult = await updateRecord({
            recordId: args.recordId as string,
            label: args.label as string,
            data: args.data as Record<string, any>,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: updateResult.message
              }
            ]
          }

        case 'deleteRecord':
          const deleteResult = await deleteRecord({
            recordId: args.recordId as string,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: deleteResult.message
              }
            ]
          }

        case 'findRecords':
          const foundRecords = await findRecords({
            labels: args.labels as string[] | undefined,
            where: args.where as Record<string, any> | undefined,
            limit: args.limit as number | undefined,
            skip: args.skip as number | undefined,
            orderBy: args.orderBy as Record<string, 'asc' | 'desc'> | undefined,
            aggregate: args.aggregate as
              | Record<string, { fn: string; field?: string; alias?: string; where?: any }>
              | undefined,
            groupBy: args.groupBy as string[] | undefined
          })

          const isAggregate = Boolean(args.aggregate) || Boolean(args.groupBy)
          const isEmpty =
            isAggregate ? false : (
              Array.isArray((foundRecords as any)?.data) && (foundRecords as any).data.length === 0
            )
          return {
            content: [
              {
                type: 'text',
                text: isEmpty ? 'No matching records found.' : JSON.stringify(foundRecords, null, 2)
              }
            ]
          }

        case 'getRecord':
          const record = await getRecord({
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

        case 'attachRelation':
          const attachResult = await attachRelation({
            sourceId: args.sourceId as string,
            targetId: args.targetId as string | undefined,
            targetIds: args.targetIds as string[] | undefined,
            relationType: args.relationType as string | undefined,
            direction: args.direction as 'outgoing' | 'incoming' | 'bidirectional' | undefined,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: attachResult.message
              }
            ]
          }

        case 'detachRelation':
          const detachResult = await detachRelation({
            sourceId: args.sourceId as string,
            targetId: args.targetId as string | undefined,
            targetIds: args.targetIds as string[] | undefined,
            relationType: args.relationType as string | undefined,
            direction: args.direction as 'outgoing' | 'incoming' | 'bidirectional' | undefined,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: detachResult.message
              }
            ]
          }

        case 'findRelationships':
          const relations = await findRelationships({
            where: args.where as Record<string, any> | undefined,
            limit: args.limit as number | undefined,
            skip: args.skip as number | undefined,
            orderBy: args.orderBy as Record<string, 'asc' | 'desc'> | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: relations.length > 0 ? JSON.stringify(relations, null, 2) : 'No relations found'
              }
            ]
          }

        case 'bulkCreateRecords':
          const bulkCreateResult = await bulkCreateRecords({
            label: args.label as string,
            data: args.data as Record<string, any>[],
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: `${bulkCreateResult.message}\nIDs: ${bulkCreateResult.ids.join(', ')}`
              }
            ]
          }

        case 'bulkDeleteRecords':
          const bulkDeleteResult = await bulkDeleteRecords({
            labels: args.labels as string[] | undefined,
            where: args.where as Record<string, any>,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: bulkDeleteResult.message
              }
            ]
          }

        case 'exportRecords':
          const exportResult = await exportRecords({
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

        case 'helpAddToClient':
          const helpAddToClientResult = await helpAddToClient()
          return {
            content: [
              {
                type: 'text',
                text: helpAddToClientResult.instructions
              }
            ]
          }

        case 'getQueryBuilderPrompt':
          return {
            content: [
              {
                type: 'text',
                text: SYSTEM_PROMPT
              }
            ]
          }

        case 'getSearchQuerySpec': {
          const spec = await getSearchQuerySpec()
          return {
            content: [
              {
                type: 'text',
                text: spec.spec
              }
            ]
          }
        }

        case 'setRecord':
          const setResult = await setRecord({
            recordId: args.recordId as string,
            label: args.label as string,
            data: args.data as Record<string, any>,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: setResult.message
              }
            ]
          }

        case 'findOneRecord':
          const foundOneRecord = await findOneRecord({
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

        case 'findUniqRecord':
          const foundUniqRecord = await findUniqRecord({
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

        case 'deleteRecordById':
          const deleteByIdResult = await deleteRecordById({
            recordId: args.recordId as string,
            transactionId: args.transactionId as string | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: deleteByIdResult.message
              }
            ]
          }

        case 'propertyValues': {
          const pvResult = await propertyValues({
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
                text: pvResult ? JSON.stringify(pvResult, null, 2) : 'No property values found'
              }
            ]
          }
        }

        case 'findProperties':
          const foundProperties = await findProperties({
            where: args.where as Record<string, any> | undefined,
            limit: args.limit as number | undefined,
            skip: args.skip as number | undefined,
            orderBy: args.orderBy as Record<string, 'asc' | 'desc'> | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text:
                  foundProperties.length > 0 ?
                    JSON.stringify(foundProperties, null, 2)
                  : 'No properties found'
              }
            ]
          }
        case 'getRecordsByIds':
          const recordsByIds = await getRecordsByIds({ recordIds: args.recordIds as string[] })
          return {
            content: [
              {
                type: 'text',
                text: recordsByIds.count > 0 ? JSON.stringify(recordsByIds.data, null, 2) : 'No records found'
              }
            ]
          }

        case 'findPropertyById':
          const foundProperty = await findPropertyById({
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

        case 'deleteProperty':
          const deletePropertyResult = await deleteProperty({
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

        case 'findEmbeddingIndexes': {
          const indexes = await findEmbeddingIndexes()
          return {
            content: [
              {
                type: 'text',
                text:
                  indexes && indexes.length > 0 ?
                    JSON.stringify(indexes, null, 2)
                  : 'No embedding indexes found'
              }
            ]
          }
        }

        case 'createEmbeddingIndex': {
          const newIndex = await createEmbeddingIndex({
            label: args.label as string,
            propertyName: args.propertyName as string,
            sourceType: args.sourceType as 'managed' | 'external' | undefined,
            similarityFunction: args.similarityFunction as 'cosine' | 'euclidean' | undefined,
            dimensions: args.dimensions as number | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text: newIndex ? JSON.stringify(newIndex, null, 2) : 'Embedding index created'
              }
            ]
          }
        }

        case 'deleteEmbeddingIndex': {
          const deleteIndexResult = await deleteEmbeddingIndex({
            indexId: args.indexId as string
          })
          return {
            content: [
              {
                type: 'text',
                text:
                  deleteIndexResult ? JSON.stringify(deleteIndexResult, null, 2) : 'Embedding index deleted'
              }
            ]
          }
        }

        case 'getEmbeddingIndexStats': {
          const stats = await getEmbeddingIndexStats({
            indexId: args.indexId as string
          })
          return {
            content: [
              {
                type: 'text',
                text: stats ? JSON.stringify(stats, null, 2) : 'No stats available'
              }
            ]
          }
        }

        case 'upsertEmbeddingVectors': {
          const upsertResult = await upsertEmbeddingVectors({
            indexId: args.indexId as string,
            items: args.items as Array<{ recordId: string; vector: number[] }>
          })
          return {
            content: [
              {
                type: 'text',
                text: upsertResult ? JSON.stringify(upsertResult, null, 2) : 'Vectors upserted'
              }
            ]
          }
        }

        case 'semanticSearch': {
          const searchResults = await semanticSearch({
            propertyName: args.propertyName as string,
            query: args.query as string | undefined,
            queryVector: args.queryVector as number[] | undefined,
            labels: args.labels as string[],
            sourceType: args.sourceType as 'managed' | 'external' | undefined,
            similarityFunction: args.similarityFunction as 'cosine' | 'euclidean' | undefined,
            dimensions: args.dimensions as number | undefined,
            where: args.where as Record<string, unknown> | undefined,
            topK: args.topK as number | undefined,
            limit: args.limit as number | undefined,
            skip: args.skip as number | undefined
          })
          return {
            content: [
              {
                type: 'text',
                text:
                  searchResults && searchResults.length > 0 ?
                    JSON.stringify(searchResults, null, 2)
                  : 'No matching records found.'
              }
            ]
          }
        }

        default:
          throw new McpError(ErrorCode.MethodNotFound, 'Tool not found')
      }
    } catch (error) {
      console.error('Error executing tool:', error)

      if (error instanceof McpError) {
        throw error
      }

      const message = error instanceof Error ? error.message : String(error)

      // Check if error is related to API endpoint or missing env vars
      if (
        message.includes('RUSHDB_API_KEY') ||
        message.includes('Invalid URL') ||
        message.includes('Failed to fetch') ||
        message.includes('Network error') ||
        !process.env.RUSHDB_API_KEY
      ) {
        return {
          content: [
            {
              type: 'text',
              text: "It seems like you haven't configured your RushDB credentials. Would you like me to open the RushDB dashboard for you so you can sign up and get your credentials?"
            }
          ]
        }
      }

      // Map raw HTTP status codes from the SDK fetcher into actionable messages
      const httpStatus = /^(\d{3})$/.exec(message.trim())?.[1]
      if (httpStatus) {
        const status = Number(httpStatus)
        let hint: string
        if (status === 400) {
          hint =
            'Bad request (400): the query or payload is invalid. Check field names, operators, and required arguments. Call getSearchQuerySpec for the correct SearchQuery syntax.'
        } else if (status === 401) {
          hint =
            'Unauthorized (401): the API key is missing or invalid. Ask the user to verify RUSHDB_API_KEY.'
        } else if (status === 403) {
          hint = 'Forbidden (403): access denied. The API key does not have permission for this operation.'
        } else if (status === 404) {
          hint =
            'Not found (404): the requested resource does not exist. Verify that record IDs, project IDs, and label names are correct (labels are case-sensitive). Call getOntologyMarkdown to rediscover the available schema.'
        } else if (status === 409) {
          hint =
            'Conflict (409): the operation conflicts with existing data. Check for duplicate keys or conflicting constraints.'
        } else if (status === 422) {
          hint =
            'Unprocessable entity (422): the server could not process the request. Check field types and required fields match the schema.'
        } else if (status >= 500) {
          hint = `Server error (${status}): an unexpected error occurred on the RushDB server. Retry the operation or contact support if it persists.`
        } else {
          hint = `HTTP error ${status} from RushDB API.`
        }
        return {
          content: [{ type: 'text', text: hint }]
        }
      }

      // Generic error fallback
      return {
        content: [
          {
            type: 'text',
            text: `Error: ${message}`
          }
        ]
      }
    }
  })

  return server
} // end createMcpServer()

// ─── Launch ───────────────────────────────────────────────────────────────────

const mcpTransport = process.env.MCP_TRANSPORT || 'stdio'

if (mcpTransport === 'http') {
  // ── HTTP / OAuth mode ──
  // Uses Hono + StreamableHTTPServerTransport.
  // Each request gets its own MCP Server instance + per-request RushDB client.
  const { Hono } = await import('hono')
  const { serve } = await import('@hono/node-server')

  const httpApp = new Hono()
  const port = Number(process.env.PORT ?? 3001)
  const resourceUrl = process.env.MCP_RESOURCE_URL || `http://localhost:${port}`
  const oauthIssuer = process.env.RUSHDB_OAUTH_ISSUER || 'https://api.rushdb.com'

  // Proxy all OAuth endpoints (authorize, token, etc.) through to platform/core,
  // rewriting Location headers and JSON bodies so redirect URLs stay on the tunnel.
  // RUSHDB_OAUTH_PRODUCTION_ISSUER lets you override the "real" public issuer URL
  // that the platform might embed in its own discovery docs (defaults to api.rushdb.com).
  const productionIssuer = process.env.RUSHDB_OAUTH_PRODUCTION_ISSUER || 'https://api.rushdb.com'
  const rewriteUpstream = (s: string) =>
    s.replaceAll(oauthIssuer, resourceUrl).replaceAll(productionIssuer, resourceUrl)

  // Fetch and cache the proxied oauth-authorization-server doc (rewritten to resourceUrl).
  // Used both for the oauth-authorization-server endpoint AND to synthesize
  // /.well-known/openid-configuration (which platform/core doesn't implement).
  let cachedAuthServerMeta: Record<string, any> | null = null
  const getAuthServerMeta = async (): Promise<Record<string, any>> => {
    if (cachedAuthServerMeta) return cachedAuthServerMeta
    const res = await fetch(`${oauthIssuer}/.well-known/oauth-authorization-server`)
    if (!res.ok) throw new Error(`upstream ${res.status}`)
    const data = await res.json()
    const upstreamIssuer: string = (data as any).issuer || oauthIssuer
    cachedAuthServerMeta = JSON.parse(
      rewriteUpstream(JSON.stringify(data).replaceAll(upstreamIssuer, resourceUrl))
    )
    return cachedAuthServerMeta!
  }

  // /.well-known/oauth-authorization-server — proxy + rewrite
  const serveAuthServerMeta = async (c: any) => {
    try {
      return c.json(await getAuthServerMeta())
    } catch {
      return c.json({ error: 'upstream_unavailable' }, 502 as any)
    }
  }

  // /.well-known/openid-configuration — synthesized from oauth-authorization-server
  // (platform/core doesn't expose this endpoint but ChatGPT requires it to discover
  // the registration_endpoint for RFC 7591 Dynamic Client Registration).
  const serveOpenIdConfig = async (c: any) => {
    try {
      const as = await getAuthServerMeta()
      const oidc: Record<string, any> = {
        issuer: as.issuer ?? resourceUrl,
        authorization_endpoint: as.authorization_endpoint,
        token_endpoint: as.token_endpoint,
        jwks_uri: as.jwks_uri,
        registration_endpoint: as.registration_endpoint,
        scopes_supported: as.scopes_supported,
        response_types_supported: as.response_types_supported ?? ['code'],
        grant_types_supported: as.grant_types_supported,
        code_challenge_methods_supported: as.code_challenge_methods_supported ?? ['S256'],
        token_endpoint_auth_methods_supported: as.token_endpoint_auth_methods_supported ?? ['none'],
        subject_types_supported: ['public'],
        id_token_signing_alg_values_supported: ['RS256'],
        service_documentation: as.service_documentation
      }
      // remove undefined values
      Object.keys(oidc).forEach((k) => oidc[k] === undefined && delete oidc[k])
      return c.json(oidc)
    } catch {
      return c.json({ error: 'upstream_unavailable' }, 502 as any)
    }
  }

  const proxyOAuthEndpoint = async (c: any) => {
    const incomingUrl = new URL(c.req.url)
    const upstreamUrl = new URL(`${oauthIssuer}${c.req.path}`)
    incomingUrl.searchParams.forEach((v, k) => upstreamUrl.searchParams.set(k, v))

    const method = c.req.method as string
    const incomingHeaders = Object.fromEntries(
      [...c.req.raw.headers.entries()].filter(([k]) => {
        const lower = k.toLowerCase()
        // Drop hop-by-hop and host headers that would confuse the upstream
        return ![
          'host',
          'connection',
          'keep-alive',
          'proxy-authorization',
          'te',
          'trailers',
          'transfer-encoding',
          'upgrade'
        ].includes(lower)
      })
    )
    const upstreamRes = await fetch(upstreamUrl.toString(), {
      method,
      headers: incomingHeaders,
      body: method !== 'GET' && method !== 'HEAD' ? await c.req.text() : undefined,
      redirect: 'manual'
    })

    const responseText = rewriteUpstream(await upstreamRes.text())
    const headers: Record<string, string> = {}
    upstreamRes.headers.forEach((v, k) => {
      const lower = k.toLowerCase()
      // Strip content-length: rewriteUpstream changes body size; let the runtime recompute it.
      // Strip transfer-encoding: chunked encoding from upstream must not propagate.
      if (lower === 'content-length' || lower === 'transfer-encoding') return
      headers[k] = lower === 'location' ? rewriteUpstream(v) : v
    })
    return new Response(responseText || null, { status: upstreamRes.status, headers })
  }

  // OAuth / OpenID discovery — intercept ANY path ending in a well-known suffix
  // before the /oauth/* catch-all claims it. This handles variants like:
  //   /.well-known/openid-configuration
  //   /mcp/.well-known/openid-configuration
  //   /oauth/token/.well-known/openid-configuration  ← ChatGPT appends to token_endpoint
  //   /.well-known/oauth-authorization-server
  //   /oauth/token/.well-known/oauth-authorization-server
  //   etc.
  httpApp.use('*', async (c, next) => {
    const p = c.req.path
    if (c.req.method === 'GET') {
      if (p.endsWith('/.well-known/openid-configuration') || p === '/.well-known/openid-configuration') {
        return serveOpenIdConfig(c)
      }
      if (
        p.endsWith('/.well-known/oauth-authorization-server') ||
        p === '/.well-known/oauth-authorization-server'
      ) {
        return serveAuthServerMeta(c)
      }
    }
    return next()
  })

  // Proxy OAuth endpoints (authorize / token / JWKS / register / etc.)
  httpApp.all('/oauth/*', proxyOAuthEndpoint)
  httpApp.get('/api/v1/dashboard/mcp-oauth/*', proxyOAuthEndpoint)
  // Proxy /.well-known/* paths not handled above (e.g. jwks.json) directly to platform/core
  httpApp.get('/.well-known/*', proxyOAuthEndpoint)

  // Protected resource metadata — all path variants ChatGPT probes.
  // Points authorization_servers at *this* server so ChatGPT follows the proxy routes.
  const protectedResourceDoc = {
    resource: resourceUrl,
    authorization_servers: [resourceUrl],
    scopes_supported: ['projects:read', 'records:read', 'records:write'],
    bearer_methods_supported: ['header'],
    resource_documentation: 'https://docs.rushdb.com/mcp-server'
  }
  for (const path of [
    '/.well-known/oauth-protected-resource',
    '/.well-known/oauth-protected-resource/mcp',
    '/mcp/.well-known/oauth-protected-resource'
  ]) {
    httpApp.get(path, (c) => c.json(protectedResourceDoc))
  }

  // MCP Streamable HTTP endpoint
  httpApp.post('/mcp', async (c) => {
    const authHeader = c.req.header('Authorization') || ''
    const bearer = authHeader.replace(/^Bearer\s+/i, '').trim()
    const projectId = c.req.query('project_id')
    const resourceMetadataUrl = `${resourceUrl}/.well-known/oauth-protected-resource`

    if (!bearer) {
      c.header(
        'WWW-Authenticate',
        `Bearer resource_metadata="${resourceMetadataUrl}", error="unauthorized", error_description="No token provided"`
      )
      return c.json(
        makeMcpAuthError(
          resourceMetadataUrl,
          'unauthorized',
          'No token provided. Link your RushDB account to continue.'
        ),
        401
      )
    }

    let ctx: RequestContext
    try {
      ctx = await resolveRequestContext(bearer, projectId)
    } catch (e) {
      c.header(
        'WWW-Authenticate',
        `Bearer resource_metadata="${resourceMetadataUrl}", error="invalid_token", error_description="Token verification failed"`
      )
      return c.json(
        makeMcpAuthError(
          resourceMetadataUrl,
          'invalid_token',
          'Token verification failed. Please re-link your RushDB account.'
        ),
        401
      )
    }

    const mcpServer = createMcpServer()
    // WebStandardStreamableHTTPServerTransport works directly with Fetch API
    // Request/Response, which is exactly what Hono provides via c.req.raw.
    // Create a fresh transport per request (stateless — sessionIdGenerator: undefined).
    // enableJsonResponse: true returns a single JSON response instead of SSE stream.
    const transport = new WebStandardStreamableHTTPServerTransport({ enableJsonResponse: true })

    // Run inside per-request AsyncLocalStorage context so all tool handlers
    // can access db, scopes, etc. via requestContext.getStore().
    return new Promise<Response>((resolve, reject) => {
      requestContext.run(ctx, async () => {
        try {
          await mcpServer.connect(transport)
          const response = await transport.handleRequest(c.req.raw)
          resolve(response)
        } catch (e) {
          reject(e)
        }
      })
    }).catch((err) => {
      console.error('[MCP HTTP] transport error:', err)
      return new Response(JSON.stringify({ error: 'internal_error', message: String(err) }), {
        status: 500,
        headers: { 'Content-Type': 'application/json' }
      })
    })
  })

  // GET /mcp — required for SSE-based MCP clients (optional but good practice)
  httpApp.get('/mcp', (c) => c.json({ error: 'method_not_allowed', error_description: 'Use POST /mcp' }, 405))

  serve({ fetch: httpApp.fetch, port }, (info) => {
    process.stderr.write(`RushDB MCP HTTP server running on port ${info.port}\n`)
    process.stderr.write(`Resource URL: ${resourceUrl}\n`)
    process.stderr.write(`Set MCP_TRANSPORT=http to enable this mode\n`)
  })
} else {
  // ── STDIO mode (default, unchanged) ──
  const server = createMcpServer()
  const transport = new StdioServerTransport()
  await server.connect(transport)
}
