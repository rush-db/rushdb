export class QueryBuilder {
  queryParts: string[] = []

  append(part: string) {
    if (part) {
      this.queryParts.push(part)
    }

    return this // Allow method chaining
  }

  build(separator = '\n') {
    return this.queryParts.join(separator)
  }

  getQuery() {
    return this.build()
  }
}
