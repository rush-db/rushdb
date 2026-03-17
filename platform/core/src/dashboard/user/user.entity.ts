import type { UserRow } from '@/database/sql/schema/types'

import { IUserClaims } from './interfaces/user-claims.interface'
import { IUserProperties } from './interfaces/user-properties.interface'
import { TUserStatuses } from './model/user.interface'

export class User {
  constructor(private readonly row: UserRow) {}

  getId(): string {
    return this.row.id
  }

  getPassword(): string {
    return this.row.password
  }

  getGoogleAuth(): string {
    return this.row.googleAuth
  }

  getGithubAuth(): string {
    return this.row.githubAuth
  }

  getStatus(): TUserStatuses {
    return this.row.status as TUserStatuses
  }

  getClaims(): IUserClaims {
    return { id: this.row.id }
  }

  toJson(): IUserProperties {
    const { password, googleAuth, githubAuth, ...rest } = this.row
    return rest as unknown as IUserProperties
  }
}
