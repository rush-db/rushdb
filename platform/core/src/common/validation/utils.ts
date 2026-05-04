import { ArgumentMetadata } from '@nestjs/common'
// eslint-disable-next-line @typescript-eslint/no-require-imports
import Joi = require('joi')

export const formatErrorMessage = (error: Joi.ValidationError, metadata: ArgumentMetadata) => {
  const reasons = error.details.map((detail: { message: string }) => detail.message).join(', ')

  return (
    `Request validation of ${metadata.type} ` +
    (metadata.data ? `item '${metadata.data}' ` : '') +
    `failed, because: ${reasons}`
  )
}
