/* eslint-disable @typescript-eslint/no-explicit-any */
import type { Store } from 'nanostores'

import { map, onMount, onNotify } from 'nanostores'

import type { AnyObject } from '~/types'

import { isAnyObject } from '~/types'

type Options = {
  defaultMutationErrorHandler?: (Error: unknown) => void
  enableLoggerQueryLogger?: boolean
}

type Init = Pick<RequestInit, 'signal'>

/** When undefined is returned, nothing happens */
type FetcherFn = (init: Init & { transformResponse?: boolean }) => Promise<unknown> | undefined

type AsyncStoreOptions<Fetcher extends FetcherFn> = {
  /** List of stores that this store depends on. Will call fetcher when data in those stores is modified */
  deps?: Store[]
  /** Function that returns data */
  fetcher: Fetcher
  /** Key for cache and logging */
  key: string
  /** Same as deps, but when it's undefined, fetcher won't be called */
  mustHaveDeps?: Store[]
}

type AsyncStore = ReturnType<ReturnType<typeof createAsyncStore>>

export const isAsyncStore = (maybeStore: unknown): maybeStore is AsyncStore =>
  isAnyObject(maybeStore) && 'data' in maybeStore

const cache = new Map()

function createAsyncStore(options: Options) {
  return <
    Fetcher extends FetcherFn,
    Response extends Awaited<ReturnType<Fetcher>>,
    Data = Response extends { data: any } ? Response['data'] : Response
  >({
    key,
    fetcher,
    deps = [],
    mustHaveDeps = []
  }: AsyncStoreOptions<Fetcher>) => {
    let controller: AbortController

    const fetchData = async () => {
      if (controller) {
        controller.abort()
      }
      controller = new AbortController()

      const response = await fetcher({
        signal: controller.signal,
        transformResponse: false
      })

      let data = response
      let total

      if (isAnyObject(response)) {
        if ('data' in response) {
          data = response['data']
        }
        if ('total' in response) {
          total = response['total']
        }
      }

      if (key) {
        cache.set(key, data)
      }
      return { data, total }
    }

    const refetch = async () => {
      if (
        mustHaveDeps.some((dep) => {
          const depValue = dep.get()

          if (isAsyncStore(dep)) {
            return depValue['data'] === undefined
          }

          return depValue === undefined
        })
      ) {
        return
      }

      store.setKey('loading', true)
      store.setKey('error', undefined)
      try {
        const { data, total } = await fetchData()

        // if we prematurely return from fetcher fn, dont do anything
        if (!data) {
          return
        }
        store.set({
          data: data as Data,
          loading: false,
          total
        })
      } catch (error) {
        if (error instanceof Error) {
          if (error.name === 'AbortError') {
          } else {
            store.set({
              error: error.message,
              loading: false,
              data: undefined
            })
          }
        }
      }
    }

    const store = map<{
      data: Data | undefined
      error?: unknown
      loading: boolean
      total?: number
    }>({
      loading: true,
      data: cache.get(key),
      total: undefined
    })

    onMount(store, () => {
      refetch()

      return () => {
        try {
          if (controller) {
            controller.abort()
          }
        } catch {}
      }
    })

    for (const dep of [...deps, ...mustHaveDeps]) {
      onNotify(dep, refetch)
    }

    if (options.enableLoggerQueryLogger && key) {
      import('@nanostores/logger')
        .then(({ logger }) => logger({ [key]: store }))
        .catch(() => console.log('Please install `@nanostores/logger` to enable query logging'))
    }

    return { ...store, refetch }
  }
}

type MutatorOptions<Params, Data> = {
  fetcher: (params: Params & { init: Init }) => Promise<Data | undefined>
  invalidates?: Array<AsyncStore>
  onError?: (error: unknown) => void
  onSuccess?: (data: NonNullable<Data>) => void
  throwError?: boolean
}

function createMutator(options: Options) {
  return <Params, Data = unknown>({
    fetcher,
    invalidates = [],
    onSuccess,
    onError,
    throwError
  }: MutatorOptions<Params, Data>) => {
    let controller: AbortController

    const store = map<{
      data: Awaited<Data> | undefined
      error: unknown
      loading: boolean
      mutate: (params: Params) => Promise<Data | undefined>
    }>({
      error: undefined,
      loading: false,
      data: undefined,
      async mutate(args) {
        store.setKey('loading', true)
        store.setKey('error', undefined)

        try {
          if (controller) {
            controller.abort()
          }
          controller = new AbortController()
          const data = await fetcher({
            init: { signal: controller.signal },
            ...args
          })

          if (data) {
            for (const dep of invalidates) {
              dep.refetch()
            }

            store.set({
              ...store.get(),
              data,
              loading: false,
              error: undefined
            })

            onSuccess?.(data)
          }

          return data
        } catch (error) {
          if (error instanceof Error) {
            store.set({
              ...store.get(),
              data: undefined,
              loading: false,
              error: error.message
            })

            if (onError) {
              onError?.(error)
            } else {
              if (options.defaultMutationErrorHandler) {
                options.defaultMutationErrorHandler(error)
              }
            }
          }

          if (throwError) {
            throw error
          }
        }
      }
    })

    return store
  }
}

export const createApiStores = (options: Options) => ({
  createAsyncStore: createAsyncStore(options),
  createMutator: createMutator(options)
})
