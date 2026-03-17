import { Injectable, Logger } from '@nestjs/common'
import { Transaction } from 'neo4j-driver'

import { AiQueryService } from '@/core/ai/ai-query.service'
import type { OntologyItem, OntologyProperty, OntologyRelationship } from '@/core/ai/ai.types'
import { isDevMode } from '@/common/utils/isDevMode'
import { KuEventsService } from '@/core/ku-events/ku-events.service'
import { KuOperation } from '@/core/ku-events/ku-events.constants'
import { ProjectRepository } from '@/dashboard/project/model/project.repository'

/** How long (ms) a cached ontology is considered fresh before a recalculation is triggered. */
const ONTOLOGY_CACHE_TTL_MS = 60 * 60 * 1000 // 1 hour

@Injectable()
export class AiService {
  constructor(
    private readonly aiQueryService: AiQueryService,
    private readonly kuEventsService: KuEventsService,
    private readonly projectRepository: ProjectRepository
  ) {}

  async getOntology({
    projectId,
    workspaceId,
    labels,
    transaction
  }: {
    projectId: string
    /** When provided, a COMPUTE_OPERATION KU event is emitted on cache miss / recalculation. */
    workspaceId?: string
    labels?: string[]
    transaction: Transaction
  }): Promise<OntologyItem[]> {
    const tTotal = Date.now()

    // ── 1. Check persisted cache in SQL projects table ────────────────────────
    const projectRow = await this.projectRepository.findById(projectId)
    const cachedAt: string | null = projectRow?.ontologyCachedAt ?? null
    const cacheJson: string | null = projectRow?.ontologyCache ?? null

    if (cachedAt && cacheJson) {
      const ageMs = Date.now() - new Date(cachedAt).getTime()
      if (ageMs < ONTOLOGY_CACHE_TTL_MS) {
        // Cache is fresh — deserialise, optionally filter, return immediately
        const fullOntology: OntologyItem[] = JSON.parse(cacheJson)
        const result =
          labels?.length ? fullOntology.filter((item) => labels.includes(item.label)) : fullOntology

        isDevMode(() =>
          Logger.debug(
            `[AiService] ontology: cache hit — age ${Math.round(ageMs / 1000)}s, ${result.length}/${fullOntology.length} labels, total ${Date.now() - tTotal}ms`
          )
        )
        return result
      }
    }

    // ── 2. Cache is stale/missing — recalculate the FULL ontology ──────────
    // Always run without a label filter so the persisted cache is complete.
    // Label filtering (if requested) is applied in-memory after building.
    const params: Record<string, any> = { projectId }

    const t0 = Date.now()

    // Run Q1, Q2, Q3 in parallel — all are read-only
    const [labelsResult, propertiesResult, relationshipsResult] = await Promise.all([
      transaction.run(this.aiQueryService.getLabelsQuery(), params),
      transaction.run(this.aiQueryService.getPropertiesWithValuesQuery(), params),
      transaction.run(this.aiQueryService.getRelationshipsQuery(), params)
    ])

    // ---------- Build label → count map (Q1) ----------
    const labelCountMap = new Map<string, number>()
    for (const r of labelsResult.records) {
      const label = r.get('label') as string
      const count = (r.get('recordCount') as any)?.toNumber?.() ?? Number(r.get('recordCount'))
      labelCountMap.set(label, count)
    }

    // ---------- Build label → properties map (Q2) ----------
    const labelPropsMap = new Map<string, OntologyProperty[]>()
    for (const r of propertiesResult.records) {
      const label = r.get('label') as string
      const prop: OntologyProperty = {
        id: r.get('propId') as string,
        name: r.get('propName') as string,
        type: r.get('propType') as string
      }

      const sampleValues = r.get('sampleValues')
      const minValue = r.get('minValue')
      const maxValue = r.get('maxValue')

      if (sampleValues != null) {
        prop.values = sampleValues
      }
      if (minValue != null) {
        prop.min = typeof minValue === 'object' && minValue?.toNumber ? minValue.toNumber() : minValue
      }
      if (maxValue != null) {
        prop.max = typeof maxValue === 'object' && maxValue?.toNumber ? maxValue.toNumber() : maxValue
      }

      if (!labelPropsMap.has(label)) {
        labelPropsMap.set(label, [])
      }
      labelPropsMap.get(label)!.push(prop)
    }

    // ---------- Build label → relationships map (Q3) ----------
    // Relationships are stored bidirectionally: fromLabel gets 'out', toLabel gets 'in'.
    const labelRelsMap = new Map<string, OntologyRelationship[]>()

    const addRel = (label: string, rel: OntologyRelationship) => {
      if (!labelRelsMap.has(label)) labelRelsMap.set(label, [])
      const existing = labelRelsMap.get(label)!
      const key = `${rel.direction}|${rel.label}|${rel.type}`
      if (!existing.some((e) => `${e.direction}|${e.label}|${e.type}` === key)) {
        existing.push(rel)
      }
    }

    for (const r of relationshipsResult.records) {
      const fromLabel = r.get('fromLabel') as string
      const relType = r.get('relType') as string
      const toLabel = r.get('toLabel') as string

      if (fromLabel === toLabel) continue // skip self-loops

      addRel(fromLabel, { label: toLabel, type: relType, direction: 'out' })
      addRel(toLabel, { label: fromLabel, type: relType, direction: 'in' })
    }

    // ---------- Merge into OntologyItem[] ----------
    // Include all labels from Q1 plus any additional labels seen in Q2/Q3
    const allLabels = new Set<string>([
      ...labelCountMap.keys(),
      ...labelPropsMap.keys(),
      ...labelRelsMap.keys()
    ])

    const ontology: OntologyItem[] = []

    for (const label of allLabels) {
      const rels = labelRelsMap.get(label) ?? []
      rels.sort((a, b) => {
        if (a.direction !== b.direction) return a.direction < b.direction ? -1 : 1
        if (a.label !== b.label) return a.label.localeCompare(b.label)
        return a.type.localeCompare(b.type)
      })

      ontology.push({
        label,
        count: labelCountMap.get(label) ?? 0,
        properties: labelPropsMap.get(label) ?? [],
        relationships: rels
      })
    }

    // Sort by count descending for consistent output
    ontology.sort((a, b) => b.count - a.count)

    const elapsed = Date.now() - t0

    isDevMode(() =>
      Logger.debug(
        `[AiService] ontology: recalculated in ${elapsed}ms (${ontology.length} labels, ${propertiesResult.records.length} properties, ${relationshipsResult.records.length} relationships)`
      )
    )

    // Emit COMPUTE_OPERATION only when a full recalculation was performed
    if (workspaceId) {
      this.kuEventsService.emit(workspaceId, projectId, KuOperation.COMPUTE_OPERATION, {
        type: 'ontology'
      })
    }

    // ── 3. Persist full ontology to SQL projects table for subsequent cache hits ──
    const nowIso = new Date().toISOString()
    try {
      await this.projectRepository.update(projectId, {
        ontologyCache: JSON.stringify(ontology),
        ontologyCachedAt: nowIso
      })
    } catch (err) {
      // Cache write is best-effort — log but never fail the caller
      console.warn('[AiService] failed to persist ontology cache:', err)
    }

    // ── 4. Return (apply in-memory label filter if requested) ─────────────────
    const result = labels?.length ? ontology.filter((item) => labels.includes(item.label)) : ontology
    isDevMode(() =>
      Logger.debug(
        `[AiService] ontology: total ${Date.now() - tTotal}ms (recalc ${elapsed}ms, ${result.length}/${ontology.length} labels returned)`
      )
    )
    return result
  }

  buildMdSchema(ontology: OntologyItem[]): string {
    const esc = (s: string) => String(s).replace(/\|/g, '\\|').replace(/`/g, '\\`')
    const MAX_DISPLAY_VALUES = 5
    const MAX_VALUE_LEN = 48 // truncate individual string values longer than this

    const truncate = (s: string): string =>
      s.length > MAX_VALUE_LEN ? s.slice(0, MAX_VALUE_LEN) + '\u2026' : s

    const fmtValues = (prop: OntologyProperty): string => {
      const t = (prop.type || '').toLowerCase()

      if (t === 'number' || t === 'datetime') {
        if (prop.min !== undefined && prop.max !== undefined) {
          return `\`${prop.min}\`..\`${prop.max}\``
        }
        return '—'
      }

      if (Array.isArray(prop.values) && prop.values.length > 0) {
        const shown = prop.values.slice(0, MAX_DISPLAY_VALUES)
        const tail =
          prop.values.length > MAX_DISPLAY_VALUES ? ` (+${prop.values.length - MAX_DISPLAY_VALUES} more)` : ''
        return shown.map((v) => `\`${esc(truncate(String(v)))}\``).join(', ') + tail
      }

      return '—'
    }

    let md = `# Graph Ontology\n\n`

    // Labels summary table
    md += `## Labels\n\n`
    md += `| Label | Count |\n|-------|------:|\n`
    for (const item of ontology) {
      md += `| \`${esc(item.label)}\` | ${item.count} |\n`
    }
    md += `\n`

    // Per-label detail
    for (const item of ontology) {
      const recordWord = item.count === 1 ? 'record' : 'records'
      md += `---\n\n## \`${esc(item.label)}\` (${item.count} ${recordWord})\n\n`

      if (item.properties.length > 0) {
        md += `### Properties\n\n`
        md += `| Property | Type | Values / Range |\n|----------|------|----------------|\n`
        for (const p of item.properties) {
          md += `| \`${esc(p.name)}\` | ${esc(p.type)} | ${fmtValues(p)} |\n`
        }
        md += `\n`
      }

      if (item.relationships.length > 0) {
        md += `### Relationships\n\n`
        md += `| Type | Direction | Other Label |\n|------|-----------|-------------|\n`
        for (const r of item.relationships) {
          md += `| \`${esc(r.type)}\` | ${r.direction} | \`${esc(r.label)}\` |\n`
        }
        md += `\n`
      }
    }

    return md.trim()
  }
}
