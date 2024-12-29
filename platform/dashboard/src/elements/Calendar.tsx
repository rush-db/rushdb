'use client'

import { ChevronLeft, ChevronRight } from 'lucide-react'
import * as React from 'react'
import { DayPicker } from 'react-day-picker'

import { cn } from '~/lib/utils'

import { buttonVariants } from './Button'
import { iconButton } from './IconButton'

export type CalendarProps = React.ComponentProps<typeof DayPicker>

export * from 'react-day-picker'

export { default as formatIso } from 'date-fns/formatISO'

export function Calendar({
  className,
  classNames,
  showOutsideDays = true,
  initialFocus = true,
  ...props
}: CalendarProps) {
  return (
    <DayPicker
      classNames={{
        months: 'flex flex-col sm:flex-row space-y-4 sm:space-x-4 sm:space-y-0',
        month: 'space-y-2',
        caption: 'flex justify-center pt-1 h-[36px] relative items-center',
        caption_label: 'text-sm font-medium',
        nav: 'space-x-1 flex items-center',
        nav_button: cn(
          'shrink-none',
          buttonVariants({ size: 'small', variant: 'outline' }),
          iconButton({ size: 'small' })
          // 'h-7 w-7 bg-transparent p-0 opacity-50 hover:opacity-100'
        ),
        nav_button_previous: 'absolute left-1',
        nav_button_next: 'absolute right-1',
        table: 'w-full border-collapse space-y-1',
        head_row: 'flex',
        head_cell: 'text-content3 rounded-md w-9 font-normal text-[0.8rem]',
        row: 'flex w-full mt-2',
        cell: 'text-center text-sm p-0 relative first:[&:has([aria-selected])]:rounded-l-md last:[&:has([aria-selected])]:rounded-r-md focus-within:relative focus-within:z-20 [&:has([aria-selected])]:text-accent-contrast [&:has([aria-selected])]:text-accent',
        day: cn(
          buttonVariants({ size: 'small', variant: 'ghost' }),
          iconButton({ size: 'small' }),
          'font-normal'
        ),
        day_selected: cn(
          buttonVariants({ size: 'small', variant: 'accent' }),
          iconButton({ size: 'small' }),
          'font-normal aria-selected:hover:bg-accent-hover'
        ),
        day_today: 'text-accent',
        day_outside: 'text-content2 aria-selected:text-accent-contrast',
        day_disabled: 'text-content3',
        day_range_middle:
          'aria-selected:rounded-none aria-selected:bg-accent/20 aria-selected:text-accent',
        day_hidden: 'invisible',
        ...classNames
      }}
      components={{
        IconLeft: () => <ChevronLeft className="h-4 w-4" />,
        IconRight: () => <ChevronRight className="h-4 w-4" />
      }}
      className={cn('', className)}
      initialFocus={initialFocus}
      showOutsideDays={showOutsideDays}
      {...props}
    />
  )
}
Calendar.displayName = 'Calendar'
