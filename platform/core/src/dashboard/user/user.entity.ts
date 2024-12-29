import { IUserClaims } from './interfaces/user-claims.interface'
import { IUserProperties } from './interfaces/user-properties.interface'
import { TUserInstance, TUserStatuses } from './model/user.interface'

export class User {
  constructor(private readonly node: TUserInstance) {}

  getId(): string {
    return this.node.dataValues.id
  }

  getPassword(): string {
    return this.node.dataValues.password
  }

  getGoogleAuth(): string {
    return this.node.dataValues.googleAuth
  }

  getGithubAuth(): string {
    return this.node.dataValues.githubAuth
  }

  getStatus(): TUserStatuses {
    return this.node.dataValues.status
  }

  getClaims(): IUserClaims {
    const { login, id, confirmed, firstName, lastName, ...properties } = this.node.dataValues

    return { login, id, confirmed, firstName, lastName }
  }

  toJson(): IUserProperties {
    const { password, googleAuth, githubAuth, ...properties } = this.node.dataValues

    return properties as unknown as IUserProperties
  }
}
