import { BadRequestException } from '@nestjs/common'

export class QueryCriteriaParsingError extends BadRequestException {
  constructor(operator: string, value: unknown) {
    super(
      `Value ${JSON.stringify(value)} of type ${typeof value} couldn't be used with '${operator}' operator.`
    )
  }
}
