import { atom, onMount } from 'nanostores'

import { debounce } from '~/lib/utils'

export const $mousePosition = atom<{ x: number; y: number }>({
  x: 0,
  y: 0
})

function handleMouseMove(event: MouseEvent) {
  if (typeof window === 'undefined') {
    return { height: 0, width: 0 }
  }
  $mousePosition.set({ x: event.clientX, y: event.clientY })
}

const debouncedHandleMouseMove = debounce(handleMouseMove, 100)

onMount($mousePosition, () => {
  window.addEventListener('mousemove', debouncedHandleMouseMove)
  return () => {
    window.removeEventListener('mousemove', debouncedHandleMouseMove)
  }
})
