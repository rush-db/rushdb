import { Button } from '~/elements/Button'
import { Divider } from '~/elements/Divider'
import { TextField } from '~/elements/Input'
import { GithubButton } from '~/features/auth/components/GitHubButton'
import { GoogleButton } from '~/features/auth/components/GoogleButton'
import { createUser } from '~/features/auth/stores/auth'
import { AuthLayout } from '~/layout/AuthLayout'
import { object, string, useForm } from '~/lib/form'
import { getRoutePath } from '~/lib/router'
import { useStore } from '@nanostores/react'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { useMemo } from 'react'
import { Spinner } from '~/elements/Spinner.tsx'

export const schema = object({
  login: string().required(),
  password: string().required().min(8, 'Should be at least 8 characters long').max(32),
  passwordConfirmation: string().test('passwords-match', 'Passwords must match', function (value?: string) {
    return this.parent.password === value
  })
}).required()

function SignUpForm() {
  const {
    formState: { errors, isSubmitting },
    handleSubmit,
    register
  } = useForm({ schema })

  return (
    <>
      <form className="flex flex-col gap-5" onSubmit={handleSubmit(createUser)}>
        <TextField
          label="Login"
          placeholder="example@example.com"
          {...register('login')}
          autoFocus
          error={errors?.login?.message as string}
        />
        <TextField
          autoComplete="new-password"
          label="Password"
          placeholder="8-24 Characters"
          type="password"
          {...register('password')}
          error={errors?.password?.message as string}
        />
        <TextField
          autoComplete="new-password"
          label="Confirm Password"
          type="password"
          {...register('passwordConfirmation')}
          error={errors?.passwordConfirmation?.message as string}
        />

        <Button className="mt-2" loading={isSubmitting} size="large" type="submit" variant="secondary">
          Continue With Email
        </Button>
      </form>
    </>
  )
}

export function SignUpPage() {
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
    <AuthLayout title={'Create new RushDB account'} type="signup">
      {hasOauth ?
        <>
          <div className="flex w-full justify-between gap-2">
            {hasGoogleOAuth ?
              <GoogleButton />
            : null}
            {hasGithubOAuth ?
              <GithubButton />
            : null}
          </div>
          <Divider />
        </>
      : null}

      <SignUpForm />

      <Divider />

      <Button as="a" href={getRoutePath('signin')} size="large" variant="ghost">
        Already have an account?
      </Button>
    </AuthLayout>
  )
}
