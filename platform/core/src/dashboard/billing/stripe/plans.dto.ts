import { ApiProperty } from '@nestjs/swagger'
import { ApiModelProperty } from '@nestjs/swagger/dist/decorators/api-model-property.decorator'
import { IsNotEmpty } from 'class-validator'
import { PlanName, PlanPeriod } from '@/dashboard/billing/stripe/interfaces/stripe.types'

export class PlansDto {
  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'start' })
  id: PlanName

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'month' })
  period: PlanPeriod

  @IsNotEmpty()
  @ApiProperty()
  @ApiModelProperty({ example: 'http://localhost:3005/' })
  returnUrl: string
}
