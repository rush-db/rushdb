import { Button } from '~/elements/Button'
import { Divider } from '~/elements/Divider'
import { TextField } from '~/elements/Input'
import { Link } from '~/elements/Link'
import { useStore } from '@nanostores/react'
import { GithubButton } from '~/features/auth/components/GitHubButton'
import { GoogleButton } from '~/features/auth/components/GoogleButton'
import { logIn } from '~/features/auth/stores/auth'
import { AuthLayout } from '~/layout/AuthLayout'
import { api } from '~/lib/api'
import { FetchError } from '~/lib/fetcher'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { KeyRound, LogIn } from 'lucide-react'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'
import { useMemo, useState } from 'react'
import { Spinner } from '~/elements/Spinner.tsx'
import { $inviteToken } from '~/features/workspaces/stores/invite.ts'
import type { SubmitHandler } from 'react-hook-form'
import { toast } from '~/elements/Toast.tsx'

interface LoginFormValues {
  login: string
  password: string
}

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
  } = useForm<LoginFormValues>({ schema })

  const loginUser: SubmitHandler<LoginFormValues> = async (values) => {
    try {
      await logIn(values)
    } catch (error) {
      if (error instanceof FetchError) {
        if (error.response.status === 404) {
          setError('login', {
            message: 'Not Found'
          })
        }

        if (error.response.status === 401) {
          setError('login', {
            message: "User's email or password mismatch"
          })
          setError('password', {
            message: "User's email or password mismatch"
          })
          toast({
            title: "User's email or password mismatch"
          })
        }
      }
    }
  }

  return (
    <>
      <form onSubmit={handleSubmit(loginUser)} className="flex flex-col gap-3">
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

function SsoSignIn() {
  const [open, setOpen] = useState(false)
  const [email, setEmail] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string>()

  const handleContinue = async () => {
    setLoading(true)
    setError(undefined)
    try {
      const result = await api.auth.ssoDiscover({ email })
      if (result.ssoAvailable && result.loginUrl) {
        window.location.href = result.loginUrl
      } else {
        setError('No SSO provider is configured for this email domain.')
      }
    } catch {
      setError('Could not start SSO sign-in. Please try again.')
    } finally {
      setLoading(false)
    }
  }

  if (!open) {
    return (
      <Button size="large" variant="outline" onClick={() => setOpen(true)} className="w-full">
        <KeyRound /> Sign in with SSO
      </Button>
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <TextField
        label="Work email"
        placeholder="you@company.com"
        type="email"
        autoFocus
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        error={error}
        onKeyDown={(e) => {
          if (e.key === 'Enter') handleContinue()
        }}
      />
      <Button size="large" variant="accent" loading={loading} disabled={!email} onClick={handleContinue}>
        Continue with SSO
      </Button>
    </div>
  )
}

export function SignInPage() {
  const invite = useStore($inviteToken)
  const { data: platformSettings, isPending } = usePlatformSettings()

  const hasGoogleOAuth = useMemo(
    () => platformSettings?.googleOAuthEnabled,
    [platformSettings?.googleOAuthEnabled]
  )

  const hasGithubOAuth = useMemo(
    () => platformSettings?.githubOAuthEnabled,
    [platformSettings?.githubOAuthEnabled]
  )

  const hasOauth = useMemo(() => hasGoogleOAuth || hasGithubOAuth, [hasGoogleOAuth, hasGithubOAuth])

  if (isPending) {
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

      <SsoSignIn />
      <Divider />

      <Button as="a" href={getRoutePath('signup')} size="large" variant="ghost">
        Don&apos;t have an account?
      </Button>
    </AuthLayout>
  )
}
