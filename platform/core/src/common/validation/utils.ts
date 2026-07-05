import { ArgumentMetadata } from '@nestjs/common'
import { ZodError, ZodIssue } from 'zod'

const formatIssue = (issue: ZodIssue): string => {
  const label = issue.path.length ? `"${issue.path.join('.')}"` : 'value'

  if (issue.code === 'invalid_union') {
    return `${label} does not match any of the allowed types`
  }

  return `${label} ${issue.message}`
}

export const formatErrorMessage = (error: ZodError, metadata: ArgumentMetadata) => {
  const reasons = error.issues.map(formatIssue).join(', ')

  return (
    `Request validation of ${metadata.type} ` +
    (metadata.data ? `item '${metadata.data}' ` : '') +
    `failed, because: ${reasons}`
  )
}
