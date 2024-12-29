jest.mock('node-fetch', () => ({
  __esModule: true,
  Response: class Response {
    _data
    ok = true
    constructor(data: any) {
      this._data = data
    }
    json() {
      return Promise.resolve(this._data)
    }
  },
  default: jest.fn()
}))
