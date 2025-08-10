import { useStore } from '@nanostores/react'

import { Button } from '~/elements/Button'
import { StatusDialog } from '~/elements/StatusDialog'
import { $searchParams, getRoutePath, removeSearchParam } from '~/lib/router'

const SEARCH_PARAM = 'payment_successful'

const close = () => removeSearchParam(SEARCH_PARAM)

const textsMap = {
  success: {
    title: 'Payment successful',
    primaryAction: 'Great',
    description: 'Enjoy your new subscription plan'
  },
  info: {
    title: 'Order canceled',
    primaryAction: 'Okay',
    description: "Сontinue to shop around and checkout when you're ready."
  }
}

export function PaymentCallbackDialog() {
  const params = useStore($searchParams)

  const open = SEARCH_PARAM in params

  const successful = params[SEARCH_PARAM] !== 'false'

  const status = successful ? 'success' : 'info'

  const texts = textsMap[status]

  return (
    <StatusDialog
      secondaryActions={
        <Button
          as="a"
          href={getRoutePath('workspaceBilling')}
          onClick={close}
          variant="secondary"
        >
          Go to subscriptions
        </Button>
      }
      description={texts.description}
      onOpenChange={close}
      open={open}
      primaryAction={close}
      primaryText={texts.primaryAction}
      title={texts.title}
      variant={status}
    />
  )
}
