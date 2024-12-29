import { RestApiProxy } from '../api/rest-api-proxy.js'

export class Transaction extends RestApiProxy {
  readonly id: string

  constructor(id: string) {
    super()
    this.id = id
  }

  async rollback() {
    return await this.apiProxy.tx.rollback(this.id)
  }

  async commit() {
    return await this.apiProxy.tx.commit(this.id)
  }
}
