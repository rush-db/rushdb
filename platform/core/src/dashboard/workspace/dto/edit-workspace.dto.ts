import { ApiPropertyOptional } from '@nestjs/swagger'

export class EditWorkspaceDto {
  @ApiPropertyOptional()
  name?: string
}
