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
import { isNode, isRelationship } from 'neo4j-driver-core'
import Result from 'neo4j-driver/lib/result-rx'
import { Observable } from 'rxjs'
import { map } from 'rxjs/operators'

import {
  RUSHDB_KEY_PROJECT_ID,
  RUSHDB_KEY_PROPERTIES_META,
  RUSHDB_INTERNALS_ALIASES
} from '@/core/common/constants'

export const toNative = (value: any, showLabelsOrType?: boolean, showIdentity?: boolean): any => {
  if (value === null || value === undefined || value instanceof Error) {
    return undefined
  } else if (value instanceof Result || value.records) {
    if (typeof value === 'object') {
      return Object.fromEntries(
        Object.keys(value).map((key) => [key, toNative(value[key], showLabelsOrType, showIdentity)])
      )
    }
    return value.records.map((row) =>
      Object.fromEntries(row.keys.map((key) => [key, toNative(row.get(key))]))
    )
  } else if (Array.isArray(value)) {
    return value.map((value) => toNative(value))
  } else if (isNode(value)) {
    return toNative({
      _id: showIdentity ? toNative(value.identity) : null,

      _labels: showLabelsOrType ? toNative(value.labels) : null,
      ...toNative(value.properties)
    })
  } else if (isRelationship(value)) {
    return toNative({
      _id: toNative(value.identity),
      _type: showLabelsOrType ? toNative(value.type) : null,
      ...toNative(value.properties)
    })
  }
  // Number
  else if (isInt(value)) {
    return value.toNumber()
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
export class NeogmaDataInterceptor<T> implements NestInterceptor<T, IResponse<T>> {
  private showLabelsOrType = false
  private showIdentity = false

  setOptions(showLabelsOrType = false, showIdentity = false) {
    this.showLabelsOrType = showLabelsOrType
    this.showIdentity = showIdentity

    return this
  }

  static withOptions(showLabelsOrType = false, showIdentity = false): NestInterceptor {
    return new NeogmaDataInterceptor().setOptions(showLabelsOrType, showIdentity)
  }

  intercept(context: ExecutionContext, next: CallHandler): Observable<IResponse<T>> {
    return next.handle().pipe(map((data) => toNative(data, this.showLabelsOrType, this.showIdentity)))
  }
}
