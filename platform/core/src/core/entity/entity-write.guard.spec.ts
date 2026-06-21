import { BadRequestException, ExecutionContext } from '@nestjs/common'

import { EntityWriteGuard } from './entity-write.guard'

const contextWithBody = (body: unknown) =>
  ({
    switchToHttp: () => ({
      getRequest: () => ({ body })
    })
  }) as ExecutionContext

describe('EntityWriteGuard', () => {
  const guard = new EntityWriteGuard()

  it('rejects __score as a data property key', () => {
    expect(() =>
      guard.canActivate(
        contextWithBody({
          label: 'ARTICLE',
          data: {
            title: 'Vector search',
            __score: 0.99
          }
        })
      )
    ).toThrow(BadRequestException)
  })

  it('rejects __score as a property-draft name', () => {
    expect(() =>
      guard.canActivate(
        contextWithBody({
          label: 'ARTICLE',
          properties: [
            {
              name: '__score',
              type: 'number',
              value: 0.99
            }
          ]
        })
      )
    ).toThrow(BadRequestException)
  })

  it('allows __score as an ordinary property value', () => {
    expect(
      guard.canActivate(
        contextWithBody({
          label: 'ARTICLE',
          data: {
            name: '__score'
          }
        })
      )
    ).toBe(true)
  })
})
