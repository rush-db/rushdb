import {
  Body,
  Controller,
  Delete,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Patch,
  Post,
  Request,
  UseInterceptors
} from '@nestjs/common'
import { ApiBearerAuth, ApiExcludeController, ApiTags } from '@nestjs/swagger'

import { NotFoundInterceptor } from '@/common/interceptors/not-found.interceptor'
import { TransformResponseInterceptor } from '@/common/interceptors/transform-response.interceptor'
import { PlatformRequest } from '@/common/types/request'
import { AuthGuard } from '@/dashboard/auth/guards/global-auth.guard'
import { ChangeCorsInterceptor } from '@/dashboard/common/interceptors/change-cors.interceptor'
import { CreateSavedQueryDto, UpdateSavedQueryDto } from '@/dashboard/saved-query/dto/create-saved-query.dto'
import { SavedQueryService } from '@/dashboard/saved-query/saved-query.service'
import { AuthUser } from '@/dashboard/user/decorators/user.decorator'
import { IUserClaims } from '@/dashboard/user/interfaces/user-claims.interface'
import { DataInterceptor } from '@/database/interceptors/data.interceptor'

@Controller()
@ApiExcludeController()
@UseInterceptors(TransformResponseInterceptor, NotFoundInterceptor, DataInterceptor, ChangeCorsInterceptor)
export class SavedQueryController {
  constructor(private readonly savedQueryService: SavedQueryService) {}

  @Post('saved-queries')
  @ApiTags('Saved Queries')
  @ApiBearerAuth()
  @AuthGuard('project')
  @HttpCode(HttpStatus.CREATED)
  create(
    @Body() dto: CreateSavedQueryDto,
    @Request() request: PlatformRequest,
    @AuthUser() user: IUserClaims
  ) {
    return this.savedQueryService.create(dto, request.projectId, user?.id)
  }

  @Get('saved-queries')
  @ApiTags('Saved Queries')
  @ApiBearerAuth()
  @AuthGuard('project')
  list(@Request() request: PlatformRequest) {
    return this.savedQueryService.list(request.projectId)
  }

  @Get('saved-queries/:savedQueryId')
  @ApiTags('Saved Queries')
  @ApiBearerAuth()
  @AuthGuard('project')
  get(@Param('savedQueryId') savedQueryId: string, @Request() request: PlatformRequest) {
    return this.savedQueryService.get(savedQueryId, request.projectId)
  }

  @Patch('saved-queries/:savedQueryId')
  @ApiTags('Saved Queries')
  @ApiBearerAuth()
  @AuthGuard('project')
  update(
    @Param('savedQueryId') savedQueryId: string,
    @Body() dto: UpdateSavedQueryDto,
    @Request() request: PlatformRequest
  ) {
    return this.savedQueryService.update(savedQueryId, request.projectId, dto)
  }

  @Delete('saved-queries/:savedQueryId')
  @ApiTags('Saved Queries')
  @ApiBearerAuth()
  @AuthGuard('project')
  delete(@Param('savedQueryId') savedQueryId: string, @Request() request: PlatformRequest) {
    return this.savedQueryService.delete(savedQueryId, request.projectId)
  }
}
