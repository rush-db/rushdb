import type { ReactNode } from 'react'
import type { FormState } from 'react-hook-form'

import { cn } from '~/lib/utils'

import { Button } from './Button'
import { Card, CardBody, CardFooter, CardHeader } from './Card'

export function Setting({
  button,
  children,
  className,
  description,
  isDirty,
  isSubmitting,
  isValid,
  title,
  readOnly,
  ...props
}: TPolymorphicComponentProps<
  'form',
  {
    button?: ReactNode
    description?: ReactNode
    title: ReactNode
    readOnly?: boolean
  } & Partial<
    Pick<
      FormState<Record<string, unknown>>,
      'isDirty' | 'isSubmitting' | 'isValid'
    >
  >
>) {
  return (
    <li>
      <Card className={className}>
        <form {...props}>
          <CardHeader description={description} title={title} />
          {children && <CardBody>{children}</CardBody>}
          {!readOnly ? (
            <CardFooter>
              {isDirty && (
                <Button type="reset" variant="secondary">
                  Reset
                </Button>
              )}

              {button ?? (
                <Button
                  disabled={!isValid}
                  loading={isSubmitting}
                  type="submit"
                  variant="primary"
                >
                  Save
                </Button>
              )}
            </CardFooter>
          ) : null}
        </form>
      </Card>
    </li>
  )
}

export function SettingsList({
  className,
  ...props
}: TPolymorphicComponentProps<'ul'>) {
  return (
    <ul className={cn('container flex flex-col gap-5', className)} {...props} />
  )
}
