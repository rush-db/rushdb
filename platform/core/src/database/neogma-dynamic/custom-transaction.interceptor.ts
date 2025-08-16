import { Injectable, NestInterceptor, ExecutionContext, CallHandler, Logger } from '@nestjs/common'
import { concatMap, Observable } from 'rxjs'
import { catchError } from 'rxjs/operators'

import { isDevMode } from '@/common/utils/isDevMode'
import { ProjectService } from '@/dashboard/project/project.service'
import { TProjectCustomDbPayload } from '@/dashboard/project/project.types'
import { NeogmaDynamicService } from '@/database/neogma-dynamic/neogma-dynamic.service'

// @Injectable()
// export class CustomTransactionInterceptor implements NestInterceptor {
//   constructor(
//     private readonly dynamicService: NeogmaDynamicService,
//     private readonly projectService: ProjectService
//   ) {}
//
//   async intercept(context: ExecutionContext, next: CallHandler): Promise<Observable<any>> {
//     const request = context.switchToHttp().getRequest()
//     const projectId = request.projectId ?? request.headers['x-project-id']
//     const defaultTx = request.transaction
//
//     if (projectId && defaultTx) {
//       try {
//         const projectNode = await this.projectService.getProject(projectId, defaultTx)
//         const project = projectNode.toJson()
//
//         if (project?.customDb) {
//           const customDbPayload = this.projectService.decryptSensitiveData<TProjectCustomDbPayload>(
//             project.customDb
//           )
//           const config = {
//             url: customDbPayload.url,
//             username: customDbPayload.username,
//             password: customDbPayload.password
//           }
//
//           const connection = await this.dynamicService.getConnection(projectId, config)
//           const session = connection.driver.session()
//           const customTransaction = session.beginTransaction()
//
//           request.customTransaction = customTransaction
//           request.customSession = session
//
//           isDevMode(() => Logger.debug(`Custom transaction created for project ${projectId}`))
//         }
//       } catch (error) {
//         isDevMode(() =>
//           Logger.error(`CustomTransactionInterceptor error for project ${projectId}: ${error.message}`)
//         )
//       }
//     }
//
//     return next.handle().pipe(
//       concatMap(async (data) => {
//         if (request.customTransaction && request.customTransaction.isOpen()) {
//           await request.customTransaction.commit()
//
//           if (request.customSession) {
//             await request.customSession.close(request.customSession)
//           }
//
//           isDevMode(() => Logger.debug('[COMMIT CUSTOM TRANSACTION]'))
//         }
//         return data
//       }),
//       catchError(async (error) => {
//         if (request.customTransaction && request.customTransaction.isOpen()) {
//           await request.customTransaction.rollback()
//
//           if (request.customSession) {
//             await request.customSession.close(request.customSession)
//           }
//
//           isDevMode(() => Logger.debug('[ROLLBACK CUSTOM TRANSACTION]'))
//         }
//         throw error
//       })
//     )
//   }
// }
