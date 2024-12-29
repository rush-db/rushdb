import { Button } from '~/elements/Button'
import { AuthLayout } from '~/layout/AuthLayout'
import { getRoutePath } from '~/lib/router'

export function NotFoundPage() {
  return (
    <AuthLayout>
      <div className="grid h-full place-content-center place-items-center gap-2">
        <h1 className="text-3xl font-bold">404</h1>

        <Button
          as="a"
          href={getRoutePath('home')}
          size="large"
          variant="primary"
        >
          Return to Sign In page
        </Button>
      </div>
    </AuthLayout>
  )
}
