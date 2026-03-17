import { Injectable } from '@nestjs/common'

import { RUSHDB_LABEL_PROPERTY, RUSHDB_LABEL_RECORD, RUSHDB_RELATION_VALUE } from '@/core/common/constants'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { QueryBuilder } from '@/database/QueryBuilder'

@Injectable()
export class AiQueryService {
  /**
   * Q1: Returns { label, recordCount } for all labels in the project.
   * Optional filterLabels narrows results to a subset.
   */
  getLabelsQuery(filterLabels?: string[]) {
    const qb = new QueryBuilder()

    qb.append(`MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`)
    qb.append(`WITH [l IN labels(record) WHERE l <> "${RUSHDB_LABEL_RECORD}"][0] AS label`)
    qb.append(`WHERE label IS NOT NULL`)

    if (filterLabels && filterLabels.length > 0) {
      qb.append(`AND label IN $filterLabels`)
    }

    qb.append(`RETURN label, count(*) AS recordCount`)
    qb.append(`ORDER BY recordCount DESC`)

    return qb.getQuery()
  }

  /**
   * Q2: Returns { label, propId, propName, propType, sampleValues, minValue, maxValue }
   * for all non-vector properties across all records in the project, in a single pass.
   */
  getPropertiesWithValuesQuery(filterLabels?: string[]) {
    const qb = new QueryBuilder()

    // Direction: property -[VALUE]-> record  (matches existing pattern in property-query.service.ts)
    qb.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })<-[:${RUSHDB_RELATION_VALUE}]-(prop:${RUSHDB_LABEL_PROPERTY} { projectId: $projectId })`
    )
    qb.append(`WHERE prop.type <> 'vector' AND record[prop.name] IS NOT NULL`)

    // Extract the user-facing label from this record
    qb.append(`WITH [l IN labels(record) WHERE l <> "${RUSHDB_LABEL_RECORD}"][0] AS label,`)
    qb.append(`     prop.id AS propId, prop.name AS propName, prop.type AS propType,`)
    qb.append(`     record[prop.name] AS rawVal`)

    if (filterLabels && filterLabels.length > 0) {
      qb.append(`WHERE label IN $filterLabels`)
    }

    // Aggregate all values per (label, property)
    qb.append(`WITH label, propId, propName, propType,`)
    qb.append(`     apoc.coll.toSet(apoc.coll.flatten(collect(DISTINCT rawVal))) AS vals`)

    // Compute sampleValues (string/boolean) and min/max (number/datetime)
    qb.append(`UNWIND vals AS v`)
    qb.append(`WITH label, propId, propName, propType, vals,`)
    qb.append(`     min(CASE propType WHEN 'datetime' THEN datetime(v) ELSE toFloatOrNull(v) END) AS minAgg,`)
    qb.append(`     max(CASE propType WHEN 'datetime' THEN datetime(v) ELSE toFloatOrNull(v) END) AS maxAgg`)

    qb.append(`RETURN`)
    qb.append(`  label, propId, propName, propType,`)
    qb.append(`  CASE propType`)
    qb.append(`    WHEN 'string'  THEN vals[0..10]`)
    qb.append(`    WHEN 'boolean' THEN ['true', 'false']`)
    qb.append(`    ELSE null`)
    qb.append(`  END AS sampleValues,`)
    qb.append(`  CASE propType WHEN 'datetime' THEN toString(minAgg) ELSE minAgg END AS minValue,`)
    qb.append(`  CASE propType WHEN 'datetime' THEN toString(maxAgg) ELSE maxAgg END AS maxValue`)

    return qb.getQuery()
  }

  /**
   * Q3: Returns { fromLabel, relType, toLabel, relCount } for all cross-record
   * relationships (excluding internal RushDB relations and self-loops).
   */
  getRelationshipsQuery(filterLabels?: string[]) {
    const qb = new QueryBuilder()

    qb.append(
      `MATCH (a:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })-[r]->(b:${RUSHDB_LABEL_RECORD} { ${projectIdInline()} })`
    )
    // Exclude only RUSHDB_RELATION_VALUE which connects PROPERTY→RECORD nodes (not RECORD→RECORD).
    // RUSHDB_RELATION_DEFAULT is the type used for user-created record-to-record links — include it.
    qb.append(`WHERE type(r) <> '${RUSHDB_RELATION_VALUE}'`)

    qb.append(`WITH [l IN labels(a) WHERE l <> "${RUSHDB_LABEL_RECORD}"][0] AS fromLabel,`)
    qb.append(`     type(r) AS relType,`)
    qb.append(`     [l IN labels(b) WHERE l <> "${RUSHDB_LABEL_RECORD}"][0] AS toLabel,`)
    qb.append(`     count(*) AS relCount`)

    qb.append(`WHERE fromLabel IS NOT NULL AND toLabel IS NOT NULL`)

    if (filterLabels && filterLabels.length > 0) {
      qb.append(`AND (fromLabel IN $filterLabels OR toLabel IN $filterLabels)`)
    }

    qb.append(`RETURN DISTINCT fromLabel, relType, toLabel, relCount`)

    return qb.getQuery()
  }
}
