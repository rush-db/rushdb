import { useEffect, useMemo, useState } from 'react'

import { Button } from '~/elements/Button'
import { Dialog, DialogFooter, DialogTitle } from '~/elements/Dialog'
import { FormField } from '~/elements/FormField'
import { TextField, input, inputWrapper } from '~/elements/Input'
import { Message } from '~/elements/Message'
import { toast } from '~/elements/Toast'
import { api } from '~/lib/api'
import { cn } from '~/lib/utils'

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const MESSAGE_MAX_LENGTH = 2000

export function CustomPlanInquiryModal({
  currentPlan,
  defaultEmail,
  onOpenChange,
  open,
  workspaceName
}: {
  currentPlan?: string
  defaultEmail?: string
  onOpenChange: (open: boolean) => void
  open: boolean
  workspaceName?: string
}) {
  const [email, setEmail] = useState(defaultEmail ?? '')
  const [message, setMessage] = useState('')
  const [error, setError] = useState<string>()
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open) {
      setEmail(defaultEmail ?? '')
      setMessage('')
      setError(undefined)
    }
  }, [defaultEmail, open])

  const messageCount = useMemo(() => message.length, [message])

  const validate = () => {
    const normalizedEmail = email.trim()

    if (!normalizedEmail) {
      return 'Contact email is required.'
    }

    if (!EMAIL_PATTERN.test(normalizedEmail)) {
      return 'Enter a valid email address.'
    }

    if (message.trim().length > MESSAGE_MAX_LENGTH) {
      return `Message must be ${MESSAGE_MAX_LENGTH} characters or less.`
    }

    return undefined
  }

  const handleSubmit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault()

    const nextError = validate()
    if (nextError) {
      setError(nextError)
      return
    }

    setSubmitting(true)
    setError(undefined)

    try {
      await api.billing.submitInquiry({
        email: email.trim(),
        message: message.trim() || undefined,
        workspaceName,
        currentPlan
      })

      toast({
        title: 'Inquiry sent',
        description: 'We received your custom plan request and will get back to you shortly.'
      })

      onOpenChange(false)
    } catch (submissionError: any) {
      const description =
        submissionError instanceof Error ? submissionError.message : 'Unable to send inquiry right now.'

      setError(description)
      toast({
        title: 'Inquiry failed',
        description,
        variant: 'danger'
      })
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open} className="gap-5">
      <div className="space-y-2 pr-8">
        <DialogTitle>Talk to us about a custom plan</DialogTitle>
        <p className="text-content2 text-sm">
          Share your contact email and any rollout context. We&apos;ll follow up with a tailored plan.
        </p>
      </div>

      <form className="flex flex-col gap-4" onSubmit={handleSubmit}>
        <TextField
          autoComplete="email"
          error={error?.toLowerCase().includes('email') ? error : undefined}
          label="Contact email"
          onChange={(event) => setEmail(event.target.value)}
          placeholder="team@company.com"
          value={email}
        />

        <FormField
          caption={`${messageCount}/${MESSAGE_MAX_LENGTH}`}
          error={error && !error.toLowerCase().includes('email') ? error : undefined}
          label="Message"
        >
          <label
            className={cn(inputWrapper({ size: 'medium', variant: 'primary' }), 'min-h-32 items-start py-3')}
          >
            <textarea
              className={cn(input({ size: 'medium', variant: 'primary' }), 'min-h-24 resize-y')}
              maxLength={MESSAGE_MAX_LENGTH}
              onChange={(event) => setMessage(event.target.value)}
              placeholder="Tell us about deployment needs, scale, compliance, or procurement requirements."
              value={message}
            />
          </label>
        </FormField>

        {error && !error.toLowerCase().includes('email') && !error.toLowerCase().includes('message') && (
          <Message variant="danger" size="small">
            {error}
          </Message>
        )}

        <DialogFooter className="mt-2 bg-transparent">
          <Button disabled={submitting} onClick={() => onOpenChange(false)} type="button" variant="secondary">
            Cancel
          </Button>
          <Button loading={submitting} type="submit" variant="primary">
            Send Inquiry
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  )
}
