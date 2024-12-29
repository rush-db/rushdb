import { useStore } from '@nanostores/react'
import { Check } from 'lucide-react'

import { Banner } from '~/elements/Banner'
import { Button } from '~/elements/Button'
import { Divider } from '~/elements/Divider'
import { TextField } from '~/elements/Input'
import { $sendRecoveryLink, resetPassword } from '~/features/auth/stores/auth'
import { AuthLayout } from '~/layout/AuthLayout'
import { object, string, useForm } from '~/lib/form'
import {
  $router,
  $searchParams,
  getRoutePath,
  redirectRoute
} from '~/lib/router'

import { schema as signUpSchema } from './signup'

const schema = object({ email: string().email().required() })

function SendPasswordForm() {
  const {
    formState: { errors, isSubmitting },
    handleSubmit,
    register,
    setError
  } = useForm({ schema })

  const { data, mutate } = useStore($sendRecoveryLink)

  const onSubmit = async ({ email }: { email?: string }) => {
    try {
      await mutate({ email })
    } catch (error) {
      setError('email', { message: "Couldn't send a link to that address" })
    }
  }

  if (data) {
    return (
      <Banner
        image={<Check className="h-24 w-24 text-success" />}
        title="We've sent you a message with a recovery link"
      />
    )
  }

  return (
    <form className="flex flex-col gap-5" onSubmit={handleSubmit(onSubmit)}>
      <TextField
        caption="We'll send a recovery link to this e-mail"
        label="Email"
        {...register('email')}
        error={errors?.email?.message as string}
      />

      <Button
        className="mt-2"
        loading={isSubmitting}
        size="large"
        type="submit"
        variant="accent"
      >
        Confirm
      </Button>
    </form>
  )
}

function ChangePasswordForm({ token }: { token: string }) {
  const {
    formState: { errors, isSubmitting },
    handleSubmit,
    register,
    setError
  } = useForm({
    defaultValues: {
      email: '',
      password: '',
      passwordConfirmation: ''
    },
    schema: signUpSchema
  })

  const onSubmit = async ({
    email,
    password
  }: {
    email: string
    password: string
  }) => {
    try {
      await resetPassword({
        email,
        userConfirmationToken: token,
        userNewPassword: password
      })
    } catch (error) {
      setError('email', { message: "Couldn't send a link to that address" })
    }
  }

  return (
    <form className="flex flex-col gap-5" onSubmit={handleSubmit(onSubmit)}>
      <TextField
        label="Email"
        {...register('email')}
        error={errors?.email?.message}
        placeholder="example@example.com"
      />

      <TextField
        label="New password"
        {...register('password')}
        error={errors?.password?.message}
        type="password"
      />

      <TextField
        label="Confirm new password"
        {...register('passwordConfirmation')}
        error={errors?.passwordConfirmation?.message}
        type="password"
      />

      <Button
        className="mt-2"
        loading={isSubmitting}
        size="large"
        type="submit"
        variant="accent"
      >
        Confirm
      </Button>
    </form>
  )
}

export function PasswordRecoveryPage() {
  const page = useStore($router)
  const searchParams = useStore($searchParams)

  if (page?.route !== 'passwordRecovery') {
    redirectRoute('signup')
    return null
  }

  const { token } = searchParams

  return (
    <AuthLayout title={'Recover your RushDB account'}>
      {token ? <ChangePasswordForm token={token} /> : <SendPasswordForm />}

      <Divider />

      <Button as="a" href={getRoutePath('signin')} size="large" variant="ghost">
        Back to Sign In
      </Button>
    </AuthLayout>
  )
}
