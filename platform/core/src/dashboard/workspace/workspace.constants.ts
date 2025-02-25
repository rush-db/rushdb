import { TWorkspaceLimits } from '@/dashboard/workspace/model/workspace.interface'

export const WORKSPACE_LIMITS_DEFAULT: TWorkspaceLimits = {
  records: 10_000,
  projects: 2,
  importSize: 5 * 1024 * 1024 // 5 MB
}

export const WORKSPACE_LIMITS_PRO: TWorkspaceLimits = {
  records: 1_000_000,
  projects: null, // UNLIMITED
  importSize: 32 * 1024 * 1024 // 32 MB
}

export const WORKSPACE_LIMITS_START: TWorkspaceLimits = {
  records: 100_000,
  projects: null, // UNLIMITED
  importSize: 32 * 1024 * 1024 // 32 MB
}

export const WORKSPACE_LIMITS_WHITE_LABEL: TWorkspaceLimits = {
  records: null,
  projects: null,
  importSize: null
}
