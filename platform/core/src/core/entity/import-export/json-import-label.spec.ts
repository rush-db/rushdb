import { jsonImportRequiresLabel } from './json-import-label'

describe('jsonImportRequiresLabel', () => {
  it('requires a label for a top-level array', () => {
    expect(jsonImportRequiresLabel([{ name: 'Leia' }])).toBe(true)
  })

  it('requires a label for an object record with primitive top-level properties', () => {
    expect(
      jsonImportRequiresLabel({
        name: 'Leia',
        starship: [{ name: 'Tantive IV' }]
      })
    ).toBe(true)
  })

  it('requires a label for primitive arrays because they are record properties', () => {
    expect(
      jsonImportRequiresLabel({
        tags: ['jedi', 'senator']
      })
    ).toBe(true)
  })

  it('does not require a label for a single complex top-level collection', () => {
    expect(
      jsonImportRequiresLabel({
        users: [{ name: 'Leia' }]
      })
    ).toBe(false)
  })

  it('does not require a label when every top-level key contains complex data', () => {
    expect(
      jsonImportRequiresLabel({
        battle: [{ name: 'Battle of Yavin' }],
        planet: [{ name: 'Alderaan' }],
        starship: [{ name: 'Tantive IV' }],
        faction: [{ name: 'Rebel Alliance' }],
        character: [{ name: 'Leia Organa' }]
      })
    ).toBe(false)
  })

  it('requires a label for an empty object', () => {
    expect(jsonImportRequiresLabel({})).toBe(true)
  })
})
