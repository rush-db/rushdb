import { ArgumentMetadata, BadRequestException, mixin } from '@nestjs/common'
import { Injectable, PipeTransform } from '@nestjs/common'
import { AnySchema } from 'joi'
import * as Joi from 'joi'

import { formatErrorMessage } from '@/common/validation/utils'

const DEFAULT_JOI_OPTS: Joi.ValidationOptions = {
  abortEarly: false,
  allowUnknown: true,
  convert: false
}

export const ValidationPipe = <T = any, R = any>(
  schema: AnySchema,
  type: ArgumentMetadata['type'],
  paramName?: string
) => {
  @Injectable()
  class DefaultValidationPipe implements PipeTransform {
    transform(value: T, metadata: ArgumentMetadata) {
      const typeMatch = type === metadata.type
      if (typeMatch) {
        if (typeof paramName !== 'undefined' && paramName === metadata.data) {
          return this.validate<T>(value, schema, DEFAULT_JOI_OPTS, metadata)
        } else if (typeof paramName === 'undefined') {
          return this.validate<T>(value, schema, DEFAULT_JOI_OPTS, metadata)
        }
      }
      return value as T
    }

    validate<T>(
      payload: unknown,
      schema: Joi.Schema,
      validationOptions: Joi.ValidationOptions,
      metadata: ArgumentMetadata = { type: 'custom' }
    ): T {
      const { error, value } = schema.validate(payload, validationOptions)

      if (error) {
        if (Joi.isError(error)) {
          throw new BadRequestException(formatErrorMessage(error, metadata))
        } else {
          throw error
        }
      }

      return value as T
    }
  }

  return mixin(DefaultValidationPipe)
}
