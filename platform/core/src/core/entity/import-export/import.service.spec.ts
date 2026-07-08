import { ImportService } from './import.service'

describe('ImportService', () => {
  const service = new ImportService(null, null, null, null, null, null) as ImportService

  describe('serializeBFS', () => {
    it('creates records from an unlabeled top-level container object', () => {
      const [records, relations] = service.serializeBFS(
        {
          character: [{ name: 'Leia Organa' }, { name: 'Luke Skywalker' }],
          starship: [{ name: 'Tantive IV' }]
        },
        undefined,
        {
          suggestTypes: true,
          capitalizeLabels: true
        }
      )

      expect(records).toHaveLength(3)
      expect(relations).toHaveLength(0)
      expect(records.map((record) => record.label).sort()).toEqual(['CHARACTER', 'CHARACTER', 'STARSHIP'])
    })
  })

  describe('skipEmptyValues', () => {
    const propsFor = (data: Record<string, unknown>, skipEmptyValues: boolean) => {
      const [records] = service.serializeBFS([data] as any, 'THING', {
        suggestTypes: true,
        skipEmptyValues
      })
      return records[0].properties ?? []
    }

    const sample = {
      name: 'Greedo',
      bio: '',
      tags: [],
      deaths: 0,
      alive: false,
      langs: ['a', '', 'b'],
      empties: ['', '']
    }

    it('stores empty strings and empty arrays as-is by default', () => {
      const props = propsFor(sample, false)
      const byName = Object.fromEntries(props.map((p) => [p.name, p.value]))
      expect(byName.bio).toBe('')
      expect(byName.tags).toEqual([])
      expect(byName.empties).toEqual(['', ''])
    })

    it('treats empty strings and empty arrays as unset when skipEmptyValues is true', () => {
      const props = propsFor(sample, true)
      const names = props.map((p) => p.name).sort()
      // bio (''), tags ([]) and empties (['','']) are dropped; name/deaths/alive/langs survive
      expect(names).toEqual(['alive', 'deaths', 'langs', 'name'])
    })

    it('keeps 0 and false (they are real values, not "empty")', () => {
      const props = propsFor(sample, true)
      const byName = Object.fromEntries(props.map((p) => [p.name, p.value]))
      expect(byName.deaths).toBe(0)
      expect(byName.alive).toBe(false)
    })

    it('strips empty-string elements from arrays, dropping arrays that become empty', () => {
      const props = propsFor(sample, true)
      const byName = Object.fromEntries(props.map((p) => [p.name, p.value]))
      expect(byName.langs).toEqual(['a', 'b'])
      expect(props.find((p) => p.name === 'empties')).toBeUndefined()
    })
  })

  // PapaParse dynamicTyping (the CSV import default when suggestTypes is on) parses
  // ISO-8601 cells into JS Date instances. A Date must never reach the driver as a
  // property value — it serializes as an empty map and Neo4j rejects the whole chunk
  // ("Property values can only be of primitive types... Encountered: Map{}").
  describe('Date instances from CSV dynamicTyping', () => {
    const propsFor = (data: Record<string, unknown>, suggestTypes: boolean) => {
      const [records] = service.serializeBFS([data] as any, 'TRANSACTION', { suggestTypes })
      return Object.fromEntries((records[0].properties ?? []).map((p) => [p.name, p]))
    }

    it('folds a Date value back to its ISO string and types it as datetime', () => {
      const byName = propsFor({ id: 't1', timestamp: new Date('2026-06-02T08:14:00Z') }, true)
      expect(byName.timestamp.value).toBe('2026-06-02T08:14:00.000Z')
      expect(byName.timestamp.type).toBe('datetime')
    })

    it('stores the ISO string (not a locale string) when suggestTypes is off', () => {
      const byName = propsFor({ id: 't1', timestamp: new Date('2026-06-02T08:14:00Z') }, false)
      expect(byName.timestamp.value).toBe('2026-06-02T08:14:00.000Z')
      expect(byName.timestamp.type).toBe('string')
    })

    it('leaves no object-typed property values anywhere in a CSV-shaped row', () => {
      const [records] = service.serializeBFS(
        [
          {
            transaction_id: 't2001',
            sender_account: 'acc_017',
            receiver_account: 'acc_042',
            amount: 9800,
            timestamp: new Date('2026-06-14T09:12:00Z'),
            memo: 'consulting fee'
          }
        ] as any,
        'TRANSACTION',
        { suggestTypes: true }
      )
      for (const property of records[0].properties ?? []) {
        const value = (property as any).value
        const values = Array.isArray(value) ? value : [value]
        for (const item of values) {
          expect(typeof item === 'object' && item !== null).toBe(false)
        }
      }
    })
  })
})
