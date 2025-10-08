import { CallHandler, ExecutionContext, Injectable, NestInterceptor } from '@nestjs/common'
import {
  isDuration,
  isLocalTime,
  isTime,
  isDate,
  isDateTime,
  isLocalDateTime,
  isInt,
  isPoint
} from 'neo4j-driver'
import Result from 'neo4j-driver/lib/result-rx'
import { isNode, isRelationship } from 'neo4j-driver-core'
import { Observable } from 'rxjs'
import { map } from 'rxjs/operators'

import {
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_INTERNALS_ALIASES
} from '@/core/common/constants'

export const toNative = (
  value: any,
  showLabelsOrType?: boolean,
  showIdentity?: boolean,
  showSummary = false
): any => {
  if (value === null || value === undefined || value instanceof Error) {
    return undefined
  } else if (value instanceof Result || value.records) {
    // Build plain records array. Prefer Record.toObject() when available because
    // neo4j driver's Record internals (like _fields) may be non-enumerable and
    // lost during JSON serialization. toObject() returns a plain object mapping
    // keys to values which is safe to convert recursively.
    const records =
      Array.isArray(value.records) ?
        value.records.map((row) => {
          // If the row is a plain object (controller already converted via toObject()),
          // just convert it recursively.
          const isRecordLike = row && typeof row === 'object' && typeof (row as any).get === 'function'

          if (!isRecordLike && row && typeof row === 'object') {
            return toNative(row, showLabelsOrType, showIdentity, false)
          }

          // Otherwise, it's a neo4j Record-like object. Prefer toObject(), but fall
          // back to mapping keys -> row.get(key) when needed.
          let sourceObj: any

          if (typeof (row as any).toObject === 'function') {
            const pojo = (row as any).toObject()
            if (pojo && Object.keys(pojo).length > 0) {
              sourceObj = pojo
            }
          }

          if (!sourceObj) {
            sourceObj = Object.fromEntries(
              (row as any).keys?.map((key: string) => [key, (row as any).get(key)]) ?? []
            )
          }

          return toNative(sourceObj, showLabelsOrType, showIdentity, false)
        })
      : []

    if (showSummary && value.summary) {
      const s: any = value.summary
      const counters = s?.counters
      const safeCounters =
        counters ?
          {
            nodesCreated: counters.nodesCreated?.(),
            nodesDeleted: counters.nodesDeleted?.(),
            relationshipsCreated: counters.relationshipsCreated?.(),
            relationshipsDeleted: counters.relationshipsDeleted?.(),
            propertiesSet: counters.propertiesSet?.(),
            labelsAdded: counters.labelsAdded?.(),
            constraintsAdded: counters.constraintsAdded?.()
          }
        : undefined

      const summary = {
        query: s?.query?.text,
        resultAvailableAfter: s?.resultAvailableAfter,
        resultConsumedAfter: s?.resultConsumedAfter,
        counters: safeCounters
      }

      return { records, summary }
    }

    return records
  } else if (Array.isArray(value)) {
    return value.map((value) => toNative(value))
  } else if (isNode(value)) {
    return toNative({
      _id: showIdentity ? toNative(value.identity, false, false, false) : null,

      _labels: showLabelsOrType ? toNative(value.labels, false, false, false) : null,
      ...toNative(value.properties, false, false, false)
    })
  } else if (isRelationship(value)) {
    return toNative({
      _id: toNative(value.identity, false, false, false),
      _type: showLabelsOrType ? toNative(value.type, false, false, false) : null,
      ...toNative(value.properties, false, false, false)
    })
  }
  // Number
  else if (isInt(value)) {
    try {
      return value.toNumber()
    } catch (err) {
      return value.toString()
    }
  }
  // Temporal
  else if (
    isDuration(value) ||
    isLocalTime(value) ||
    isTime(value) ||
    isDate(value) ||
    isDateTime(value) ||
    isLocalDateTime(value)
  ) {
    return value.toString()
  }

  // Spatial
  if (isPoint(value)) {
    switch (value.srid.toNumber()) {
      case 4326:
        return { longitude: value.y, latitude: value.x }

      case 4979:
        return { longitude: value.y, latitude: value.x, height: value.z }

      default:
        return toNative({ x: value.x, y: value.y, z: value.z })
    }
  }

  // Object
  else if (typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value).map((key) => {
        if (key === RUSHDB_KEY_PROJECT_ID) {
          return []
        } else if (key === RUSHDB_KEY_PROPERTIES_META) {
          return [
            RUSHDB_INTERNALS_ALIASES[key],
            JSON.parse(toNative(value[key], showLabelsOrType, showIdentity))
          ]
        } else {
          return [RUSHDB_INTERNALS_ALIASES[key] ?? key, toNative(value[key], showLabelsOrType, showIdentity)]
        }
      })
    )
  } else if (RUSHDB_INTERNALS_ALIASES[value]) {
    return RUSHDB_INTERNALS_ALIASES[value]
  }

  return value
}

export interface IResponse<T> {
  data: T
}

@Injectable()
export class DataInterceptor<T> implements NestInterceptor<T, IResponse<T>> {
  private showLabelsOrType = false
  private showIdentity = false
  private showSummary = false

  setOptions(showLabelsOrType = false, showIdentity = false, showSummary = false) {
    this.showLabelsOrType = showLabelsOrType
    this.showIdentity = showIdentity
    this.showSummary = showSummary

    return this
  }

  static withOptions(showLabelsOrType = false, showIdentity = false, showSummary = false): NestInterceptor {
    return new DataInterceptor().setOptions(showLabelsOrType, showIdentity, showSummary)
  }

  intercept(context: ExecutionContext, next: CallHandler): Observable<IResponse<T>> {
    return next
      .handle()
      .pipe(map((data) => toNative(data, this.showLabelsOrType, this.showIdentity, this.showSummary)))
  }
}
