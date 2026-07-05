import { SchemaItem } from '@/core/ai/ai.types'
import { EntityQueryService } from '@/core/entity/entity-query.service'

import { RelationshipPatternsService } from './relationship-patterns.service'
import { RelationshipPatternCandidate } from './relationship-patterns.types'

type TestableRelationshipPatternsService = {
  validateCandidate(
    candidate: RelationshipPatternCandidate,
    schema: SchemaItem[]
  ): RelationshipPatternCandidate | undefined
  suggestDeterministicCandidates(schema: SchemaItem[]): RelationshipPatternCandidate[]
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
  },
  {
    label: 'BATCH_CHARACTER',
    count: 3,
    properties: [
      {
        id: 'p7',
        name: 'id',
        type: 'string',
        values: ['character_padme_amidala', 'character_boss_nass', 'character_luke_skywalker']
      },
      { id: 'p8', name: 'name', type: 'string', values: ['Padme Amidala', 'Boss Nass'] }
    ],
    relationships: []
  }
]

describe('RelationshipPatternsService', () => {
  const service = new RelationshipPatternsService(
    {} as never,
    {} as never,
    {} as never,
    {} as never,
    {} as never,
    {} as never,
    {} as never
  ) as unknown as TestableRelationshipPatternsService

  describe('validateCandidate', () => {
    it('accepts same-label joins with sampled value overlap', () => {
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

    it('rejects cross-label joins without sampled value overlap', () => {
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

      expect(candidate).toBeUndefined()
    })

    it('rejects boolean and categorical enum overlap as relationship evidence', () => {
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

    it('accepts reference fields with sampled overlap', () => {
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

    it('accepts array reference fields with sampled overlap', () => {
      const candidate = service.validateCandidate(
        {
          source: { label: 'BATTLE', key: 'commander_character_ids' },
          target: { label: 'BATCH_CHARACTER', key: 'id' },
          direction: 'out',
          type: 'HAS_COMMANDER_CHARACTER',
          mode: 'join_pattern',
          confidence: 0.9
        },
        starWarsSchema
      )

      expect(candidate).toMatchObject({
        source: { label: 'BATTLE', key: 'commander_character_ids' },
        target: { label: 'BATCH_CHARACTER', key: 'id' },
        type: 'HAS_COMMANDER_CHARACTER',
        mode: 'join_pattern'
      })
    })

    it('accepts name-backed reference fields even without sampled overlap', () => {
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

  describe('suggestDeterministicCandidates', () => {
    it('suggests joins from sampled overlap', () => {
      expect(service.suggestDeterministicCandidates(starWarsSchema)).toContainEqual(
        expect.objectContaining({
          source: { label: 'BATTLE', key: 'commander_character_ids' },
          target: { label: 'CHARACTER', key: 'id' },
          type: 'HAS_COMMANDER_CHARACTER_ID',
          mode: 'join_pattern'
        })
      )
      expect(service.suggestDeterministicCandidates(starWarsSchema)).toContainEqual(
        expect.objectContaining({
          source: { label: 'STARSHIP', key: 'pilots' },
          target: { label: 'CHARACTER', key: 'name' },
          type: 'HAS_PILOT',
          mode: 'join_pattern'
        })
      )
    })

    it('does not suggest enum or list-to-list joins', () => {
      const suggestions = service.suggestDeterministicCandidates(starWarsSchema)

      expect(suggestions).not.toContainEqual(
        expect.objectContaining({
          source: expect.objectContaining({ key: 'force_sensitive' })
        })
      )
      expect(suggestions).not.toContainEqual(
        expect.objectContaining({
          source: expect.objectContaining({ key: 'mentor_character_ids' }),
          target: expect.objectContaining({ key: 'apprentice_character_ids' })
        })
      )
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
})
