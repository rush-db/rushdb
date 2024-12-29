export interface IOauthUrl {
  url: string
}

export interface IDecodedResetToken {
  login: string
  id: string
}

export type TVerifyOwnershipConfig = {
  nodeProperty: string
  projectIdProperty: string
}
