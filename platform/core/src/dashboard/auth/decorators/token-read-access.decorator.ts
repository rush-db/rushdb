import { SetMetadata } from '@nestjs/common'

export const TOKEN_READ_SAFE_KEY = 'tokenReadSafe'

// Marks a route as safe for read-only API tokens. Routes without this
// decorator reject read-only tokens (default-deny) in GlobalAuthGuard.
export const TokenReadAccess = () => SetMetadata(TOKEN_READ_SAFE_KEY, true)
