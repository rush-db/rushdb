import { useEffect } from 'react'

export type Key =
  | 'ArrowDown'
  | 'ArrowLeft'
  | 'ArrowRight'
  | 'ArrowUp'
  | 'Backspace'
  | 'Delete'
  | 'End'
  | 'Enter'
  | 'Escape'
  | 'Home'
  | 'PageDown'
  | 'PageUp'
  | 'Space'
  | 'Tab'

export const SHIFT_MODIFIER = 'shift' as const
export const ALT_MODIFIER = 'alt' as const
export const META_MODIFIER = 'meta' as const
export const CTRL_MODIFIER = 'ctrl' as const

export const modifiers = [
  SHIFT_MODIFIER,
  ALT_MODIFIER,
  META_MODIFIER,
  CTRL_MODIFIER
] as const

export type ModifierKey = (typeof modifiers)[number]

export type HotKeyEventHandler = (event: KeyboardEvent) => void

export type Hotkeys = Partial<
  Record<`${ModifierKey}+${Key}` | Key | (string & {}), HotKeyEventHandler>
>

export const shouldFireEvent = (event: KeyboardEvent) => {
  if (event.target instanceof HTMLElement) {
    return !['INPUT', 'SELECT', 'TEXTAREA'].includes(event.target.tagName)
  }
  return true
}

const isHotKeyModifier = (key: string): key is ModifierKey =>
  (modifiers as ReadonlyArray<string>).includes(key)

const parseHotkey = (hotkey: string): [ModifierKey[], string] => {
  const parsedKeys = hotkey
    .toLowerCase()
    .split('+')
    .map((part) => part.trim())

  let passedKey = ''
  const passedModifiers: ModifierKey[] = []

  for (const parsedKey of parsedKeys) {
    if (isHotKeyModifier(parsedKey)) {
      passedModifiers.push(parsedKey)
    } else {
      passedKey = parsedKey
    }
  }

  return [passedModifiers, passedKey]
}

export const matchesHotKey =
  (hotkey: string) =>
  (event: KeyboardEvent): boolean => {
    const [passedModifiers, passedKey] = parseHotkey(hotkey)

    const eventModifiersMap: Record<ModifierKey, boolean> = {
      shift: event.shiftKey,
      alt: event.altKey,
      meta: event.metaKey,
      ctrl: event.ctrlKey
    }
    const eventKey = event.key.toLowerCase()
    // low browser support, event code may not be present in some old or rare browsers such as Samsung Internet
    const eventCode = event.code?.toLowerCase() as string | undefined

    const matchesModifier = Object.entries(eventModifiersMap).every(
      ([eventModifier, eventModifierValue]) =>
        passedModifiers.includes(eventModifier as ModifierKey)
          ? eventModifierValue === true // passed values must be included in the event
          : eventModifierValue === false // missing values must be absent in the event
    )

    const matchesKey =
      eventKey === passedKey ||
      eventCode === passedKey ||
      eventCode?.slice(-1) === passedKey

    if (matchesKey && matchesModifier) {
      return true
    }

    return false
  }

/**
 * Binds hotkeys to document element
 */
export const useHotkeys = (
  hotkeys: Hotkeys,
  { disabled }: { disabled?: boolean } = {}
) => {
  useEffect(() => {
    if (!disabled) {
      const keydownListener = (event: KeyboardEvent) => {
        if (!shouldFireEvent(event)) {
          return
        }
        Object.entries(hotkeys).forEach(([hotkey, callback]) => {
          if (matchesHotKey(hotkey)(event) && callback) {
            callback(event)
          }
        })
      }

      document.documentElement.addEventListener('keydown', keydownListener)
      return () =>
        document.documentElement.removeEventListener('keydown', keydownListener)
    }
  }, [hotkeys, disabled])
}

// /**
//  * Binds hotkeys to react element
//  *
//  * @returns bind object
//  */
// export const useHotKeysHandler = (
//   hotkeys: Hotkeys,
//   { disabled }: { disabled?: boolean } = {}
// ) => {
//   const handleKeyDown = useCallback(
//     (event: React.KeyboardEvent) => {
//       const { nativeEvent } = event

//       if (!disabled) {
//         if (!shouldFireEvent(nativeEvent)) {
//           return
//         }
//         Object.entries(hotkeys).forEach(([hotkey, callback]) => {
//           if (matchesHotKey(hotkey)(nativeEvent) && callback) {
//             callback(nativeEvent)
//           }
//         })
//       }
//     },
//     [disabled, hotkeys]
//   )

//   return useMemo(() => ({ onKeyDown: handleKeyDown }), [handleKeyDown])
// }
