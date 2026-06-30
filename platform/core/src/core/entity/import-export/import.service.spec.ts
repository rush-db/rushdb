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
})
