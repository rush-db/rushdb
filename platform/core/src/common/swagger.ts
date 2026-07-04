import { RequestMethod } from '@nestjs/common'
import { GlobalPrefixOptions } from '@nestjs/common/interfaces/global-prefix-options.interface'
import { RouteInfo } from '@nestjs/common/interfaces/middleware/middleware-configuration.interface'
import { DocumentBuilder, OpenAPIObject } from '@nestjs/swagger'

export const GLOBAL_PREFIX = 'api/v1'

export const GLOBAL_PREFIX_OPTIONS: GlobalPrefixOptions<string | RouteInfo> = {
  exclude: [
    { path: '/', method: RequestMethod.GET },
    { path: 'health', method: RequestMethod.GET },
    { path: '.well-known/*path', method: RequestMethod.GET },
    { path: 'oauth/*path', method: RequestMethod.GET },
    { path: 'oauth/*path', method: RequestMethod.POST },
    { path: 'oauth/*path', method: RequestMethod.DELETE },
    { path: 'oauth/*path', method: RequestMethod.PATCH }
  ]
}

export const buildSwaggerConfig = (): Omit<OpenAPIObject, 'paths'> =>
  new DocumentBuilder()
    .setTitle('RushDB API')
    .setDescription('RushDB API specs')
    .setVersion('1.0')
    .addBearerAuth()
    .build()
