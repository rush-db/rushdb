import { BASE_URL } from '~/config'
import { toast } from '~/elements/Toast'
import { $token } from '~/features/auth/stores/token'
import { $limitReachModalOpen } from '~/features/billing/components/LimitReachedDialog'
import { BillingErrorCodes } from '~/features/billing/constants'
import { $currentProjectId } from '~/features/projects/stores/id'
import { $currentWorkspaceId } from '~/features/workspaces/stores/current'
import { createApiStores } from '~/lib/nanorequest'

type FetcherInit = RequestInit & { transformResponse?: boolean }

// Pick out data and prevent nested properties in a hook or selector
const transformAPIResponse = <Data>(response: { data: Data }) => response.data

const defaultHeaders: HeadersInit = {
  'Content-Type': 'application/json'
}

export class FetchError extends Error {
  data: {
    message: string
  }
  response: Response

  constructor({
    data,
    message,
    response
  }: {
    data?: {
      message: string
    }
    message: string
    response: Response
  }) {
    // Pass remaining arguments (including vendor specific ones) to parent constructor
    super(message)

    // Maintains proper stack trace for where our error was thrown (only available on V8)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, FetchError)
    }

    this.name = 'FetchError'
    this.response = response
    this.data = data ?? { message: message }
  }
}

export async function fetcher<Data = unknown>(
  input: URL | string,
  { headers: initHeaders, transformResponse = true, ...init }: FetcherInit = {}
): Promise<Data> {
  const token = $token.get()
  const currentWorkspaceId = $currentWorkspaceId.get()
  const currentProjectId = $currentProjectId.get()

  const authHeaders: HeadersInit =
    token ?
      {
        Authorization: `Bearer ${token}`,
        ...(currentWorkspaceId ? { 'x-workspace-id': currentWorkspaceId } : {}),
        ...(currentProjectId ? { 'x-project-id': currentProjectId } : {})
      }
    : {}

  const needsBaseUrl = !/^https?:\/\//.test(input as string)

  const url = needsBaseUrl ? `${BASE_URL}${input}` : input

  const response = await fetch(url, {
    headers: {
      ...defaultHeaders,
      ...authHeaders,
      ...initHeaders
    },
    ...init
  })

  if (response.ok) {
    const data = await response.json()

    if (transformResponse && 'data' in data) {
      return transformAPIResponse(data)
    }

    return data
  }

  const errorMessage = response.statusText

  throw new FetchError({
    message: errorMessage,
    response
  })
}

function defaultMutationErrorHandler(error: unknown) {
  if (error instanceof FetchError && error.response.status === BillingErrorCodes.PaymentRequired) {
    return $limitReachModalOpen.set(true)
  }

  if (error instanceof Error && error.message) {
    return toast({
      variant: 'danger',
      title: error.message
    })
  }
}

export const { createAsyncStore, createMutator } = createApiStores({
  defaultMutationErrorHandler: defaultMutationErrorHandler
})
