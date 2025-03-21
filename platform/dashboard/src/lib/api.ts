/**
 * This file is responsible for all api calls and data normalization
 */
import type { AnyObject, Property, SearchQuery, DBRecord, MaybeArray } from '@rushdb/javascript-sdk'

import { DBRecordsBatchDraft } from '@rushdb/javascript-sdk'

import type { GetUserResponse, User } from '~/features/auth/types'
import type { PlanId, PlanPeriod } from '~/features/billing/types'
import type { Project, ProjectStats, WithProjectID } from '~/features/projects/types'
import type { ProjectToken } from '~/features/tokens/types'
import type { Workspace } from '~/features/workspaces/types'
import type { GenericApiResponse, Override } from '~/types'

import { sdk } from '~/lib/sdk.ts'

import { fetcher } from './fetcher'
import { BillingErrorCodes } from '~/features/billing/constants.ts'
import { $limitReachModalOpen } from '~/features/billing/components/LimitReachedDialog.tsx'
import { IncomingBillingData } from '~/features/billing/types'

type WithInit = {
  init?: RequestInit
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFunction = (args: any) => void

export type ApiParams<Method extends AnyFunction> = Parameters<Method>[0]

export type ApiResult<Method extends AnyFunction> = Awaited<ReturnType<Method>>

export const api = {
  records: {
    async batchUpload({
      init,
      payload,
      label,
      options = {}
    }: WithInit & {
      label: string
      options?: { suggestTypes?: boolean }
      payload: MaybeArray<AnyObject>
    }) {
      const data = new DBRecordsBatchDraft({ label, options, payload })
      try {
        return await sdk(init).records.createMany(data)
      } catch (e: any) {
        if (e.message === BillingErrorCodes.PaymentRequired.toString()) {
          $limitReachModalOpen.set(true)
        }
        return {}
      }
    },
    async batchDelete({ init, ...payload }: WithInit & ({ ids: string[] } | SearchQuery)) {
      if ('ids' in payload && payload.ids) {
        return sdk(init).records.deleteById(payload.ids)
      } else {
        return sdk(init).records.delete(payload as SearchQuery)
      }
    },
    async findById({ id, init }: { id: DBRecord['__id']; init: RequestInit }) {
      return sdk(init).records.findById(id)
    },
    async deleteById({ id, init }: { id: DBRecord['__id']; init?: RequestInit }) {
      return sdk(init).records.deleteById(id)
    },
    async relations({
      id,
      init
    }: WithInit & {
      id: DBRecord['__id']
    }) {
      return sdk(init).records.relations(id)
    },
    async labels({ searchQuery = {}, init }: { searchQuery?: SearchQuery } & WithInit) {
      return sdk(init).labels.find(searchQuery)
    },
    async properties(id: string, init: RequestInit) {
      return sdk(init).records.properties(id)
    },
    async find(queryOdId: SearchQuery | string, init: RequestInit) {
      return sdk(init).records.find(queryOdId)
    },
    async exportCsv(query: SearchQuery, init: RequestInit) {
      return sdk(init).records.export(query)
    }
  },
  relations: {
    async find({
      init,
      pagination,
      searchQuery
    }: {
      searchQuery: SearchQuery
      pagination?: Pick<SearchQuery, 'limit' | 'skip'>
    } & WithInit) {
      return sdk(init).relations.find({ pagination, search: searchQuery })
    }
  },
  workspaces: {
    async workspace({ id }: Pick<Workspace, 'id'>, init: RequestInit): Promise<Workspace> {
      const { data } = await fetcher<GenericApiResponse<Override<Workspace, { limits: string }>>>(
        `/api/v1/workspaces/${id}`,
        init
      )
      return { ...data, limits: JSON.parse(data.limits) }
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
      const { id, ...body } = params

      return fetcher<Workspace>(`/api/v1/workspaces/${id}`, {
        ...init,
        method: 'PATCH',
        body: JSON.stringify(body)
      })
    },
    delete(params: Pick<Workspace, 'id'>, init: RequestInit) {
      const { id } = params

      return fetcher<Workspace>(`/api/v1/workspaces/${id}`, {
        ...init,
        method: 'DELETE'
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

      return { ...user, settings: JSON.parse(user.settings) }
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
    async list(query: SearchQuery, init: RequestInit) {
      return sdk(init).properties.find(query)
    },
    async values({ init, propertyId }: WithInit & { propertyId: Property['id'] }) {
      return sdk(init).properties.values(propertyId)
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
      id: PlanId
      period: PlanPeriod
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
      return fetcher<IncomingBillingData>('https://billing.rushdb.com/api/prices', {
        method: 'GET'
      })
    }
  },
  settings: {
    get: ({ init }: WithInit) => {
      return fetcher<{
        data: {
          selfHosted: boolean
          dashboardUrl: string
          googleOAuthEnabled: boolean
          githubOAuthEnabled: boolean
        }
      }>(`/api/v1/settings`, {
        ...init,
        method: 'GET'
      })
    }
  }
}
