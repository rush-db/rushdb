import {
  ArgumentsHost,
  Catch,
  ExceptionFilter,
  HttpException,
  HttpStatus,
  Injectable,
  Logger
} from '@nestjs/common'
import { FastifyReply, FastifyRequest } from 'fastify'
import { Session, Transaction } from 'neo4j-driver'

import { isDevMode } from '@/common/utils/isDevMode'
import { NeogmaService } from '@/database/neogma/neogma.service'

@Injectable()
@Catch()
export class GlobalExceptionFilter implements ExceptionFilter {
  constructor(private readonly neogmaService: NeogmaService) {}

  async catch(exception: HttpException, host: ArgumentsHost) {
    Logger.error('Global Exception Filter', exception)
    const ctx = host.switchToHttp()
    const response = ctx.getResponse<FastifyReply>()
    const request = ctx.getRequest<FastifyRequest>()
    const isHttpException = exception instanceof HttpException

    const status = isHttpException ? exception.getStatus() : HttpStatus.INTERNAL_SERVER_ERROR

    const { transaction, session, customTransaction } = request as unknown as {
      transaction: Transaction
      customTransaction: Transaction
      session: Session
    }
    isDevMode(() => {
      Logger.log('[ROLLBACK TRANSACTION]: Exception filter', exception)
    })

    await transaction?.close()
    await customTransaction?.close()
    await this.neogmaService.closeSession(session, 'global-exception-filter')

    response.status(status).send(isHttpException ? exception.getResponse() : {})
  }
}
