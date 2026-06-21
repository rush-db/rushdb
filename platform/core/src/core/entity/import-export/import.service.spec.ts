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
})
