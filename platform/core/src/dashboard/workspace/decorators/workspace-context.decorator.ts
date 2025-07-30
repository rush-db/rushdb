import { createParamDecorator, ExecutionContext } from '@nestjs/common'
import { WorkspaceContext, WorkspaceContextStorage } from '@/dashboard/workspace/workspace.context'

export const WorkspaceContextDecorator = createParamDecorator(
  (_: unknown, ctx: ExecutionContext): WorkspaceContext | undefined => {
    return WorkspaceContextStorage.getStore()
  }
)
