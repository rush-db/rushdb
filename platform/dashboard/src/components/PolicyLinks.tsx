import { Link } from '~/elements/Link'

export function PolicyLinks() {
  return (
    <div className="space-y-2 text-center text-sm">
      <div className="flex flex-wrap justify-center gap-4">
        <Link href="https://rushdb.com/privacy-policy" target="_blank" size="small" variant="text">
          Privacy Policy
        </Link>
        <Link href="https://rushdb.com/terms-of-service" target="_blank" size="small" variant="text">
          Terms of Service
        </Link>
        <Link href="https://rushdb.com/cookie-policy" target="_blank" size="small" variant="text">
          Cookie Policy
        </Link>
      </div>
    </div>
  )
}
