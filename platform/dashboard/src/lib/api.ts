/**
 * This file is responsible for all api calls and data normalization
 */
import type {
  AnyObject,
  Property,
  SearchQuery,
  DBRecord,
  MaybeArray,
  DBRecordTarget,
  InferSchemaTypesWrite,
  RelationTarget,
  RelationOptions,
  PropertyDraft,
  DBRecordCreationOptions,
  OrderDirection,
  SemanticSearchParams
} from '@rushdb/javascript-sdk'

import type { GetUserResponse, User } from '~/features/auth/types'
import type { BillingData, BillingInquiryPayload } from '~/features/billing/types'
import type { Connector, ConnectorEvent, CreateConnectorInput } from '~/features/connectors/types'
import type {
  EmbeddingIndex,
  CreateEmbeddingIndexParams,
  EmbeddingIndexStats
} from '~/features/indexes/types'
import type { RelationshipPatternsResponse } from '~/features/relationship-patterns/types'
import type { Project, ProjectStats, WithProjectID } from '~/features/projects/types'
import type { ProjectToken } from '~/features/tokens/types'
import type {
  Workspace,
  WorkspaceUser,
  WorkspaceAccessList,
  InviteToWorkspaceDto,
  RevokeAccessDto,
  PendingInvite
} from '~/features/workspaces/types'
import type { GenericApiResponse, Override } from '~/types'

import { rushDBInstance } from '~/lib/sdk.ts'
import { $token } from '~/features/auth/stores/token.ts'
import { $currentProjectId } from '~/features/projects/stores/id.ts'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current.ts'
import { BASE_URL } from '~/config.ts'

import { fetcher } from './fetcher'
import { BillingErrorCodes } from '~/features/billing/constants.ts'
import type { AcceptedUserInviteDto } from '~/features/workspaces/types'
import { $limitReachModalOpen } from '~/components/billing/LimitReachedDialog.tsx'

type WithInit = {
  init?: RequestInit
}

type RelationshipEndpointQuery = {
  labels?: string[]
  where?: AnyObject
}

type RelationshipSearchQuery = Pick<SearchQuery, 'limit' | 'skip' | 'orderBy'> & {
  where?: AnyObject
  source?: RelationshipEndpointQuery
  target?: RelationshipEndpointQuery
}

type OntologyVectorIndex = {
  id: string
  sourceType: string
  similarityFunction: string
  dimensions: number
  status: string
  modelKey: string
}

export type OntologyProperty = {
  id: string
  name: string
  type: string
  min?: number | string
  max?: number | string
  values?: Array<string | number>
  recordsCount?: number
  vectorIndexes?: OntologyVectorIndex[]
}

export type OntologyItem = {
  label: string
  count: number
  properties: OntologyProperty[]
  relationships: Array<{
    label: string
    type: string
    direction: 'in' | 'out'
    count?: number
  }>
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFunction = (args: any) => void

export type ApiParams<Method extends AnyFunction> = Parameters<Method>[0]

export type ApiResult<Method extends AnyFunction> = Awaited<ReturnType<Method>>

export const api = {
  ai: {
    async generateSearchQuery({
      currentQuery,
      init,
      projectId,
      prompt
    }: WithInit & WithProjectID & { prompt: string; currentQuery?: SearchQuery }) {
      return fetcher<{ searchQuery: SearchQuery; warnings?: string[] }>(`/api/v1/ai/search-query`, {
        ...init,
        body: JSON.stringify({ prompt, currentQuery }),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async getOntology({
      force,
      init,
      labels,
      projectId
    }: WithInit & WithProjectID & { labels?: string[]; force?: boolean }) {
      return fetcher<OntologyItem[]>(`/api/v1/ai/ontology`, {
        ...init,
        body: JSON.stringify({ force, labels }),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async search(params: SemanticSearchParams) {
      return rushDBInstance.ai.search(params)
    }
  },
  records: {
    async importJson({
      init,
      data,
      label,
      options = {}
    }: WithInit & {
      label?: string
      data: MaybeArray<AnyObject>
      options?: DBRecordCreationOptions
    }) {
      return fetcher<boolean | Array<DBRecord> | { message: string; count: number }>(
        `/api/v1/records/import/json`,
        {
          ...init,
          method: 'POST',
          body: JSON.stringify({
            ...(label?.trim() ? { label: label.trim() } : {}),
            data,
            options
          })
        }
      )
    },
    async importCsv({
      init,
      data,
      label,
      options = {},
      parseConfig
    }: WithInit & {
      label: string
      data: string
      options?: DBRecordCreationOptions
      parseConfig?: {
        delimiter?: string
        header?: boolean
        skipEmptyLines?: boolean | 'greedy'
        dynamicTyping?: boolean
        quoteChar?: string
        escapeChar?: string
        newline?: string
      }
    }) {
      try {
        const recordsAny = rushDBInstance.records as any
        if (typeof recordsAny.importCsv === 'function') {
          const res = await recordsAny.importCsv({ label, data, options, parseConfig })
          return res
        }

        // Fallback: direct REST call if SDK method not present (older bundle)
        const token = $token.get()
        const projectId = $currentProjectId.get()
        const workspaceId = $currentWorkspaceId.get()
        const response = await fetch(`${BASE_URL || ''}/api/v1/records/import/csv`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            ...(token ? { Authorization: `Bearer ${token}` } : {}),
            ...(projectId ? { 'x-project-id': projectId } : {}),
            ...(workspaceId ? { 'x-workspace-id': workspaceId } : {}),
            ...(init?.headers || {})
          },
          body: JSON.stringify({ label, data, options, parseConfig })
        })
        if (!response.ok) {
          const text = await response.text().catch(() => '')
          ;(console.error || (() => undefined))('CSV import failed', response.status, text)
          throw new Error('CSV import failed')
        }
        return await response.json().catch(() => ({}))
      } catch (e: any) {
        if (e.message === BillingErrorCodes.PaymentRequired.toString()) {
          $limitReachModalOpen.set(true)
        }
        console.error('importCsv error', e)
        return {}
      }
    },
    async delete({ init, ...payload }: WithInit & ({ ids: string[] } | SearchQuery)) {
      if ('ids' in payload && payload.ids) {
        return rushDBInstance.records.deleteById(payload.ids)
      } else {
        return rushDBInstance.records.delete(payload as SearchQuery)
      }
    },
    async findById({ id, init }: { id: DBRecord['__id']; init: RequestInit }) {
      return rushDBInstance.records.findById(id)
    },
    async deleteById({ id, init }: { id: DBRecord['__id']; init?: RequestInit }) {
      return rushDBInstance.records.deleteById(id)
    },
    async find(searchQuery: SearchQuery, init: RequestInit) {
      return rushDBInstance.records.find(searchQuery)
    },
    async findOne(searchQuery: Omit<SearchQuery, 'skip' | 'limit'>, init: RequestInit) {
      return rushDBInstance.records.findOne(searchQuery)
    },
    async findUniq(searchQuery: Omit<SearchQuery, 'skip' | 'limit'>, init: RequestInit) {
      return rushDBInstance.records.findUniq(searchQuery)
    },
    async set(
      target: DBRecordTarget,
      label: string,
      data: InferSchemaTypesWrite<any> | Array<PropertyDraft>,
      init: RequestInit
    ) {
      return rushDBInstance.records.set({ target, label, data })
    },
    async update(
      target: DBRecordTarget,
      label: string,
      data: Partial<InferSchemaTypesWrite<any>> | Array<PropertyDraft>,
      init: RequestInit
    ) {
      return rushDBInstance.records.update({ target, label, data })
    },
    async export(query: SearchQuery, init: RequestInit) {
      return rushDBInstance.records.export(query)
    },
    async attach({
      source,
      target,
      options,
      init
    }: {
      source: DBRecordTarget
      target: RelationTarget
      options?: RelationOptions
    } & WithInit) {
      return rushDBInstance.records.attach({ source, target, options })
    },
    async detach({
      source,
      target,
      options,
      init
    }: {
      source: DBRecordTarget
      target: RelationTarget
      options?: RelationOptions
    } & WithInit) {
      return rushDBInstance.records.detach({ source, target, options })
    }
  },
  labels: {
    async find({ searchQuery = {}, init }: { searchQuery?: SearchQuery } & WithInit) {
      return rushDBInstance.labels.find(searchQuery)
    }
  },
  relationships: {
    async find({
      init,
      pagination,
      searchQuery
    }: {
      searchQuery: RelationshipSearchQuery
      pagination?: Pick<SearchQuery, 'limit' | 'skip'>
    } & WithInit) {
      return rushDBInstance.relationships.find({ ...searchQuery, ...pagination })
    }
  },
  relationshipPatterns: {
    async list({ projectId, init }: WithProjectID & WithInit) {
      return fetcher<RelationshipPatternsResponse>(`/api/v1/relationships/patterns`, {
        ...init,
        headers: {
          'x-project-id': projectId
        },
        method: 'GET'
      })
    },
    async analyze({ projectId, init }: WithProjectID & WithInit) {
      return fetcher<{ queued: true }>(`/api/v1/relationships/patterns/analyze`, {
        ...init,
        body: JSON.stringify({}),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async approve({ projectId, id, init }: WithProjectID & WithInit & { id: string }) {
      return fetcher(`/api/v1/relationships/patterns/${id}/approve`, {
        ...init,
        body: JSON.stringify({}),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async ignore({ projectId, id, init }: WithProjectID & WithInit & { id: string }) {
      return fetcher(`/api/v1/relationships/patterns/${id}/ignore`, {
        ...init,
        body: JSON.stringify({}),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async delete({
      projectId,
      id,
      deleteExisting,
      init
    }: WithProjectID & WithInit & { id: string; deleteExisting?: boolean }) {
      const params = deleteExisting ? '?deleteExisting=true' : ''
      return fetcher<{ deleted: true }>(`/api/v1/relationships/patterns/${id}${params}`, {
        ...init,
        headers: {
          'x-project-id': projectId
        },
        method: 'DELETE'
      })
    }
  },
  workspaces: {
    workspace({ id }: Pick<Workspace, 'id'>, init: RequestInit): Promise<Workspace> {
      return fetcher<Workspace>(`/api/v1/workspaces/${id}`, init)
    },
    list(init: RequestInit) {
      return fetcher<Workspace[]>(`/api/v1/workspaces`, init)
    },
    create({ init, name }: WithInit & Pick<Workspace, 'name'>) {
      return fetcher<Workspace>(`/api/v1/workspaces`, {
        ...init,
        method: 'POST',
        body: JSON.stringify({ name })
      })
    },
    update(params: Partial<Omit<Workspace, 'id'> & Pick<Workspace, 'id'>>, init: RequestInit) {
      const { id, name, ...body } = params

      return fetcher<Workspace>(`/api/v1/workspaces/${id}`, {
        ...init,
        method: 'PATCH',
        body: JSON.stringify({ name })
      })
    },
    delete(params: Pick<Workspace, 'id'>, init: RequestInit) {
      const { id } = params

      return fetcher<Workspace>(`/api/v1/workspaces/${id}`, {
        ...init,
        method: 'DELETE'
      })
    },
    async inviteUser({
      id,
      email,
      projectIds,
      init
    }: WithInit & Pick<Workspace, 'id'> & InviteToWorkspaceDto) {
      return fetcher<{ message: string }>(`/api/v1/workspaces/${id}/invite`, {
        ...init,
        method: 'POST',
        body: JSON.stringify({ email, projectIds })
      })
    },
    async getAccessList({ id, init }: WithInit & Pick<Workspace, 'id'>) {
      return fetcher<WorkspaceAccessList>(`/api/v1/workspaces/${id}/access-list`, {
        ...init,
        method: 'GET'
      })
    },
    async getUserList({ id, init }: WithInit & Pick<Workspace, 'id'>) {
      return fetcher<WorkspaceUser[]>(`/api/v1/workspaces/${id}/user-list`, {
        ...init,
        method: 'GET'
      })
    },
    async revokeAccess({ id, userIds, init }: WithInit & Pick<Workspace, 'id'> & RevokeAccessDto) {
      return fetcher<{ message: string }>(`/api/v1/workspaces/${id}/revoke-access`, {
        ...init,
        method: 'PATCH',
        body: JSON.stringify({ userIds })
      })
    },
    async updateAccessList({
      id,
      accessMap,
      init
    }: WithInit & Pick<Workspace, 'id'> & { accessMap: WorkspaceAccessList }) {
      return fetcher<{ message: string }>(`/api/v1/workspaces/${id}/access-list`, {
        ...init,
        method: 'PATCH',
        body: JSON.stringify(accessMap)
      })
    },
    async acceptInvitation({ token, init }: WithInit & { token: string }) {
      return fetcher<AcceptedUserInviteDto>(`/api/v1/workspaces/join-workspace`, {
        ...init,
        method: 'POST',
        body: JSON.stringify({ token })
      })
    },
    async getPendingInvites({ init, id }: WithInit & Pick<Workspace, 'id'>): Promise<PendingInvite[]> {
      return fetcher<PendingInvite[]>(`/api/v1/workspaces/${id}/pending-invites`, {
        ...init,
        method: 'GET'
      })
    },

    async removePendingInvite({
      init,
      id,
      email
    }: WithInit & Pick<Workspace, 'id'> & { email: string }): Promise<{ message: string }> {
      return fetcher<{ message: string }>(`/api/v1/workspaces/${id}/pending-invites`, {
        ...init,
        method: 'PATCH',
        body: JSON.stringify({ email })
      })
    },

    async leaveWorkspace({ id, init }: { id: string; init?: RequestInit }): Promise<{ message: string }> {
      return fetcher<{ message: string }>(`/api/v1/workspaces/${id}/leave-workspace`, {
        ...init,
        body: JSON.stringify({}),
        method: 'PATCH'
      })
    }
  },
  tokens: {
    async list({ projectId }: WithProjectID, init: RequestInit) {
      return fetcher<ProjectToken[]>(`/api/v1/tokens`, init)
    },
    async create({ projectId, init, ...body }: WithProjectID & Partial<ProjectToken> & WithInit) {
      const { noExpire, ...payload } = body
      return fetcher<ProjectToken>(`/api/v1/tokens`, {
        ...init,
        body: JSON.stringify({
          ...payload,
          expiration: noExpire ? '*' : payload.expiration
        }),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async delete({ init, id }: WithInit & Pick<ProjectToken, 'id'>) {
      return fetcher<ProjectToken>(`/api/v1/tokens/${id}`, {
        ...init,
        method: 'DELETE'
      })
    }
  },
  connectors: {
    async list({ projectId, init }: WithProjectID & WithInit) {
      return fetcher<Connector[]>(`/api/v1/connectors`, {
        ...init,
        headers: {
          'x-project-id': projectId
        }
      })
    },
    async get({ projectId, id, init }: WithProjectID & WithInit & { id: string }) {
      return fetcher<Connector>(`/api/v1/connectors/${id}`, {
        ...init,
        headers: {
          'x-project-id': projectId
        }
      })
    },
    async events({ projectId, id, init }: WithProjectID & WithInit & { id: string }) {
      const events = await fetcher<ConnectorEvent[]>(`/api/v1/connectors/${id}/events`, {
        ...init,
        headers: {
          'x-project-id': projectId
        }
      })

      return events.map((event) => {
        if (!event.metadata || typeof event.metadata !== 'string') return event

        try {
          return { ...event, metadata: JSON.parse(event.metadata) as Record<string, unknown> }
        } catch {
          return event
        }
      })
    },
    async create({ projectId, init, ...body }: WithProjectID & WithInit & CreateConnectorInput) {
      return fetcher<Connector>(`/api/v1/connectors`, {
        ...init,
        method: 'POST',
        body: JSON.stringify(body),
        headers: {
          'x-project-id': projectId
        }
      })
    },
    async action({
      projectId,
      id,
      action,
      init
    }: WithProjectID & WithInit & { id: string; action: 'pause' | 'resume' | 'resnapshot' | 'test' }) {
      return fetcher<Connector | { ok: boolean; message: string }>(`/api/v1/connectors/${id}/${action}`, {
        ...init,
        method: 'POST',
        headers: {
          'x-project-id': projectId
        }
      })
    }
  },
  projects: {
    async list(init: RequestInit) {
      const { data: projects, total } = await fetcher<
        GenericApiResponse<Array<Override<Project, { stats?: string }>>>
      >(`/api/v1/projects`, {
        ...init,
        transformResponse: false
      })

      return {
        data: projects.map<Project>((p) => ({
          ...p,
          stats: JSON.parse(p?.stats ?? '{"records":0, "properties": 0}') as ProjectStats
        })),
        total
      }
    },
    async project({ projectId }: WithProjectID, init: RequestInit) {
      const { data: project } = await fetcher<GenericApiResponse<Project>>(`/api/v1/projects/${projectId}`, {
        ...init,
        transformResponse: false
      })

      return project
    },
    async delete({ init, id }: WithInit & Pick<Project, 'id'>) {
      return fetcher<boolean>(`/api/v1/projects/${id}`, {
        method: 'DELETE',
        ...init
      })
    },
    async update({ init, id, ...body }: WithInit & Partial<Project>) {
      return fetcher<Project>(`/api/v1/projects/${id}`, {
        method: 'PATCH',
        body: JSON.stringify(body),
        ...init
      })
    },
    async create({ init, ...body }: WithInit & Partial<Project>) {
      return fetcher<Project>(`/api/v1/projects`, {
        method: 'POST',
        body: JSON.stringify(body),
        ...init
      })
    }
  },
  user: {
    async current(init?: RequestInit) {
      const user = await fetcher<GetUserResponse['data']>(`/api/v1/user`, {
        ...init,
        transformResponse: true
      })

      return { ...user }
    },
    async update({ init, ...body }: { init?: RequestInit } & Partial<User>) {
      return fetcher<GetUserResponse>(`/api/v1/user`, {
        ...init,
        body: JSON.stringify(body),
        method: 'PATCH'
      })
    },
    async delete({ init }: { init?: RequestInit }) {
      return fetcher<GetUserResponse>(`/api/v1/user`, {
        ...init,
        body: JSON.stringify({}),
        method: 'DELETE'
      })
    }
  },
  properties: {
    async find({ searchQuery, init }: { searchQuery: SearchQuery; init: RequestInit }) {
      return rushDBInstance.properties.find(searchQuery)
    },
    async values({
      id,
      searchQuery,
      init
    }: {
      id: Property['id']
      searchQuery: SearchQuery & { query?: string; orderBy?: OrderDirection }
      init: RequestInit
    }) {
      return rushDBInstance.properties.values(id, searchQuery)
    }
  },
  auth: {
    async sendRecoveryLink({ email }: { email?: string } & WithInit) {
      return fetcher(`/api/v1/auth/forgot-password/${email}`, {
        method: 'GET'
      })
    },
    async resendConfirmationLink(id: User['id']) {
      return fetcher(`/api/v1/auth/resend-confirmation-link`, {
        body: JSON.stringify({ id }),
        method: 'POST'
      })
    },
    async resetPassword({
      password,
      token,
      login
    }: { login: string; token: string; password: string } & WithInit) {
      return fetcher<GetUserResponse['data']>(`/api/v1/auth/reset-password`, {
        body: JSON.stringify({ password, token, login }),
        method: 'POST'
      })
    },
    async register({ login, password }: { login?: string; password?: string }) {
      return fetcher<GetUserResponse['data']>(`/api/v1/auth/register`, {
        body: JSON.stringify({ login, password }),
        method: 'POST'
      })
    }
  },
  billing: {
    async createSession({
      init,
      ...body
    }: WithInit & {
      discountCode?: string
      priceId: string
      projectId?: string
      returnUrl: string
    }) {
      // TODO: add response
      return fetcher<{ id: string }>(`/api/v1/billing/payment/create-session`, {
        ...init,
        method: 'POST',
        body: JSON.stringify(body)
      })
    },
    /** https://stripe.com/docs/billing/quickstart */
    async createPortalSession({
      init,
      ...body
    }: WithInit & {
      returnUrl: string
    }) {
      return fetcher<{ redirectUrl: string }>(`/api/v1/billing/payment/create-portal-session`, {
        ...init,
        method: 'POST',
        body: JSON.stringify(body)
      })
    },
    async getBillingData() {
      // Proxied through core so the billing service host is never exposed to the browser.
      return fetcher<BillingData>(`/api/v1/billing/payment/prices`, {
        method: 'GET'
      })
    },
    async getKuHistory({
      init,
      limit,
      before,
      since,
      projectId,
      operation
    }: WithInit & {
      limit?: number
      before?: string
      since?: string
      projectId?: string | null
      operation?: string | null
    }) {
      const params = new URLSearchParams()
      if (limit) params.set('limit', limit.toString())
      if (before) params.set('before', before)
      if (since) params.set('since', since)
      if (projectId) params.set('projectId', projectId)
      if (operation) params.set('operation', operation)
      const query = params.toString() ? `?${params}` : ''

      return fetcher<{
        events: Array<{
          id: string
          workspaceId: string
          projectId: string
          operation: string
          kuConsumed: number
          metadata: Record<string, unknown> | null
          timestamp: string
        }>
        hasMore: boolean
        nextCursor: string | null
      }>(`/api/v1/billing/payment/ku-history${query}`, {
        ...init,
        method: 'GET'
      })
    },
    async getUsage(init?: RequestInit) {
      return fetcher<{
        plan: string
        kuConsumed: number
        kuLimit: number | null
        kuIncluded: number | null
        remaining: number | null
        billingModel: 'fixed' | 'overage' | 'usage'
        billingPeriodStart: string
      }>(`/api/v1/billing/payment/usage`, {
        ...init,
        method: 'GET'
      })
    },
    async submitInquiry({ init, ...body }: WithInit & BillingInquiryPayload) {
      return fetcher<{ success: boolean }>(`/api/v1/billing/payment/inquiry`, {
        ...init,
        method: 'POST',
        body: JSON.stringify(body)
      })
    }
  },
  settings: {
    get: ({ init }: WithInit) => {
      return fetcher<{
        selfHosted: boolean
        dashboardUrl: string
        googleOAuthEnabled: boolean
        githubOAuthEnabled: boolean
        embeddingEnabled: boolean
        llmEnabled: boolean
        synxEnabled: boolean
      }>(`/api/v1/settings`, {
        ...init,
        method: 'GET'
      })
    }
  },
  query: {
    'records-find': async (params: { searchQuery: SearchQuery }) => {
      return fetcher<string>('/api/v1/query/records/find', {
        method: 'POST',
        body: JSON.stringify(params.searchQuery)
      })
    }
  },
  oauth: {
    async getAuthRequest(id: string) {
      return fetcher<{
        auth_request_id: string
        client_name: string
        scope: string
        resource: string
        expires_at: string
      }>(`/oauth/authorize/request/${id}`, { method: 'GET' })
    },
    async acceptAuthorization({
      authRequestId,
      projectId,
      scope
    }: {
      authRequestId: string
      projectId: string
      scope?: string
    }) {
      return fetcher<{ redirectTo: string }>('/oauth/authorize/accept', {
        method: 'POST',
        body: JSON.stringify({
          auth_request_id: authRequestId,
          project_id: projectId,
          ...(scope !== undefined ? { scope } : {})
        })
      })
    },
    async denyAuthorization(authRequestId: string) {
      return fetcher<{ redirectTo: string }>('/oauth/authorize/deny', {
        method: 'POST',
        body: JSON.stringify({ authRequestId })
      })
    },
    async listConsents() {
      return fetcher<
        Array<{
          id: string
          client_id: string
          client_name: string
          scope: string
          project_id: string
          project_name: string
          resource: string
          created: string
        }>
      >('/oauth/consents', { method: 'GET' })
    },
    async revokeConsent(id: string) {
      return fetcher<void>(`/oauth/consents/${id}`, { method: 'DELETE' })
    }
  },
  indexes: {
    async list({ projectId }: WithProjectID, init?: RequestInit) {
      return fetcher<EmbeddingIndex[]>(`/api/v1/ai/indexes`, {
        ...init,
        headers: {
          'x-project-id': projectId
        },
        method: 'GET'
      })
    },
    async create({ projectId, init, ...body }: WithProjectID & CreateEmbeddingIndexParams & WithInit) {
      return fetcher<EmbeddingIndex>(`/api/v1/ai/indexes`, {
        ...init,
        body: JSON.stringify(body),
        headers: {
          'x-project-id': projectId
        },
        method: 'POST'
      })
    },
    async delete({ init, id }: WithInit & { id: string }) {
      return fetcher<{ deleted: boolean }>(`/api/v1/ai/indexes/${id}`, {
        ...init,
        method: 'DELETE'
      })
    },
    async stats({ init, id, projectId }: WithInit & WithProjectID & { id: string }) {
      return fetcher<EmbeddingIndexStats>(`/api/v1/ai/indexes/${id}/stats`, {
        ...init,
        headers: {
          'x-project-id': projectId
        },
        method: 'GET'
      })
    }
  }
}
