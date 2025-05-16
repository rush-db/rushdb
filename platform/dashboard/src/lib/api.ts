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
  DBRecordCreationOptions
} from '@rushdb/javascript-sdk'

import type { GetUserResponse, User } from '~/features/auth/types'
import type { PlanId, PlanPeriod } from '~/features/billing/types'
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

import { sdk } from '~/lib/sdk.ts'

import { fetcher } from './fetcher'
import { BillingErrorCodes } from '~/features/billing/constants.ts'
import { $limitReachModalOpen } from '~/features/billing/components/LimitReachedDialog.tsx'
import { IncomingBillingData } from '~/features/billing/types'
import { AcceptedUserInviteDto } from '~/features/workspaces/types'

type WithInit = {
  init?: RequestInit
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyFunction = (args: any) => void

export type ApiParams<Method extends AnyFunction> = Parameters<Method>[0]

export type ApiResult<Method extends AnyFunction> = Awaited<ReturnType<Method>>

export const api = {
  records: {
    async createMany({
      init,
      payload,
      label,
      options = {}
    }: WithInit & {
      label: string
      options?: DBRecordCreationOptions
      payload: MaybeArray<AnyObject>
    }) {
      try {
        return await sdk(init).records.createMany({ label, options, payload })
      } catch (e: any) {
        if (e.message === BillingErrorCodes.PaymentRequired.toString()) {
          $limitReachModalOpen.set(true)
        }
        return {}
      }
    },
    async delete({ init, ...payload }: WithInit & ({ ids: string[] } | SearchQuery)) {
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
    async find(searchQuery: SearchQuery, init: RequestInit) {
      return sdk(init).records.find(searchQuery)
    },
    async findOne(searchQuery: Omit<SearchQuery, 'skip' | 'limit'>, init: RequestInit) {
      return sdk(init).records.findOne(searchQuery)
    },
    async findUniq(searchQuery: Omit<SearchQuery, 'skip' | 'limit'>, init: RequestInit) {
      return sdk(init).records.findUniq(searchQuery)
    },
    async set(
      target: DBRecordTarget,
      label: string,
      data: InferSchemaTypesWrite<any> | Array<PropertyDraft>,
      init: RequestInit
    ) {
      return sdk(init).records.set({ target, label, data })
    },
    async update(
      target: DBRecordTarget,
      label: string,
      data: Partial<InferSchemaTypesWrite<any>> | Array<PropertyDraft>,
      init: RequestInit
    ) {
      return sdk(init).records.update({ target, label, data })
    },
    async export(query: SearchQuery, init: RequestInit) {
      return sdk(init).records.export(query)
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
      return sdk(init).records.attach({ source, target, options })
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
      return sdk(init).records.detach({ source, target, options })
    }
  },
  labels: {
    async find({ searchQuery = {}, init }: { searchQuery?: SearchQuery } & WithInit) {
      return sdk(init).labels.find(searchQuery)
    }
  },
  relationships: {
    async find({
      init,
      pagination,
      searchQuery
    }: {
      searchQuery: SearchQuery
      pagination?: Pick<SearchQuery, 'limit' | 'skip'>
    } & WithInit) {
      return sdk(init).relationships.find({ ...searchQuery, ...pagination })
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
    async find({ searchQuery, init }: { searchQuery: SearchQuery; init: RequestInit }) {
      return sdk(init).properties.find(searchQuery)
    },
    async values({ id, init }: { id: Property['id']; init: RequestInit }) {
      return sdk(init).properties.values(id)
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
