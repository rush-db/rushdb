import { atom } from 'nanostores'
import type { api } from '~/lib/api.ts'

export const onboardingAgentRunSelectQuery = `{
    "labels": [
        "AGENT"
    ],
    "where": {
        "RUN": {
            "$alias": "$run"
        }
    },
    "select": {
        "agentName": "$record.name",
        "runs": {
            "$count": "$run"
        },
        "avgTotalTokens": {
            "$avg": "$run.totalTokens",
            "$precision": 0
        },
        "minTotalTokens": {
            "$min": "$run.totalTokens"
        },
        "maxTotalTokens": {
            "$max": "$run.totalTokens"
        },
        "avgLatencyMs": {
            "$avg": "$run.latencyMs",
            "$precision": 0
        },
        "maxLatencyMs": {
            "$max": "$run.latencyMs"
        },
        "avgScore": {
            "$avg": "$run.score",
            "$precision": 2
        }
    },
    "orderBy": {
        "avgTotalTokens": "desc"
    }
}`

export const $editorData = atom<string | undefined>('')

export const $selectedOperation = atom<
  | `records.${keyof (typeof api)['records']}`
  | `labels.${keyof (typeof api)['labels']}`
  | `properties.${keyof (typeof api)['properties']}`
  | `relations.${keyof (typeof api)['relationships']}`
>('records.find')
