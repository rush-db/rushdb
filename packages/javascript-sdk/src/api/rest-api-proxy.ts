import type { RestAPI } from './api.js'

export class RestApiProxy {
  protected apiProxy: RestAPI = {} as RestAPI

  constructor() {
    return new Proxy(this, {
      get: (target, prop, receiver) => {
        if (prop in target) {
          return Reflect.get(target, prop, receiver)
        }
        if (this.apiProxy && prop in this.apiProxy) {
          return Reflect.get(this.apiProxy, prop, receiver)
        }
      }
    })
  }

  // @TODO: Probably we want add allowList for some methods to be exposed explicitly (or blockList?)
  init(api: RestAPI) {
    this.apiProxy = api
  }
}
