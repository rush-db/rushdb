export const USER_STATUS_ACTIVE = 'active' as const
export const USER_STATUS_BLOCKED = 'blocked' as const
export const USER_STATUS_DEACTIVATED = 'deactivated' as const
export const USER_STATUS_DELETED = 'deleted' as const

export const USER_ROLE_OWNER = 'owner' as const
export const USER_ROLE_ADMIN = 'admin' as const
export const USER_ROLE_EDITOR = 'developer' as const
export const USER_ROLE_VIEWER = 'viewer' as const

export const USER_ROLE_WEIGHT = {
  [USER_ROLE_VIEWER]: 0,
  [USER_ROLE_EDITOR]: 1,
  [USER_ROLE_ADMIN]: 2,
  [USER_ROLE_OWNER]: 3
} as const

export const USER_ROLE_LIST = [USER_ROLE_OWNER, USER_ROLE_ADMIN, USER_ROLE_EDITOR, USER_ROLE_VIEWER] as const
export const USER_STATUS_LIST = [
  USER_STATUS_ACTIVE,
  USER_STATUS_BLOCKED,
  USER_STATUS_DEACTIVATED,
  USER_STATUS_DELETED
] as const
