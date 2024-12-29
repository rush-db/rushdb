import { createContext } from 'react'
import { useContext, useMemo, useState } from 'react'

import { useControllableState } from '~/hooks/useControllableState'

type TDisclosureContext = {
  close: () => void
  isOpen: boolean
  open: () => void
  setOpen: (open: boolean) => void
}

export const DisclosureContext = createContext<TDisclosureContext | null>(null)

export const useDisclosureContext = () => {
  const ctx = useContext(DisclosureContext)

  if (!ctx) {
    throw new Error('Must be used within Disclosure Provider')
  }

  return ctx
}

export type DisclosureOptions = {
  onOpenChange?: (open: boolean) => void
  open?: boolean
}

export const useDisclosure = ({
  open: openProp,
  onOpenChange: onOpenChangeProp
}: DisclosureOptions = {}) => {
  const [isOpen, setOpen] = useControllableState({
    value: openProp,
    onChange: onOpenChangeProp
  })

  return useMemo<TDisclosureContext>(
    () => ({
      isOpen,
      setOpen,
      open: () => {
        setOpen(true)
      },
      close: () => {
        setOpen(false)
      }
    }),
    [isOpen]
  )
}
