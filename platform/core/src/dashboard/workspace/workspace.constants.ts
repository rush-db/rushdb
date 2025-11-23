import { TWorkspaceLimits } from '@/dashboard/workspace/model/workspace.interface'

export const WORKSPACE_LIMITS_SELF_HOSTED: TWorkspaceLimits = {
  records: null,
  projects: null,
  importSize: null,
  users: null
}

export const WORKSPACE_LIMITS_START: TWorkspaceLimits = {
  records: 30_000,
  projects: 2,
  importSize: 5 * 1024 * 1024, // 5 MB
  users: 1
}

export const WORKSPACE_LIMITS_PRO: TWorkspaceLimits = {
  records: 200_000,
  projects: null, // UNLIMITED
  importSize: 32 * 1024 * 1024, // 32 MB
  users: 3
}

export const WORKSPACE_LIMITS_TEAM: TWorkspaceLimits = {
  records: null, // UNLIMITED
  projects: null, // UNLIMITED
  importSize: 32 * 1024 * 1024, // 32 MB
  users: 10
}
