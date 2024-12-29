import { processCriteria } from '@/core/search/parser/processCriteria'

describe('processCriteria', () => {
  it('converts correctly 1', () => {
    const input = {
      where: {
        $or: {
          CAR: {
            color: 'red',
            $xor: {
              SPOUSE: {
                gender: 'male'
              },
              title: {
                $ne: 'Forest'
              }
            }
          },
          JOB: {
            title: 'Manager'
          }
        }
      }
    }

    const result1 = processCriteria(input)

    const output = {
      where: {
        $or: [
          {
            CAR: {
              color: 'red',
              $xor: [
                {
                  SPOUSE: {
                    gender: 'male'
                  }
                },
                {
                  title: {
                    $ne: 'Forest'
                  }
                }
              ]
            }
          },
          {
            JOB: {
              title: 'Manager'
            }
          }
        ]
      }
    }

    expect(result1).toEqual(output)
  })

  it('converts correctly 2', () => {
    const input = {
      where: {
        created: true,
        rating: 5,

        $or: {
          CAR: {
            color: 'red'
          },

          title: {
            $ne: 'Forest'
          },

          SPOUSE: {
            gender: 'male'
          }
        }
      }
    }

    const result2 = processCriteria(input)

    const output = {
      where: {
        created: true,
        rating: 5,

        $or: [
          {
            CAR: {
              color: 'red'
            }
          },
          {
            title: {
              $ne: 'Forest'
            }
          },
          {
            SPOUSE: {
              gender: 'male'
            }
          }
        ]
      }
    }

    expect(result2).toEqual(output)
  })

  it('converts correctly 3', () => {
    const input = {
      where: {
        $xor: {
          foundingDate: { $gte: '1989-10-01T19:05:17.780Z' },
          city: 'Doral'
        }
      }
    }

    const result2 = processCriteria(input)

    const output = {
      where: {
        $xor: [{ foundingDate: { $gte: '1989-10-01T19:05:17.780Z' } }, { city: 'Doral' }]
      }
    }

    expect(result2).toEqual(output)
  })

  it('converts correctly 4', () => {
    const input = {
      where: {
        $xor: [{ foundingDate: { $gte: '1989-10-01T19:05:17.780Z' } }, { city: 'Doral' }]
      }
    }

    const result2 = processCriteria(input)

    const output = {
      where: {
        $xor: [{ foundingDate: { $gte: '1989-10-01T19:05:17.780Z' } }, { city: 'Doral' }]
      }
    }

    expect(result2).toEqual(output)
  })
})
