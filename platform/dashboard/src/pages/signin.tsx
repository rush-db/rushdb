import { Button } from '~/elements/Button'
import { Divider } from '~/elements/Divider'
import { TextField } from '~/elements/Input'
import { Link } from '~/elements/Link'
import { GithubButton } from '~/features/auth/components/GitHubButton'
import { GoogleButton } from '~/features/auth/components/GoogleButton'
import { logIn } from '~/features/auth/stores/auth'
import { AuthLayout } from '~/layout/AuthLayout'
import { FetchError } from '~/lib/fetcher'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { LogIn } from 'lucide-react'
import { useStore } from '@nanostores/react'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { useMemo } from 'react'
import { Spinner } from '~/elements/Spinner.tsx'
import { $inviteToken } from '~/features/workspaces/stores/invite.ts'

const schema = object({
  login: string().required(),
  password: string().required().min(8, 'Should be at least 8 characters long').max(32)
}).required()

function SignInForm() {
  const {
    formState: { errors, isSubmitting },
    handleSubmit,
    register,
    setError
  } = useForm({ schema })

  return (
    <>
      <form
        onSubmit={handleSubmit(async (values) => {
          try {
            await logIn(values)
          } catch (error) {
            if (error instanceof FetchError) {
              if (error.response.status === 404) {
                setError('login', {
                  message: 'Not Found'
                })
              }
            }
          }
        })}
        className="flex flex-col gap-3"
      >
        <TextField
          label="Login"
          placeholder="example@example.com"
          {...register('login')}
          autoComplete="email"
          autoFocus
          error={errors?.login?.message as string}
        />
        <TextField
          caption={<Link href={getRoutePath('passwordRecovery')}>Forgot password?</Link>}
          label="Password"
          type="password"
          {...register('password')}
          autoComplete="current-password"
          error={errors?.password?.message as string}
        />

        <Button
          className="mt-2 flex grow items-center justify-center"
          loading={isSubmitting}
          size="large"
          type="submit"
          variant="accent"
        >
          Sign In <LogIn />
        </Button>
      </form>
    </>
  )
}

export function SignInPage() {
  const invite = useStore($inviteToken)
  const platformSettings = useStore($platformSettings)

  const hasGoogleOAuth = useMemo(
    () => platformSettings.data?.googleOAuthEnabled,
    [platformSettings.data?.googleOAuthEnabled]
  )

  const hasGithubOAuth = useMemo(
    () => platformSettings.data?.githubOAuthEnabled,
    [platformSettings.data?.githubOAuthEnabled]
  )

  const hasOauth = useMemo(() => hasGoogleOAuth || hasGithubOAuth, [hasGoogleOAuth, hasGithubOAuth])

  if (platformSettings.loading) {
    return (
      <AuthLayout title={'Sign in to RushDB'} type="signin">
        <div className="m-auto flex content-center items-center justify-between">
          <Spinner />
        </div>
      </AuthLayout>
    )
  }

  return (
    <AuthLayout title={'Sign in to RushDB'} type="signin">
      {hasOauth ?
        <>
          <div className="flex w-full justify-between gap-2">
            {hasGoogleOAuth ?
              <GoogleButton invite={invite ?? undefined} />
            : null}
            {hasGithubOAuth ?
              <GithubButton />
            : null}
          </div>
          <Divider />
        </>
      : null}

      <SignInForm />
      <Divider />
      {invite ? null : (
        <Button as="a" href={getRoutePath('signup')} size="large" variant="ghost">
          Don&apos;t have an account?
        </Button>
      )}
    </AuthLayout>
  )
}
