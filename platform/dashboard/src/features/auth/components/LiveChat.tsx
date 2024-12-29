import { useStore } from '@nanostores/react'
import { useEffect } from 'react'

import type { AnyObject } from '~/types'

import { $user } from '../stores/user'

const config = {
  id: '3Y9qAjMyWH7tt26zB',
  background: '#CBEE4C' /* chat button background color */,
  foreground: '#000' /* chat button text color */
}

declare global {
  interface Window {
    Chatra?: {
      (command?: 'expandWidget' | 'hide' | 'kill'): void
      q?: []
    }
    ChatraID: string
    ChatraIntegration: AnyObject
    ChatraSetup: {}
  }
}

const loadWidget = () => {
  window.ChatraSetup = {
    // https://chatra.com/help/api/#api-reference
    buttonStyle: 'round',
    buttonSize: 48,
    colors: {
      buttonText: config.foreground,
      buttonBg: config.background
      // clientBubbleBg: '#e7ffd1', /* visitor’s message bubble color */
      // agentBubbleBg: '#deffff'
    }
    // gaTrackingId: 'UA-12345678-1'
  }

  window.ChatraID = config.id
  const script = document.createElement('script')
  script.src = 'https://call.chatra.io/chatra.js'
  script.defer = true

  window.Chatra =
    window.Chatra ||
    function () {
      // @ts-ignore
      // eslint-disable-next-line prefer-rest-params, @typescript-eslint/no-extra-semi
      ;(window.Chatra.q = window.Chatra.q || []).push(arguments)
    }

  if (document.head) document.head.appendChild(script)
}

export function LiveChat() {
  const user = useStore($user)

  useEffect(() => {
    if (!user.isLoggedIn) {
      return window.Chatra?.('kill')
    }

    window.ChatraIntegration = {
      email: user.login,
      clientId: user.id,
      name: user.login
    }
    loadWidget()
  }, [user])

  return null
}
