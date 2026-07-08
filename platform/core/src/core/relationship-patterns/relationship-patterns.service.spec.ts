import { SchemaItem } from '@/core/ai/ai.types'
import { EntityQueryService } from '@/core/entity/entity-query.service'

import { RelationshipPatternsService } from './relationship-patterns.service'
import { RelationshipPatternCandidate } from './relationship-patterns.types'

type TestableRelationshipPatternsService = {
  validateCandidate(
    candidate: RelationshipPatternCandidate,
    schema: SchemaItem[]
  ): RelationshipPatternCandidate | undefined
  buildCandidateHints(schema: SchemaItem[]): RelationshipPatternCandidate[]
  probeJoinCandidates(
    projectId: string,
    candidates: RelationshipPatternCandidate[]
  ): Promise<RelationshipPatternCandidate[]>
}

const folderSchema: SchemaItem[] = [
  {
    label: 'Folder',
    count: 3,
    properties: [
      { id: 'p1', name: 'id', type: 'string', values: ['folder_root', 'folder_child'] },
      { id: 'p2', name: 'parentId', type: 'string', values: ['folder_root'] },
      { id: 'p3', name: 'name', type: 'string' }
    ],
    relationships: []
  }
]

const employeeSchema: SchemaItem[] = [
  {
    label: 'Employee',
    count: 3,
    properties: [
      { id: 'p1', name: 'id', type: 'string', values: ['employee_alice', 'employee_bob'] },
      { id: 'p2', name: 'mentorId', type: 'string', values: ['employee_alice'] },
      { id: 'p3', name: 'name', type: 'string' }
    ],
    relationships: []
  }
]

const starWarsSchema: SchemaItem[] = [
  {
    label: 'CHARACTER',
    count: 112,
    properties: [
      {
        id: 'p1',
        name: 'name',
        type: 'string',
        values: ['Luke Skywalker', 'Leia Organa', 'Han Solo', 'Chewbacca', 'Darth Vader']
      },
      { id: 'p2', name: 'species', type: 'string', values: ['Human', 'Wookiee'] },
      {
        id: 'p9',
        name: 'force_sensitive',
        type: 'boolean',
        values: [true as unknown as string, false as unknown as string]
      },
      {
        id: 'p13',
        name: 'is_droid',
        type: 'boolean',
        values: [true as unknown as string, false as unknown as string]
      },
      {
        id: 'p10',
        name: 'mentor_character_ids',
        type: 'string',
        isArray: true,
        values: ['character_obi_wan_kenobi', 'character_yoda', 'character_qui_gon_jinn']
      },
      {
        id: 'p11',
        name: 'apprentice_character_ids',
        type: 'string',
        isArray: true,
        values: ['character_luke_skywalker', 'character_obi_wan_kenobi']
      },
      {
        id: 'p12',
        name: 'id',
        type: 'string',
        values: ['character_obi_wan_kenobi', 'character_yoda', 'character_luke_skywalker']
      }
    ],
    relationships: []
  },
  {
    label: 'STARSHIP',
    count: 56,
    properties: [
      {
        id: 'p3',
        name: 'name',
        type: 'string',
        values: ['Millennium Falcon', 'X-wing', 'TIE Fighter', 'Star Destroyer']
      },
      {
        id: 'p4',
        name: 'pilots',
        type: 'string',
        values: ['Han Solo, Chewbacca', 'Luke Skywalker', 'None']
      }
    ],
    relationships: []
  },
  {
    label: 'BATTLE',
    count: 2,
    properties: [
      {
        id: 'p5',
        name: 'id',
        type: 'string',
        values: ['battle_naboo', 'battle_geonosis']
      },
      {
        id: 'p6',
        name: 'commander_character_ids',
        type: 'string',
        isArray: true,
        values: ['character_padme_amidala', 'character_boss_nass', 'character_luke_skywalker']
      }
    ],
    relationships: []
  }
]

const makeService = (overrides: Partial<Record<'neogmaService' | 'entityService', unknown>> = {}) =>
  new RelationshipPatternsService(
    {} as never,
    {} as never,
    {} as never,
    (overrides.neogmaService ?? {}) as never,
    {} as never,
    {} as never,
    (overrides.entityService ?? {}) as never
  ) as unknown as TestableRelationshipPatternsService

describe('RelationshipPatternsService', () => {
  const service = makeService()

  // Validation is structural plausibility only: semantics are the LLM's judgment and
  // evidence comes from the live-graph probe — sampled values never gate a candidate,
  // because tiny samples of high-cardinality data (thousands of account ids) carry no
  // overlap signal even for perfectly joinable columns.
  describe('validateCandidate', () => {
    it('accepts same-label self-reference joins', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'Folder', key: 'id' },
          target: { label: 'Folder', key: 'parentId' },
          direction: 'out',
          type: 'CONTAINS',
          mode: 'join_pattern',
          confidence: 0.91
        },
        folderSchema
      )

      expect(candidate).toMatchObject({
        source: { label: 'Folder', key: 'id' },
        target: { label: 'Folder', key: 'parentId' },
        type: 'CONTAINS',
        mode: 'join_pattern'
      })
    })

    it('rejects same-label joins that would only match a record to itself', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'Folder', key: 'id' },
          target: { label: 'Folder', key: 'id' },
          direction: 'out',
          type: 'RELATED_TO',
          mode: 'join_pattern',
          confidence: 0.91
        },
        folderSchema
      )

      expect(candidate).toBeUndefined()
    })

    it('accepts same-label joins without requiring hardcoded key names', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'Employee', key: 'id' },
          target: { label: 'Employee', key: 'mentorId' },
          direction: 'out',
          type: 'MENTORS',
          mode: 'join_pattern',
          confidence: 0.88
        },
        employeeSchema
      )

      expect(candidate).toMatchObject({
        source: { label: 'Employee', key: 'id' },
        target: { label: 'Employee', key: 'mentorId' },
        type: 'MENTORS',
        mode: 'join_pattern'
      })
    })

    it('passes cross-label joins through to graph probing instead of judging sampled overlap', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'CHARACTER', key: 'name' },
          target: { label: 'STARSHIP', key: 'name' },
          direction: 'out',
          type: 'RELATED_TO',
          mode: 'join_pattern',
          confidence: 0.9
        },
        starWarsSchema
      )

      // Structurally plausible — whether any records actually match is the probe's call.
      expect(candidate).toBeDefined()
    })

    it('rejects joins on non-reference value types', () => {
      expect(
        service.validateCandidate(
          {
            source: { label: 'CHARACTER', key: 'force_sensitive' },
            target: { label: 'CHARACTER', key: 'is_droid' },
            direction: 'out',
            type: 'HAS_FORCE_SENSITIVE',
            mode: 'join_pattern',
            confidence: 0.9
          },
          starWarsSchema
        )
      ).toBeUndefined()
    })

    it('rejects joins between two list-valued reference fields', () => {
      expect(
        service.validateCandidate(
          {
            source: { label: 'CHARACTER', key: 'mentor_character_ids' },
            target: { label: 'CHARACTER', key: 'apprentice_character_ids' },
            direction: 'out',
            type: 'MENTORS',
            mode: 'join_pattern',
            confidence: 0.95
          },
          starWarsSchema
        )
      ).toBeUndefined()
    })

    it('accepts comma-separated reference fields joining scalar fields', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'STARSHIP', key: 'pilots' },
          target: { label: 'CHARACTER', key: 'name' },
          direction: 'out',
          type: 'HAS_PILOT',
          mode: 'join_pattern',
          confidence: 0.9
        },
        starWarsSchema
      )

      expect(candidate).toMatchObject({
        source: { label: 'STARSHIP', key: 'pilots' },
        target: { label: 'CHARACTER', key: 'name' },
        type: 'HAS_PILOT',
        mode: 'join_pattern'
      })
    })

    it('accepts array reference fields joining scalar fields', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'BATTLE', key: 'commander_character_ids' },
          target: { label: 'CHARACTER', key: 'id' },
          direction: 'out',
          type: 'HAS_COMMANDER_CHARACTER',
          mode: 'join_pattern',
          confidence: 0.9
        },
        starWarsSchema
      )

      expect(candidate).toMatchObject({
        source: { label: 'BATTLE', key: 'commander_character_ids' },
        target: { label: 'CHARACTER', key: 'id' },
        type: 'HAS_COMMANDER_CHARACTER',
        mode: 'join_pattern'
      })
    })
  })

  // Hints are derived from names and types only and handed to the LLM for semantic
  // vetting — they are hypotheses, not suggestions.
  describe('buildCandidateHints', () => {
    it('hints name-backed reference fields regardless of sampled values', () => {
      expect(service.buildCandidateHints(starWarsSchema)).toContainEqual(
        expect.objectContaining({
          source: { label: 'BATTLE', key: 'commander_character_ids' },
          target: { label: 'CHARACTER', key: 'id' },
          type: 'HAS_COMMANDER_CHARACTER_ID',
          mode: 'join_pattern'
        })
      )
    })

    it('does not hint boolean columns or list-to-list pairs', () => {
      const hints = service.buildCandidateHints(starWarsSchema)

      expect(hints).not.toContainEqual(
        expect.objectContaining({
          source: expect.objectContaining({ key: 'force_sensitive' })
        })
      )
      expect(hints).not.toContainEqual(
        expect.objectContaining({
          source: expect.objectContaining({ key: 'mentor_character_ids' }),
          target: expect.objectContaining({ key: 'apprentice_character_ids' })
        })
      )
    })
  })

  // Same-label chain/flow patterns: two reference-like columns on ONE label identifying
  // the same kind of entity (receiver_account / sender_account). Detected purely from
  // names — shared entity token + opposing direction words. Sampled values are ignored:
  // on real data (thousands of accounts) 10-value samples of the two columns virtually
  // never overlap even though the graph joins on them everywhere.
  describe('same-label chain hints', () => {
    const transactionsSchema: SchemaItem[] = [
      {
        label: 'TRANSACTION',
        count: 25,
        properties: [
          { id: 'p1', name: 'transaction_id', type: 'string', values: ['t1001', 't1002', 't2001'] },
          {
            id: 'p2',
            name: 'sender_account',
            type: 'string',
            values: ['acc_003', 'acc_006', 'acc_017']
          },
          {
            id: 'p3',
            name: 'receiver_account',
            type: 'string',
            // Deliberately no overlap with sender_account samples — must not matter.
            values: ['acc_201', 'acc_205', 'acc_311']
          },
          { id: 'p4', name: 'amount', type: 'number', values: [54.2, 9800, 42000] as any },
          { id: 'p5', name: 'currency', type: 'string', values: ['USD'] },
          { id: 'p6', name: 'status', type: 'string', values: ['posted', 'pending'] },
          { id: 'p7', name: 'previous_status', type: 'string', values: ['pending', 'created'] }
        ],
        relationships: []
      }
    ]

    it('hints chaining records through destination/origin reference columns without sampled overlap', () => {
      expect(service.buildCandidateHints(transactionsSchema)).toContainEqual(
        expect.objectContaining({
          source: { label: 'TRANSACTION', key: 'receiver_account' },
          target: { label: 'TRANSACTION', key: 'sender_account' },
          direction: 'out',
          type: 'FLOWS_TO_ACCOUNT',
          mode: 'join_pattern'
        })
      )
    })

    it('passes end-to-end candidate validation', () => {
      const validated = service.validateCandidate(
        {
          source: { label: 'TRANSACTION', key: 'receiver_account' },
          target: { label: 'TRANSACTION', key: 'sender_account' },
          direction: 'out',
          type: 'FLOWS_TO_ACCOUNT',
          mode: 'join_pattern',
          confidence: 0.82
        },
        transactionsSchema
      )
      expect(validated).toBeDefined()
      expect(validated?.type).toBe('FLOWS_TO_ACCOUNT')
    })

    it('does not hint columns whose names carry no flow direction (status, previous_status)', () => {
      const hints = service.buildCandidateHints(transactionsSchema)
      expect(hints).not.toContainEqual(
        expect.objectContaining({
          source: expect.objectContaining({ key: expect.stringContaining('status') })
        })
      )
      expect(hints).not.toContainEqual(
        expect.objectContaining({
          target: expect.objectContaining({ key: expect.stringContaining('status') })
        })
      )
    })

    it('infers flow direction from from/to naming', () => {
      const legsSchema: SchemaItem[] = [
        {
          label: 'RouteLeg',
          count: 10,
          properties: [
            {
              id: 'p1',
              name: 'from_station',
              type: 'string',
              values: ['st_alpha', 'st_beta']
            },
            {
              id: 'p2',
              name: 'to_station',
              type: 'string',
              values: ['st_gamma', 'st_omega']
            }
          ],
          relationships: []
        }
      ]

      expect(service.buildCandidateHints(legsSchema)).toContainEqual(
        expect.objectContaining({
          source: { label: 'RouteLeg', key: 'to_station' },
          target: { label: 'RouteLeg', key: 'from_station' },
          type: 'FLOWS_TO_STATION'
        })
      )
    })

    it('leaves direction-ambiguous pairs to the LLM but validates them as plausible joins', () => {
      const pairSchema: SchemaItem[] = [
        {
          label: 'Merger',
          count: 8,
          properties: [
            {
              id: 'p1',
              name: 'primary_company',
              type: 'string',
              values: ['co_ax', 'co_bx']
            },
            {
              id: 'p2',
              name: 'secondary_company',
              type: 'string',
              values: ['co_cx', 'co_ex']
            }
          ],
          relationships: []
        }
      ]

      // No deterministic hint — the column names carry no flow direction…
      expect(service.buildCandidateHints(pairSchema)).toEqual([])
      // …but an LLM-proposed candidate for the pair survives validation.
      const validated = service.validateCandidate(
        {
          source: { label: 'Merger', key: 'primary_company' },
          target: { label: 'Merger', key: 'secondary_company' },
          direction: 'out',
          type: 'MERGED_WITH',
          mode: 'join_pattern',
          confidence: 0.7
        },
        pairSchema
      )
      expect(validated).toBeDefined()
    })

    it('does not pair columns whose only shared name token is a generic reference suffix', () => {
      const refSchema: SchemaItem[] = [
        {
          label: 'Asset',
          count: 8,
          properties: [
            { id: 'p1', name: 'owner_ref', type: 'string', values: ['x_1', 'x_2', 'x_3'] },
            { id: 'p2', name: 'region_ref', type: 'string', values: ['x_2', 'x_3', 'x_4'] }
          ],
          relationships: []
        }
      ]
      expect(service.buildCandidateHints(refSchema)).toEqual([])
    })
  })

  // The probe is the evidence gate: a join candidate survives only when the live graph
  // actually contains at least one matching record pair.
  describe('probeJoinCandidates', () => {
    const makeProbeService = (matchCounts: number[]) => {
      const transaction = {
        commit: jest.fn().mockResolvedValue(undefined),
        rollback: jest.fn().mockResolvedValue(undefined),
        isOpen: jest.fn().mockReturnValue(false)
      }
      const countRelationCandidatesByKeys = jest.fn()
      for (const count of matchCounts) {
        countRelationCandidatesByKeys.mockResolvedValueOnce(count)
      }
      const probeService = makeService({
        neogmaService: {
          createSession: jest.fn().mockReturnValue({ beginTransaction: () => transaction }),
          closeSession: jest.fn().mockResolvedValue(undefined)
        },
        entityService: { countRelationCandidatesByKeys }
      })
      return { probeService, countRelationCandidatesByKeys }
    }

    const joinCandidate = (key: string): RelationshipPatternCandidate => ({
      source: { label: 'TRANSACTION', key: 'receiver_account' },
      target: { label: 'TRANSACTION', key },
      direction: 'out',
      type: 'FLOWS_TO_ACCOUNT',
      mode: 'join_pattern',
      confidence: 0.82
    })

    it('keeps candidates with matching pairs and records the probed count as evidence', async () => {
      const { probeService } = makeProbeService([42])

      const evidenced = await probeService.probeJoinCandidates('project-1', [joinCandidate('sender_account')])

      expect(evidenced).toEqual([expect.objectContaining({ sampleMatchCount: 42 })])
    })

    it('drops join candidates the live graph shows no matching pairs for', async () => {
      const { probeService } = makeProbeService([0, 7])

      const evidenced = await probeService.probeJoinCandidates('project-1', [
        joinCandidate('sender_account'),
        joinCandidate('sender_name')
      ])

      expect(evidenced).toEqual([
        expect.objectContaining({
          target: expect.objectContaining({ key: 'sender_name' }),
          sampleMatchCount: 7
        })
      ])
    })

    it('passes retype candidates through without probing', async () => {
      const { probeService, countRelationCandidatesByKeys } = makeProbeService([1])

      const retype: RelationshipPatternCandidate = {
        source: { label: 'ORDER' },
        target: { label: 'CUSTOMER' },
        direction: 'out',
        type: 'PLACED_BY',
        mode: 'retype_existing_relationship',
        confidence: 0.9
      }
      const evidenced = await probeService.probeJoinCandidates('project-1', [
        retype,
        joinCandidate('sender_account')
      ])

      expect(evidenced).toHaveLength(2)
      expect(countRelationCandidatesByKeys).toHaveBeenCalledTimes(1)
    })
  })
})

describe('EntityQueryService relationship creation', () => {
  it('excludes self-links when creating relationships within the same label', () => {
    const query = new EntityQueryService().createRelationsByKeys({
      sourceLabel: 'Folder',
      sourceKey: 'id',
      targetLabel: 'Folder',
      targetKey: 'parentId',
      relationType: 'CONTAINS',
      direction: 'out'
    })

    expect(query).toContain('id(s) <> id(t)')
    expect(query).toContain('any(sourceValue IN CASE WHEN s.`id` IS NULL THEN []')
    expect(query).toContain('sourceValue IN CASE WHEN t.`parentId` IS NULL THEN []')
  })

  it('matches array-valued source keys to scalar target keys', () => {
    const query = new EntityQueryService().createRelationsByKeys({
      sourceLabel: 'BATTLE',
      sourceKey: 'commander_character_ids',
      targetLabel: 'CHARACTER',
      targetKey: 'id',
      relationType: 'HAS_COMMANDER',
      direction: 'out'
    })

    expect(query).toContain('s.`commander_character_ids` IS :: LIST<ANY>')
    expect(query).toContain('sourceValue IN CASE WHEN t.`id` IS NULL THEN []')
  })

  it('joins via a single-pass key map instead of re-matching the target label per source row', () => {
    const query = new EntityQueryService().createRelationsByKeys({
      sourceLabel: 'A',
      sourceKey: 'key',
      targetLabel: 'B',
      targetKey: 'key',
      relationType: 'REL',
      direction: 'out'
    })

    // Target set is collected once into a key→targets map inside an isolated CALL scope…
    expect(query).toContain('CALL { MATCH (t:')
    expect(query).toContain('apoc.map.fromPairs(collect([joinKey, targets])) AS targetsByKey')
    // …then source rows probe the map; the pair statement emits distinct pairs
    expect(query).toContain('targetsByKey[toString(sourceValue)] AS candidates WHERE candidates IS NOT NULL')
    expect(query).toContain('RETURN DISTINCT s, t')
    // …and the write statement handles the batch itself (no per-row target re-match)
    expect(query).toContain('batchMode: "BATCH_SINGLE"')
    expect(query).toContain('UNWIND $_batch AS row WITH row.s AS s, row.t AS t MERGE (s)-[rel:REL]->(t)')
    expect(query).not.toContain('apoc.meta.cypher.type')
  })

  it('allows an unscoped source for key joins (linear cost), including with manyToMany set', () => {
    const service = new EntityQueryService()
    expect(() =>
      service.createRelationsByKeys({
        sourceLabel: 'A',
        sourceKey: 'key',
        targetLabel: 'B',
        targetKey: 'key',
        manyToMany: true
      })
    ).not.toThrow()
  })

  it('still rejects a cartesian manyToMany without where filters on both sides', () => {
    const service = new EntityQueryService()
    expect(() =>
      service.createRelationsByKeys({
        sourceLabel: 'A',
        targetLabel: 'B',
        manyToMany: true,
        targetWhere: { key: 'x' }
      })
    ).toThrow(/non-empty `where` filters/)
  })

  it('collects the target set once for the cartesian manyToMany path', () => {
    const query = new EntityQueryService().createRelationsByKeys({
      sourceLabel: 'A',
      targetLabel: 'B',
      manyToMany: true,
      sourceWhere: { key: 'x' },
      targetWhere: { key: 'y' }
    })

    expect(query).toContain('RETURN collect(DISTINCT t) AS targets')
    expect(query).toContain('UNWIND targets AS t')
  })

  it('deletes through the same pair-producing statement', () => {
    const query = new EntityQueryService().deleteRelationsByKeys({
      sourceLabel: 'A',
      sourceKey: 'key',
      targetLabel: 'B',
      targetKey: 'key',
      relationType: 'REL',
      direction: 'out'
    })

    expect(query).toContain('apoc.map.fromPairs(collect([joinKey, targets])) AS targetsByKey')
    expect(query).toContain(
      'UNWIND $_batch AS row WITH row.s AS s, row.t AS t OPTIONAL MATCH (s)-[rel:REL]->(t) DELETE rel'
    )
    expect(query).toContain('batchMode: "BATCH_SINGLE"')
  })

  it('probes join candidates through the same pair statement with a bounded count', () => {
    const query = new EntityQueryService().countRelationCandidatesByKeys({
      sourceLabel: 'TRANSACTION',
      sourceKey: 'receiver_account',
      targetLabel: 'TRANSACTION',
      targetKey: 'sender_account',
      limit: 100
    })

    // Same join semantics as apply: key map, type-strict pair filter, self-exclusion…
    expect(query).toContain('apoc.map.fromPairs(collect([joinKey, targets])) AS targetsByKey')
    expect(query).toContain('id(s) <> id(t)')
    // …but read-only, with cost bounded by the probe limit.
    expect(query).toContain('WITH s, t LIMIT 100')
    expect(query).toContain('RETURN count(*) AS matchCount')
    expect(query).not.toContain('MERGE')
  })
})
