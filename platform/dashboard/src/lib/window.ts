import { atom, computed, onMount } from 'nanostores'

import { debounce } from '~/lib/utils'

export const $windowSize = atom<{ height: number; width: number }>({
  height: 0,
  width: 0
})

export const isMobile = computed($windowSize, ({ width }) => width <= 768)

const handleResize = debounce(function handleResize() {
  if (typeof window === 'undefined') {
    return { height: 0, width: 0 }
  }
  $windowSize.set({ height: window.innerHeight, width: window.innerWidth })
}, 100)

onMount($windowSize, () => {
  handleResize()
  window.addEventListener('resize', handleResize)
  return () => {
    window.removeEventListener('resize', handleResize)
  }
})
