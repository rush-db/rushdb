import type { PropertyWithValue } from '@rushdb/javascript-sdk'
import { useMemo, type PointerEventHandler } from 'react'

import { flip, shift, useFloating } from '@floating-ui/react-dom'
import { useStore } from '@nanostores/react'
import { atom, onMount } from 'nanostores'

import type { AnySearchOperation } from '~/features/search/types'

import { Divider } from '~/elements/Divider'
import { IconButton, IconCopyButton } from '~/elements/IconButton'
import { Kbd } from '~/elements/Kbd'
import { Link } from '~/elements/Link'
import { Tooltip } from '~/elements/Tooltip'
import { addFilter } from '~/features/projects/stores/current-project'
import { SearchOperationIcon } from '~/features/search/components/SearchOperationIcon'
import { SearchOperations, operationsRecord } from '~/features/search/constants'
import { getViableSearchOperations, isViableSearchOperation } from '~/features/search/types'
import { useHotkeys } from '~/hooks/useHotkeys'
import { $router } from '~/lib/router'
import { cn, isUrl } from '~/lib/utils'

import { formatPropertyValue } from '../utils'
import { PropertyName } from './PropertyName'

export const PROPERTY_TOOLTIP_ID = 'propertyValueTooltip'

export const $propertyValueTooltip = atom<
  | (Pick<PropertyWithValue, 'name' | 'type' | 'value'> & {
      element: HTMLElement
      showOperations: boolean
    } & { date?: string })
  | undefined
>()

$router.subscribe(() => {
  $propertyValueTooltip.set(undefined)
})

onMount($propertyValueTooltip, () => {
  // close tooltip on scroll
  const handleScroll = () => {
    $propertyValueTooltip.set(undefined)
  }
  window.addEventListener('scroll', handleScroll)
  return () => {
    window.removeEventListener('scroll', handleScroll)
  }
})

export const handlePointerEnter =
  ({
    property,
    showOperations = true
  }: {
    property: Pick<PropertyWithValue, 'name' | 'type' | 'value'> & { date?: string }
    showOperations?: boolean
  }): PointerEventHandler<HTMLElement> =>
  (event) => {
    const element = event.currentTarget as HTMLElement
    $propertyValueTooltip.set({ ...property, showOperations, element })
  }

export const handlePointerLeave: PointerEventHandler<HTMLElement> = (event) => {
  const relatedTarget = event.relatedTarget as HTMLElement | null

  if (relatedTarget?.id === PROPERTY_TOOLTIP_ID) {
    return
  }

  $propertyValueTooltip.set(undefined)
}

function Operations({ property }: { property: Pick<PropertyWithValue, 'name' | 'type' | 'value'> }) {
  const createHandler = (operation: SearchOperations) => () => {
    if (
      !isViableSearchOperation({
        propertyType: property.type,
        searchOperation: operation
      })
    ) {
      return
    }

    // @TODO handle arrays
    addFilter({
      operation,
      name: property.name,
      value: property.value
    } as AnySearchOperation)

    $propertyValueTooltip.set(undefined)
  }

  const { hotkeys, operations } = useMemo(() => {
    const operations: Record<SearchOperations, { code: string; handler: () => void; label: string }> = {
      [SearchOperations.Equals]: {
        code: 'E',
        handler: createHandler(SearchOperations.Equals),
        label: operationsRecord[SearchOperations.Equals].label
      },
      [SearchOperations.Contains]: {
        code: 'C',
        handler: createHandler(SearchOperations.Contains),
        label: operationsRecord[SearchOperations.Contains].label
      },
      [SearchOperations.StartsWith]: {
        code: 'shift+s',
        handler: createHandler(SearchOperations.StartsWith),
        label: operationsRecord[SearchOperations.StartsWith].label
      },
      [SearchOperations.EndsWith]: {
        code: 'shift+e',
        handler: createHandler(SearchOperations.EndsWith),
        label: operationsRecord[SearchOperations.EndsWith].label
      },
      [SearchOperations.Greater]: {
        code: 'g',
        handler: createHandler(SearchOperations.Greater),
        label: operationsRecord[SearchOperations.Greater].label
      },
      [SearchOperations.GreaterOrEqual]: {
        code: 'alt+g',
        handler: createHandler(SearchOperations.GreaterOrEqual),
        label: operationsRecord[SearchOperations.GreaterOrEqual].label
      },
      [SearchOperations.Less]: {
        code: 'l',
        handler: createHandler(SearchOperations.Less),
        label: operationsRecord[SearchOperations.Less].label
      },
      [SearchOperations.LessOrEqual]: {
        code: 'alt+l',
        handler: createHandler(SearchOperations.LessOrEqual),
        label: operationsRecord[SearchOperations.LessOrEqual].label
      },
      [SearchOperations.NotEquals]: {
        code: 'n',
        handler: createHandler(SearchOperations.NotEquals),
        label: operationsRecord[SearchOperations.NotEquals].label
      }
      // [SearchOperations.Exists]: {
      //   code: 'e',
      //   handler: createHandler(SearchOperations.Exists),
      //   label: operationsRecord[SearchOperations.Exists].label
      // },
      // [SearchOperations.Type]: {
      //   code: 't',
      //   handler: createHandler(SearchOperations.Type),
      //   label: operationsRecord[SearchOperations.Type].label
      // }
    }

    const hotkeys = Object.fromEntries(Object.values(operations).map((entry) => [entry.code, entry.handler]))

    return { hotkeys, operations }
  }, [property])

  useHotkeys(hotkeys)

  return (
    <>
      <Divider />
      <div className="flex w-full flex-wrap gap-1">
        {getViableSearchOperations(property.type).map((operation) => {
          if (!(operation in operations)) {
            if (import.meta.env.NODE_ENV)
              console.warn(`Operation ${operation} missing in PropertyValueTooltip config`)

            return null
          }

          const { handler, code, label } = operations[operation]

          const text = `Add ${label} filter`

          return (
            <Tooltip
              key={label}
              trigger={
                <IconButton aria-label={text} onClick={handler} size="xsmall" variant="secondary">
                  <SearchOperationIcon operation={operation} />
                </IconButton>
              }
            >
              <Kbd code={code}>{text}</Kbd>
            </Tooltip>
          )
        })}
      </div>
    </>
  )
}

export function PropertyValueTooltip() {
  const propertyTooltip = useStore($propertyValueTooltip)

  const { refs, floatingStyles, isPositioned } = useFloating({
    open: typeof propertyTooltip !== 'undefined',
    placement: 'bottom-start',
    elements: {
      reference: propertyTooltip?.element
    },
    middleware: [shift(), flip()]
  })

  if (!propertyTooltip) {
    return null
  }

  const { showOperations, ...property } = propertyTooltip ?? {}

  const formattedValue = formatPropertyValue(property)

  const hasUrl = isUrl(formattedValue)
  const Component = hasUrl ? Link : 'h6'

  return (
    <div
      className={cn(
        'z-tooltip bg-menu text-menu-contrast pointer-events-auto fixed flex max-h-[60vh] w-max min-w-[200px] max-w-[300px] flex-col justify-start gap-1 overflow-auto rounded-md border p-2 shadow-lg'
        // { 'transition-transform': isPositioned }
      )}
      onPointerDown={(event) => {
        event.stopPropagation()
      }}
      style={{
        ...floatingStyles
      }}
      id={PROPERTY_TOOLTIP_ID}
      onPointerLeave={handlePointerLeave}
      ref={refs.setFloating}
    >
      <div className="flex justify-between">
        <PropertyName
          className="bg-accent/30 text-2xs text-accent gap-1 rounded-sm px-1 leading-snug"
          iconSize={12}
          name={property.name}
          type={property.type}
        />{' '}
        <p className="text-accent text-xs">{property.date}</p>
      </div>

      <div className="flex justify-between gap-1">
        <div className="flex overflow-auto">
          <Component
            {...(hasUrl ? { href: formattedValue, target: '_blank' } : {})}
            className="mt-2 whitespace-normal text-xs font-medium"
          >
            {formattedValue}
          </Component>
        </div>

        <IconCopyButton
          className="pointer-events-auto"
          onClick={(e: { stopPropagation: () => any }) => e.stopPropagation()}
          size="xsmall"
          text={formattedValue}
          variant="secondary"
        />
      </div>

      {showOperations && <Operations property={property} />}
    </div>
  )
}
