import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Put,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController, ApiParam, ApiTags } from '@nestjs/swagger'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { UpsertSsoConfigDto } from '@/dashboard/auth/providers/sso/dto/upsert-sso-config.dto'
import { SsoService } from '@/dashboard/auth/providers/sso/sso.service'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'

@Controller('workspaces')
@ApiExcludeController()
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor, ChangeCorsInterceptor)
export class SsoAdminController {
  constructor(private readonly ssoService: SsoService) {}

  @Get(':id/sso')
  @ApiParam({ name: 'id', required: true, description: 'workspace identifier (UUIDv7)', type: 'string' })
  @ApiTags('SSO')
  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  async list(@Param('id') id: string) {
    return this.ssoService.listConfigs(id)
  }

  @Put(':id/sso')
  @ApiParam({ name: 'id', required: true, description: 'workspace identifier (UUIDv7)', type: 'string' })
  @ApiTags('SSO')
  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  @HttpCode(HttpStatus.OK)
  async upsert(@Param('id') id: string, @Body() dto: UpsertSsoConfigDto) {
    return this.ssoService.upsertConfig(id, dto)
  }

  @Delete(':id/sso/:configId')
  @ApiParam({ name: 'id', required: true, description: 'workspace identifier (UUIDv7)', type: 'string' })
  @ApiParam({ name: 'configId', required: true, description: 'SSO config identifier', type: 'string' })
  @ApiTags('SSO')
  @ApiBearerAuth()
  @AuthGuard('workspace', 'owner')
  @HttpCode(HttpStatus.OK)
  async remove(@Param('id') id: string, @Param('configId') configId: string) {
    return this.ssoService.deleteConfig(id, configId)
  }
}
