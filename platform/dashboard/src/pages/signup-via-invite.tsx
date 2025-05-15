import { useStore } from '@nanostores/react'
import { useEffect, useMemo, useState } from 'react'
import { useForm, object, string } from '~/lib/form'
import { AuthLayout } from '~/layout/AuthLayout'
import { TextField } from '~/elements/Input'
import { Button } from '~/elements/Button'
import { Dialog, DialogTitle, DialogFooter } from '~/elements/Dialog'
import { AlertTriangle } from 'lucide-react'
import { createUserViaInvite } from '~/features/auth/stores/auth'
import { $searchParams, redirectRoute, getRoutePath } from '~/lib/router'
import { DialogContent } from '@radix-ui/react-dialog'
import { $platformSettings } from '~/features/auth/stores/settings.ts'
import { GoogleButton } from '~/features/auth/components/GoogleButton.tsx'
import { Divider } from '~/elements/Divider.tsx'

const schema = object({
  login: string().required(),
  password: string().required().min(8, 'Should be at least 8 characters long').max(32),
  passwordConfirmation: string().test('pwd-match', 'Passwords must match', function (val?: string) {
    return this.parent.password === val
  })
})

export default function SignupViaInvitePage() {
  const platformSettings = useStore($platformSettings)
  const search = useStore($searchParams)
  const invite = search.invite
  const email = search.email
  const [dialogOpen, setDialogOpen] = useState(false)

  const hasGoogleOAuth = useMemo(
    () => platformSettings.data?.googleOAuthEnabled,
    [platformSettings.data?.googleOAuthEnabled]
  )

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting }
  } = useForm<{ login: string; password: string; passwordConfirmation: string }>({
    schema,
    defaultValues: { login: email ?? '' }
  })

  useEffect(() => {
    if (!invite || !email) {
      setDialogOpen(true)
    }
  }, [])

  const onSubmit = async ({ password }: { password: string }) => {
    await createUserViaInvite({ invite, login: email!, password })
  }

  const closeDialog = () => {
    setDialogOpen(false)
    redirectRoute('signup')
  }

  return (
    <AuthLayout title="Accept Workspace Invitation" type="signup">
      <Dialog open={dialogOpen} onOpenChange={closeDialog}>
        <DialogTitle>
          <AlertTriangle /> Invitation Error
        </DialogTitle>
        <DialogContent className="my-4">Invalid or expired invitation link.</DialogContent>
        <DialogFooter>
          <Button variant="accent" as="a" href={getRoutePath('signup')}>
            Go to Sign Up
          </Button>
        </DialogFooter>
      </Dialog>

      {!dialogOpen && (
        <>
          {hasGoogleOAuth ?
            <>
              <div className="flex w-full justify-between gap-2">
                <GoogleButton invite={invite} />
              </div>
              <Divider />
            </>
          : null}
          <form onSubmit={handleSubmit(onSubmit)} className="flex flex-col gap-5">
            <TextField
              label="Email"
              {...register('login')}
              disabled
              error={errors.login?.message as string}
            />
            <TextField
              autoComplete="new-password"
              label="Password"
              type="password"
              autoFocus
              placeholder="8-24 Characters"
              {...register('password')}
              error={errors.password?.message as string}
            />
            <TextField
              autoComplete="new-password"
              label="Confirm Password"
              type="password"
              {...register('passwordConfirmation')}
              error={errors.passwordConfirmation?.message as string}
            />
            <Button className="mt-2" size="large" variant="secondary" type="submit" loading={isSubmitting}>
              Create Account
            </Button>
          </form>
        </>
      )}
    </AuthLayout>
  )
}
