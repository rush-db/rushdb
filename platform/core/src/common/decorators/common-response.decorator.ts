import { applyDecorators, Type } from '@nestjs/common'
import { ApiExtraModels, ApiOkResponse, getSchemaPath } from '@nestjs/swagger'

export const CommonResponseDecorator = <TModel extends Type<any>>(model?: TModel) => {
  const apiResponse = ApiOkResponse({
    status: 200,
    description: '',
    content: {
      'application-json': {
        schema: {
          properties: {
            ...(model && {
              data: {
                $ref: getSchemaPath(model)
              }
            }),
            ...(!model && {
              data: {
                type: 'boolean'
              }
            }),
            success: {
              type: 'boolean'
            }
          }
        }
      }
    }
  })

  return model ? applyDecorators(ApiExtraModels(model), apiResponse) : applyDecorators(apiResponse)
}
