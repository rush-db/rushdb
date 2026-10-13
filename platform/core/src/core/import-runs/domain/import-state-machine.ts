import { TERMINAL_FILE_STATUSES, type ImportFileStatus, type ImportRunStatus } from './import-run.types'

interface FileShape {
  status: ImportFileStatus
  role: 'records' | 'links'
}

const ACTIVE_RUN_STATUSES: ImportRunStatus[] = ['uploading', 'queued', 'running', 'canceling']

export const FILE_TRANSITIONS: Record<ImportFileStatus, ImportFileStatus[]> = {
  awaiting_upload: ['uploading', 'uploaded', 'queued', 'canceled'],
  uploading: ['uploaded', 'queued', 'retry_wait', 'failed', 'canceled'],
  uploaded: ['queued', 'failed', 'canceled'],
  queued: ['validating', 'running', 'retry_wait', 'blocked', 'canceled'],
  validating: ['running', 'retry_wait', 'failed', 'canceled'],
  running: ['finalizing', 'completed', 'retry_wait', 'blocked', 'failed', 'canceled'],
  retry_wait: ['queued', 'blocked', 'failed', 'canceled'],
  blocked: ['queued', 'canceled'],
  finalizing: ['completed', 'retry_wait', 'failed', 'canceled'],
  completed: [],
  failed: [],
  canceled: [],
  source_expired: []
}

export function canTransitionFile(from: ImportFileStatus, to: ImportFileStatus): boolean {
  return FILE_TRANSITIONS[from]?.includes(to) ?? false
}

export function isTerminalFile(status: ImportFileStatus): boolean {
  return TERMINAL_FILE_STATUSES.includes(status)
}

/**
 * Pure run-status derivation. Order matters:
 * canceling > blocked > uploading > queued/running > finalizing > terminal outcomes.
 */
export function deriveRunStatus(
  files: FileShape[],
  run?: { cancelRequestedAt?: string | null }
): ImportRunStatus {
  if (files.length === 0) {
    return 'draft'
  }

  const statuses = files.map((f) => f.status)
  const allTerminal = statuses.every((s) => isTerminalFile(s))
  const anyBlocked = statuses.includes('blocked')
  const anyUploading = statuses.includes('awaiting_upload') || statuses.includes('uploading')
  const anyQueuedOrRunning =
    statuses.some((s) => ['queued', 'validating', 'running', 'retry_wait', 'finalizing'].includes(s)) ||
    (anyBlocked && !allTerminal)
  const anyCompleted = statuses.includes('completed')
  const allCanceled = statuses.every((s) => s === 'canceled')
  const anyFailed = statuses.includes('failed')

  if (
    !allTerminal &&
    run?.cancelRequestedAt &&
    statuses.some((s) => ACTIVE_RUN_STATUSES.includes(s as never))
  ) {
    return 'canceling'
  }

  if (allTerminal) {
    if (statuses.includes('finalizing')) {
      // finalizing is modeled as a nonterminal file state; treat as pending side effects
      return 'finalizing'
    }
    if (allCanceled) {
      return 'canceled'
    }
    if (anyFailed && !anyCompleted) {
      return 'failed'
    }
    if (anyFailed || anyCompleted !== statuses.every((s) => s === 'completed')) {
      return 'completed_with_errors'
    }
    return 'completed'
  }

  if (anyBlocked && !statuses.some((s) => ['queued', 'validating', 'running', 'retry_wait'].includes(s))) {
    return 'blocked'
  }

  if (run?.cancelRequestedAt) {
    return 'canceling'
  }

  if (anyUploading) {
    return 'uploading'
  }

  if (anyQueuedOrRunning) {
    return 'running'
  }

  return 'running'
}

export function canTransitionRun(from: ImportRunStatus, to: ImportRunStatus): boolean {
  const allowed: Record<ImportRunStatus, ImportRunStatus[]> = {
    draft: ['uploading', 'queued', 'canceled'],
    uploading: ['queued', 'running', 'blocked', 'canceling', 'canceled', 'failed'],
    queued: ['running', 'blocked', 'canceling', 'canceled', 'failed'],
    running: [
      'blocked',
      'canceling',
      'finalizing',
      'completed',
      'completed_with_errors',
      'failed',
      'canceled'
    ],
    blocked: ['queued', 'running', 'canceling', 'canceled'],
    canceling: ['finalizing', 'completed', 'completed_with_errors', 'failed', 'canceled'],
    finalizing: ['completed', 'completed_with_errors', 'failed', 'canceled'],
    completed: [],
    completed_with_errors: [],
    failed: [],
    canceled: []
  }
  return allowed[from]?.includes(to) ?? false
}
