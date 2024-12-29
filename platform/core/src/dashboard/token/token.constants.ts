export const WRITE_ACCESS = 'write' as const
export const READ_ACCESS = 'read' as const

export const ACCESS_WEIGHT = {
  [WRITE_ACCESS]: 1,
  [READ_ACCESS]: 0
}
