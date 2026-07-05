import { ArgumentMetadata, BadRequestException, mixin } from '@nestjs/common'
import { Injectable, PipeTransform } from '@nestjs/common'
import { ZodTypeAny } from 'zod'

import { formatErrorMessage } from '@/common/validation/utils'

export const ValidationPipe = <T = any, R = any>(
  schema: ZodTypeAny,
  type: ArgumentMetadata['type'],
  paramName?: string
) => {
  @Injectable()
  class DefaultValidationPipe implements PipeTransform {
    transform(value: T, metadata: ArgumentMetadata) {
      const typeMatch = type === metadata.type
      if (typeMatch) {
        if (typeof paramName !== 'undefined' && paramName === metadata.data) {
          return this.validate<T>(value, schema, metadata)
        } else if (typeof paramName === 'undefined') {
          return this.validate<T>(value, schema, metadata)
        }
      }
      return value as T
    }

    validate<T>(payload: unknown, schema: ZodTypeAny, metadata: ArgumentMetadata = { type: 'custom' }): T {
      const result = schema.safeParse(payload)

      if (!result.success) {
        throw new BadRequestException(formatErrorMessage(result.error, metadata))
      }

      return result.data as T
    }
  }

  return mixin(DefaultValidationPipe)
}
