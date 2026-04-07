import { Injectable } from '@nestjs/common'

import {
  RUSHDB_KEY_ID,
  RUSHDB_LABEL_PROPERTY,
  RUSHDB_LABEL_RECORD,
  RUSHDB_RELATION_VALUE
} from '@/core/common/constants'
import { projectIdInline } from '@/core/search/parser/projectIdInline'
import { QueryBuilder } from '@/database/QueryBuilder'

@Injectable()
export class AiQueryService {
  private quoteIdentifier(value: string): string {
    return `\`${value.replace(/`/g, '')}\``
  }

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
    qb.append(`WHERE record[prop.name] IS NOT NULL`)

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

  /**
   * Looks up a property node by name and projectId.
   * Returns { propType } or nothing if not found.
   */
  getPropertyTypeQuery() {
    const qb = new QueryBuilder()
    qb.append(`MATCH (prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })`)
    qb.append(`RETURN prop.type AS propType LIMIT 1`)
    return qb.getQuery()
  }

  /**
   * Returns aggregate stats for an embedding index policy.
   * - totalRecords: records of this label that have the property
   * - indexedRecords: records where the VALUE relation already carries the selected vector property for this propKey
   * @param labelSuffix - Neo4j label suffix e.g. ":Book" scopes the record MATCH
   */
  getEmbeddingIndexStatsQuery(labelSuffix: string, vectorPropertyName: string) {
    const qb = new QueryBuilder()
    const vectorProperty = `rel.${this.quoteIdentifier(vectorPropertyName)}`
    qb.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD}${labelSuffix} { ${projectIdInline()} })<-[rel:${RUSHDB_RELATION_VALUE}]-(prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })`
    )
    qb.append(
      `RETURN count(*) AS totalRecords, count(CASE WHEN rel.__propKey = $propKey AND ${vectorProperty} IS NOT NULL THEN 1 END) AS indexedRecords`
    )
    return qb.getQuery()
  }

  /**
   * DDL: create a relationship vector index for a single vector slot.
   * Must be run outside a transaction (DDL not allowed in explicit transactions).
   */
  getCreateVectorIndexQuery({
    indexName,
    vectorPropertyName,
    similarityFunction
  }: {
    indexName: string
    vectorPropertyName: string
    similarityFunction: 'cosine' | 'euclidean'
  }) {
    return `CREATE VECTOR INDEX ${this.quoteIdentifier(indexName)} IF NOT EXISTS FOR ()-[r:${this.quoteIdentifier(RUSHDB_RELATION_VALUE)}]-() ON r.${this.quoteIdentifier(vectorPropertyName)} OPTIONS { indexConfig: { \`vector.dimensions\`: $dimensions, \`vector.similarity_function\`: '${similarityFunction}' } }`
  }

  /**
   * DDL: drop a vector index for a specific slot.
   * Must be run outside a transaction.
   */
  getDropVectorIndexQuery(indexName: string) {
    return `DROP INDEX ${this.quoteIdentifier(indexName)} IF EXISTS`
  }

  /**
   * Strips vector data from VALUE relationships for a given (projectId, label, propertyName).
   * Only removes the selected vector property from relationships whose __propKey matches exactly — this is critical
   * when another label's index shares the same property node (e.g. Book:title vs Task:title).
   * @param labelSuffix - Neo4j label suffix e.g. ":Book" scopes the record MATCH
   */
  getStripEmbeddingsQuery(labelSuffix: string, vectorPropertyName: string) {
    const qb = new QueryBuilder()
    const vectorProperty = `rel.${this.quoteIdentifier(vectorPropertyName)}`
    qb.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD}${labelSuffix} { ${projectIdInline()} })<-[rel:${RUSHDB_RELATION_VALUE}]-(prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })`
    )
    qb.append(`WHERE ${vectorProperty} IS NOT NULL AND rel.__propKey = $propKey`)
    qb.append(`REMOVE ${vectorProperty}`)
    return qb.getQuery()
  }

  /**
   * Returns pending relationships (prop→record) that still need embeddings.
   * Scoped to the exact label so that Book:title and Task:title are indexed independently.
   * @param labelSuffix - Neo4j label suffix e.g. ":Book"
   */
  getUnindexedRelationsQuery(labelSuffix: string, vectorPropertyName: string) {
    const qb = new QueryBuilder()
    const vectorProperty = `rel.${this.quoteIdentifier(vectorPropertyName)}`
    qb.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD}${labelSuffix} { ${projectIdInline()} })<-[rel:${RUSHDB_RELATION_VALUE}]-(prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })`
    )
    qb.append(
      `WHERE (${vectorProperty} IS NULL OR rel.__propKey <> $propKey) AND record[prop.name] IS NOT NULL`
    )
    qb.append(`RETURN elementId(rel) AS relId, record[prop.name] AS value`)
    qb.append(`SKIP $skip LIMIT $batchSize`)
    return qb.getQuery()
  }

  /**
   * Writes embedding vectors back onto the VALUE relationships identified by elementId.
   * Also sets rel.__projectId and rel.__propKey so the global vector index can post-filter by tenant.
   * Expects parameter: $updates = [{ relId: string, emb: number[], projectId: string, propKey: string }, ...]
   */
  getWriteEmbeddingsQuery(vectorPropertyName: string) {
    return `UNWIND $updates AS u MATCH ()-[rel:${RUSHDB_RELATION_VALUE}]-() WHERE elementId(rel) = u.relId SET rel.${this.quoteIdentifier(vectorPropertyName)} = u.emb, rel.__projectId = u.projectId, rel.__propKey = u.propKey`
  }

  /**
   * Writes vectors to VALUE relationships by record id for external vector ingestion.
   * Returns requestedCount and updatedCount so the caller can detect missing records.
   */
  getWriteEmbeddingsByRecordIdQuery(labelSuffix: string, vectorPropertyName: string) {
    const qb = new QueryBuilder()
    const vectorProperty = `rel.${this.quoteIdentifier(vectorPropertyName)}`
    qb.append(`UNWIND $updates AS u`)
    qb.append(
      `OPTIONAL MATCH (record:${RUSHDB_LABEL_RECORD}${labelSuffix} { ${projectIdInline()} }) WHERE record.${this.quoteIdentifier(RUSHDB_KEY_ID)} = u.recordId`
    )
    qb.append(
      `OPTIONAL MATCH (prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })-[rel:${RUSHDB_RELATION_VALUE}]->(record)`
    )
    qb.append(
      `FOREACH (_ IN CASE WHEN rel IS NULL THEN [] ELSE [1] END | SET ${vectorProperty} = u.emb, rel.__projectId = $projectId, rel.__propKey = $propKey)`
    )
    qb.append(
      `RETURN count(*) AS requestedCount, count(CASE WHEN rel IS NOT NULL THEN 1 END) AS updatedCount`
    )
    return qb.getQuery()
  }

  /**
   * Count of VALUE relationships that still need this label's embedding (for progress tracking).
   * @param labelSuffix - Neo4j label suffix e.g. ":Book"
   */
  getUnindexedCountQuery(labelSuffix: string, vectorPropertyName: string) {
    const qb = new QueryBuilder()
    const vectorProperty = `rel.${this.quoteIdentifier(vectorPropertyName)}`
    qb.append(
      `MATCH (record:${RUSHDB_LABEL_RECORD}${labelSuffix} { ${projectIdInline()} })<-[rel:${RUSHDB_RELATION_VALUE}]-(prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })`
    )
    qb.append(
      `WHERE (${vectorProperty} IS NULL OR rel.__propKey <> $propKey) AND record[prop.name] IS NOT NULL`
    )
    qb.append(`RETURN count(rel) AS remaining`)
    return qb.getQuery()
  }

  /**
   * Exact semantic search query.
   * Candidate records are narrowed via Cypher MATCH/WHERE first, then ranked with
   * vector similarity over stored relationship vectors.
   *
   * @param combinedWhere      - compiled Cypher WHERE expression for the root record (may be empty)
   * @param labelSuffix        - Neo4j label suffix e.g. ":Book"
   * @param extraMatchClauses  - optional OPTIONAL MATCH clauses for related-record traversal
   * @param nodeAliases        - all node aliases referenced by the where clause (default: ['record'])
   * @param requiredAliasCheck - e.g. "record IS NOT NULL AND record1 IS NOT NULL" produced by
   *                             parseWhereClause(...).where; enforces that traversed nodes existed
   */
  getSemanticSearchPrefilterQuery({
    combinedWhere,
    labelSuffix,
    similarityFunction,
    vectorPropertyName,
    extraMatchClauses = [],
    nodeAliases = ['record'],
    requiredAliasCheck = ''
  }: {
    combinedWhere: string
    labelSuffix: string
    similarityFunction: 'cosine' | 'euclidean'
    vectorPropertyName: string
    extraMatchClauses?: string[]
    nodeAliases?: string[]
    requiredAliasCheck?: string
  }) {
    const qb = new QueryBuilder()
    const vectorProperty = `rel.${this.quoteIdentifier(vectorPropertyName)}`
    qb.append(`MATCH (record:${RUSHDB_LABEL_RECORD}${labelSuffix} { ${projectIdInline()} })`)
    if (combinedWhere) {
      qb.append(`WHERE ${combinedWhere}`)
    }
    // Inject OPTIONAL MATCH clauses for nested where (related-record traversal predicates).
    for (const clause of extraMatchClauses) {
      qb.append(clause)
    }
    // Enforce that traversed nodes were actually found (converts OPTIONAL to required).
    if (extraMatchClauses.length > 0 && requiredAliasCheck) {
      qb.append(`WITH ${nodeAliases.join(', ')} WHERE ${requiredAliasCheck}`)
    }
    qb.append(
      `MATCH (prop:${RUSHDB_LABEL_PROPERTY} { name: $propertyName, projectId: $projectId })-[rel:${RUSHDB_RELATION_VALUE}]->(record)`
    )
    qb.append(`WHERE ${vectorProperty} IS NOT NULL AND rel.__propKey = $propKey`)
    qb.append(
      `WITH record, vector.similarity.${similarityFunction}(${vectorProperty}, $queryVector) AS score`
    )
    qb.append(`WHERE score IS NOT NULL`)
    qb.append(`ORDER BY score DESC`)
    qb.append(`SKIP $skip LIMIT $limit`)
    qb.append(`RETURN record, score`)
    return qb.getQuery()
  }
}
