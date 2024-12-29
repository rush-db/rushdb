import type {
  CollectProperty,
  CollectPropertySingleValue
} from '@collect.so/javascript-sdk'
import type { Control, UseFormSetValue, UseFormWatch } from 'react-hook-form'

import { useStore } from '@nanostores/react'
import { PlusCircle, X } from 'lucide-react'
import { atom } from 'nanostores'
import { useEffect, useState } from 'react'
import { Controller } from 'react-hook-form'

// import type { DateRange } from '~/elements/Calendar'
import type { FormFieldProps } from '~/elements/FormField'
import type { InputProps } from '~/elements/Input'
import type { Filter } from '~/features/search/types'
import type { InferType } from '~/lib/form'

import { Button } from '~/elements/Button'
import { Calendar, formatIso } from '~/elements/Calendar'
import {
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxPopover,
  SearchItem
} from '~/elements/Combobox'
import { Combobox } from '~/elements/Combobox'
import { FormField } from '~/elements/FormField'
import { IconButton } from '~/elements/IconButton'
import { TextField } from '~/elements/Input'
import { Message } from '~/elements/Message'
import { Popover, PopoverContent, PopoverTrigger } from '~/elements/Popover'
import { SearchSelect, SelectItem } from '~/elements/SearchSelect'
import { Slider } from '~/elements/Slider'
import { Tooltip } from '~/elements/Tooltip'
import {
  $currentProjectSuggestedFields,
  editFilter
} from '~/features/projects/stores/current-project'
import { PropertyName } from '~/features/properties/components/PropertyName'
import { PropertyTypeIcon } from '~/features/properties/components/PropertyTypeIcon'
import { formatPropertyValue } from '~/features/properties/utils'
import { SearchOperationIcon } from '~/features/search/components/SearchOperationIcon'
import { SearchOperations, operatorOptions } from '~/features/search/constants'
import { isViableSearchOperation } from '~/features/search/types'
import { api } from '~/lib/api'
import { createAsyncStore } from '~/lib/fetcher'
import { mixed, object, string, useForm } from '~/lib/form'
// import { formatMinMax } from '~/lib/formatters'

const $fieldId = atom<CollectProperty['id'] | undefined>(undefined)
const $fieldValues = createAsyncStore({
  key: '$fieldValues',
  deps: [$fieldId],
  async fetcher(init) {
    const fieldId = $fieldId.get()
    if (!fieldId) {
      return
    }
    return await api.properties.values({ init, propertyId: fieldId })
  }
})

const filterSchema = object({
  field: string().required(),
  operator: string().oneOf(Object.values(SearchOperations)).required(),
  value: mixed(),
  min: mixed(),
  max: mixed()
})

type TFilterSchema = InferType<typeof filterSchema>

function SelectSearchOperator({
  control,
  name,
  setValue,
  ...props
}: TExtendableProps<
  InputProps & FormFieldProps,
  {
    control: Control<any>
    name: string
    setValue: UseFormSetValue<any>
  }
>) {
  return (
    <Controller
      render={({ field, fieldState }) => {
        const currentOption = operatorOptions.find(
          (o) => o.value === field.value
        )

        return (
          <SearchSelect
            trigger={
              <TextField
                prefix={
                  currentOption && (
                    <SearchOperationIcon operation={currentOption.value} />
                  )
                }
                error={fieldState.error?.message}
                label="Operator"
                readOnly
                {...props}
                {...field}
                size="small"
                value={currentOption?.label}
              />
            }
          >
            {operatorOptions.map((o) => (
              <SelectItem
                onSelect={() => {
                  setValue(name, o.value, {
                    shouldDirty: true
                  })
                }}
                key={o.value}
                value={o.label}
              >
                <SearchOperationIcon operation={o.value} />
                {o.label}
              </SelectItem>
            ))}
          </SearchSelect>
        )
      }}
      control={control}
      name={name}
    />
  )
}

function SelectField({
  control,
  name,
  setValue,
  fields,
  value,
  ...props
}: TExtendableProps<
  InputProps & FormFieldProps,
  {
    control: Control<any>
    fields: CollectProperty[]
    name: string
    setValue: UseFormSetValue<any>
    value?: CollectProperty
  }
>) {
  return (
    <Controller
      render={({ field, fieldState }) => (
        <SearchSelect
          trigger={
            <TextField
              className="cursor-pointer"
              label="Property"
              prefix={value && <PropertyTypeIcon type={value?.type} />}
              readOnly
              size="small"
              {...field}
              {...props}
              error={fieldState.error?.message}
            />
          }
        >
          {fields.map((field) => (
            <SelectItem
              onSelect={() => {
                setValue(name, field.name, { shouldDirty: true })
              }}
              key={field.id}
              value={field.name}
            >
              <PropertyTypeIcon type={field.type} />
              {field.name}
              <span className="hidden">{field.id}</span>
            </SelectItem>
          ))}
        </SearchSelect>
      )}
      control={control}
      name={name}
    />
  )
}

function SelectOperationValue({
  control,
  name,
  setValue,
  value,
  field,
  ...props
}: TExtendableProps<
  React.ComponentPropsWithoutRef<typeof Combobox>,
  {
    control: Control<TFilterSchema>
    field?: CollectProperty
    name: keyof TFilterSchema
    setValue: UseFormSetValue<TFilterSchema>
    value?: CollectPropertySingleValue
  }
>) {
  const [open, setOpen] = useState(false)

  const fieldType = field?.type
  const { data: fieldValues } = useStore($fieldValues)
  const values = fieldValues?.values

  return (
    <Controller
      render={({ field }) => {
        const currentValue = field.value
        const formattedCurrent = formatPropertyValue({
          value: currentValue,
          type: fieldType!
        })

        return (
          <Combobox
            {...props}
            onOpenChange={(open) => {
              if (open && fieldType !== 'string') setValue(name, '')
              setOpen(open)
            }}
            open={open}
          >
            <FormField label="Value">
              <ComboboxInput
                onValueChange={(search) => setValue(name, search)}
                prefix={fieldType && <PropertyTypeIcon type={fieldType} />}
                showSearchIcon={false}
                size="small"
                value={formattedCurrent}
              />
            </FormField>
            <ComboboxPopover>
              <ComboboxList>
                <SearchItem
                  hasMatch={
                    !!formattedCurrent &&
                    values?.some(
                      (value) =>
                        formatPropertyValue({ value, type: fieldType! }) ==
                        formattedCurrent
                    )
                  }
                  onSelect={() => {
                    setValue(name, currentValue, {
                      shouldDirty: true
                    })
                    setOpen(false)
                  }}
                  size="small"
                  value={formattedCurrent}
                >
                  {currentValue}
                </SearchItem>

                {values?.map((value) => {
                  const formattedValue = formatPropertyValue({
                    value,
                    // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
                    type: fieldType!
                  })
                  return (
                    <ComboboxItem
                      onSelect={() => {
                        setValue(name, value, {
                          shouldDirty: true
                        })
                        setOpen(false)
                      }}
                      key={`value-${value}`}
                      size="small"
                      value={value?.toString()}
                    >
                      {fieldType && <PropertyTypeIcon type={fieldType} />}

                      <span className="truncate">{formattedValue}</span>
                    </ComboboxItem>
                  )
                })}
              </ComboboxList>
            </ComboboxPopover>
          </Combobox>
        )
      }}
      control={control}
      name={name}
    />
  )
}

// function SelectNumberMinMax({
//   setValue,
//   // value,
//   field,
//   min,
//   max,
//   operator,
//   ...props
// }: TExtendableProps<
//   React.ComponentPropsWithoutRef<typeof Combobox>,
//   {
//     control: Control<TFilterSchema>
//     field?: CollectProperty
//     max?: number
//     min?: number
//     operator: SearchOperations
//     setValue: UseFormSetValue<TFilterSchema>
//   }
// >) {
//   const { data: fieldValues } = useStore($fieldValues)
//
//   if (field?.type !== 'number') {
//     return null
//   }
//
//   const hasMin = typeof min !== 'undefined'
//   const hasMax = typeof max !== 'undefined'
//
//   const value = [min, max]
//
//   return (
//     <div className="flex w-full flex-col gap-5">
//       <div className="flex items-end justify-between gap-3">
//         {hasMin ? (
//           <TextField
//             onChange={(event) => {
//               setValue('min', event.currentTarget.valueAsNumber, {
//                 shouldDirty: true
//               })
//             }}
//             suffix={
//               <Tooltip
//                 trigger={
//                   <IconButton
//                     onClick={() => {
//                       setValue('min', undefined, {
//                         shouldDirty: true
//                       })
//                     }}
//                     aria-label="Remove min bound"
//                     className="-mr-2"
//                     size="xsmall"
//                   >
//                     <X />
//                   </IconButton>
//                 }
//               >
//                 Remove lower bound
//               </Tooltip>
//             }
//             className="w-32"
//             label="Min"
//             placeholder="Min"
//             size="small"
//             type="number"
//             value={min}
//           />
//         ) : (
//           <FormField label="Min">
//             <Tooltip
//               trigger={
//                 <IconButton
//                   onClick={() => {
//                     setValue('min', fieldValues?.min, {
//                       shouldDirty: true
//                     })
//                   }}
//                   aria-label="Add upper bound"
//                   size="small"
//                   variant="outline"
//                 >
//                   <PlusCircle />
//                 </IconButton>
//               }
//             >
//               Add lower bound
//             </Tooltip>
//           </FormField>
//         )}
//
//         {hasMax ? (
//           <TextField
//             onChange={(event) => {
//               setValue('max', event.currentTarget.valueAsNumber, {
//                 shouldDirty: true
//               })
//             }}
//             suffix={
//               <Tooltip
//                 trigger={
//                   <IconButton
//                     onClick={() => {
//                       setValue('max', undefined, {
//                         shouldDirty: true
//                       })
//                     }}
//                     aria-label="Remove min bound"
//                     className="-mr-2"
//                     size="xsmall"
//                   >
//                     <X />
//                   </IconButton>
//                 }
//               >
//                 Remove upper bound
//               </Tooltip>
//             }
//             className="w-32"
//             label="Max"
//             placeholder="Max"
//             size="small"
//             type="number"
//             value={max}
//           />
//         ) : (
//           <FormField label="Max">
//             <Tooltip
//               trigger={
//                 <IconButton
//                   onClick={() => {
//                     setValue('max', fieldValues?.max, {
//                       shouldDirty: true
//                     })
//                   }}
//                   aria-label="Add upper bound"
//                   size="small"
//                   variant="outline"
//                 >
//                   <PlusCircle />
//                 </IconButton>
//               }
//             >
//               Add upper bound
//             </Tooltip>
//           </FormField>
//         )}
//       </div>
//
//       <Slider
//         onValueChange={(newValue) => {
//           let newMin, newMax
//           if (newValue.length > 1) {
//             newMin = newValue[0]
//             newMax = newValue[1]
//           } else if (hasMin) {
//             newMin = newValue[0]
//           } else {
//             newMax = newValue[0]
//           }
//
//           setValue('min', newMin, {
//             shouldDirty: true
//           })
//           setValue('max', newMax, {
//             shouldDirty: true
//           })
//         }}
//         inverted={!hasMax && hasMin}
//         max={fieldValues?.max}
//         min={fieldValues?.min}
//         thumbsCount={value.filter(Boolean)?.length}
//         value={value.filter(Boolean) as number[]}
//       />
//     </div>
//   )
// }

function NumberValues({
  watch,
  field,
  control,
  setValue
}: {
  control: Control<TFilterSchema>
  field: CollectProperty
  setValue: UseFormSetValue<TFilterSchema>
  watch: UseFormWatch<TFilterSchema>
}) {
  const selectedOperator = watch('operator')
  let selectedValue = watch('value')
  let selectedMin = watch('min')
  let selectedMax = watch('max')

  const { data: fieldValues } = useStore($fieldValues)

  if (field?.type !== 'number') {
    return null
  }

  if (typeof selectedMax === 'string') {
    selectedMax = fieldValues?.max
  }
  if (typeof selectedMin === 'string') {
    selectedMin = fieldValues?.min
  }
  if (typeof selectedValue === 'string') {
    selectedValue = undefined
  }

  // if (isNumberRangeOperation({ operation: selectedOperator })) {
  //   return (
  //     <SelectNumberMinMax
  //       control={control}
  //       field={field}
  //       max={selectedMax}
  //       min={selectedMin}
  //       operator={selectedOperator}
  //       setValue={setValue}
  //     />
  //   )
  // }

  if (
    selectedOperator === SearchOperations.Greater ||
    selectedOperator === SearchOperations.GreaterOrEqual
  ) {
    const value =
      selectedValue === undefined || Number.isNaN(selectedValue)
        ? fieldValues?.min ?? 0
        : Number(selectedValue)

    return (
      <div className="flex flex-col gap-5">
        <TextField
          onChange={(event) =>
            setValue('value', event.currentTarget.valueAsNumber, {
              shouldDirty: true
            })
          }
          label="Value"
          size="small"
          type="number"
          value={value}
        />
        <Slider
          onValueChange={(newValue) => {
            setValue('value', newValue[0], {
              shouldDirty: true
            })
          }}
          inverted
          max={fieldValues?.max}
          min={fieldValues?.min}
          thumbsCount={1}
          value={[value]}
        />
      </div>
    )
  }

  if (
    selectedOperator === SearchOperations.Less ||
    selectedOperator === SearchOperations.LessOrEqual
  ) {
    const value =
      selectedValue === undefined || Number.isNaN(selectedValue)
        ? fieldValues?.max ?? 0
        : Number(selectedValue)

    return (
      <div className="flex flex-col gap-5">
        <TextField
          onChange={(event) =>
            setValue('value', event.currentTarget.valueAsNumber, {
              shouldDirty: true
            })
          }
          label="Value"
          size="small"
          type="number"
          value={value}
        />
        <Slider
          onValueChange={(newValue) => {
            setValue('value', newValue[0], {
              shouldDirty: true
            })
          }}
          max={fieldValues?.max}
          min={fieldValues?.min}
          thumbsCount={1}
          value={[value]}
        />
      </div>
    )
  }

  return (
    <SelectOperationValue
      control={control}
      field={field}
      name="value"
      setValue={setValue}
    />
  )
}

function DateTimeValues({
  watch,
  field,
  control,
  setValue
}: {
  control: Control<TFilterSchema>
  field: CollectProperty
  setValue: UseFormSetValue<TFilterSchema>
  watch: UseFormWatch<TFilterSchema>
}) {
  const selectedOperator = watch('operator')
  let selectedValue = watch('value')
  let selectedMin = watch('min')
  let selectedMax = watch('max')

  if (field.type !== 'datetime') {
    return null
  }

  const todayISO = formatIso(new Date())
  if (typeof selectedMin !== 'string') {
    selectedMin = todayISO
  }
  if (typeof selectedMax !== 'string') {
    selectedMax = todayISO
  }
  if (typeof selectedValue !== 'string') {
    selectedValue = todayISO
  }

  // range operations
  // if (isNumberRangeOperation({ operation: selectedOperator })) {
  //   const selected = {
  //     from: new Date(selectedMin),
  //     to: new Date(selectedMax)
  //   } satisfies DateRange
  //
  //   return (
  //     <>
  //       <div className="grid grid-cols-2 gap-[inherit]">
  //         <TextField
  //           onChange={(event) =>
  //             setValue('min', event.target.value, { shouldDirty: true })
  //           }
  //           label="Min"
  //           prefix={<PropertyTypeIcon type="datetime" />}
  //           size="small"
  //           type="datetime"
  //           value={selectedMin}
  //         />
  //         <TextField
  //           onChange={(event) =>
  //             setValue('max', event.target.value, { shouldDirty: true })
  //           }
  //           label="Max"
  //           prefix={<PropertyTypeIcon type="datetime" />}
  //           size="small"
  //           type="datetime"
  //           value={selectedMax}
  //         />
  //       </div>
  //       <Calendar
  //         onSelect={(range) => {
  //           if (range?.from) {
  //             setValue('min', formatIso(range?.from), { shouldDirty: true })
  //           }
  //           if (range?.to) {
  //             setValue('max', formatIso(range?.to), { shouldDirty: true })
  //           }
  //         }}
  //         defaultMonth={selected.from}
  //         initialFocus
  //         mode="range"
  //         numberOfMonths={2}
  //         selected={selected}
  //       />
  //     </>
  //   )
  // }

  if (
    selectedOperator === SearchOperations.Greater ||
    selectedOperator === SearchOperations.GreaterOrEqual
  ) {
    const selected = new Date(selectedValue)

    return (
      <>
        <TextField
          onChange={(event) =>
            setValue('value', event.target.value, { shouldDirty: true })
          }
          label="Value"
          prefix={<PropertyTypeIcon type="datetime" />}
          size="small"
          type="datetime"
          value={selectedValue}
        />
        <Calendar
          onSelect={(from) => {
            if (from) {
              setValue('value', formatIso(from), { shouldDirty: true })
            }
          }}
          defaultMonth={selected}
          initialFocus
          mode="single"
          numberOfMonths={1}
          selected={selected}
        />
      </>
    )
  }

  if (
    selectedOperator === SearchOperations.Less ||
    selectedOperator === SearchOperations.LessOrEqual
  ) {
    const selected = new Date(selectedValue)

    return (
      <>
        <TextField
          onChange={(event) =>
            setValue('value', event.target.value, { shouldDirty: true })
          }
          label="Value"
          prefix={<PropertyTypeIcon type="datetime" />}
          size="small"
          type="datetime"
          value={selectedValue}
        />
        <Calendar
          onSelect={(from) => {
            if (from) {
              setValue('value', formatIso(from), { shouldDirty: true })
            }
          }}
          defaultMonth={selected}
          initialFocus
          mode="single"
          numberOfMonths={1}
          selected={selected}
        />
      </>
    )
  }

  return (
    <SelectOperationValue
      control={control}
      field={field}
      name="value"
      setValue={setValue}
    />
  )
}

function SelectFilterValues({
  watch,
  field,
  control,
  setValue
}: {
  control: Control<TFilterSchema>
  field?: CollectProperty
  setValue: UseFormSetValue<TFilterSchema>
  watch: UseFormWatch<TFilterSchema>
}) {
  const selectedOperator = watch('operator')

  if (!field) {
    return null
  }

  if (
    !isViableSearchOperation({
      propertyType: field.type,
      searchOperation: selectedOperator
    })
  ) {
    return (
      <Message size="small" variant="danger">
        Selected operation is not available for property of this type.
      </Message>
    )
  }

  switch (field.type) {
    case 'datetime':
      return (
        <DateTimeValues
          control={control}
          field={field}
          setValue={setValue}
          watch={watch}
        />
      )
    case 'number':
      return (
        <NumberValues
          control={control}
          field={field}
          setValue={setValue}
          watch={watch}
        />
      )
    default:
      return (
        <SelectOperationValue
          control={control}
          field={field}
          name="value"
          setValue={setValue}
        />
      )
  }
}

export function FilterPopover({
  filter,
  onRemove
}: {
  filter: Filter
  onRemove: (filter: Filter) => void
}) {
  const [open, setOpen] = useState(false)
  const { data: fields = [] } = useStore($currentProjectSuggestedFields)

  const defaultValues = {
    field: filter.name,
    operator: filter.operation,
    value: 'value' in filter ? filter.value : undefined,
    min: 'min' in filter ? filter.min : undefined,
    max: 'max' in filter ? filter.max : undefined
  } as TFilterSchema

  const {
    formState: { isDirty },
    handleSubmit,
    setValue,
    reset,
    control,
    watch,
    getValues
  } = useForm({
    defaultValues,
    schema: filterSchema
  })

  const selectedField = watch('field')

  let rightPart

  const field = fields.find((field) => field.name === selectedField)

  // if (isNumberRangeOperation(filter)) {
  //   rightPart = field ? formatMinMax({ ...filter, type: field?.type }) : ''
  // } else {
  // eslint-disable-next-line prefer-const
  rightPart = field
    ? formatPropertyValue({ value: filter.value, type: field?.type })
    : ''
  // }

  useEffect(() => {
    $fieldId.set(field?.id)
  }, [field?.id])

  const submit = handleSubmit(() => {
    // native values convert types, thus we use getValues helper
    const { value, operator, field: name, min, max } = getValues()

    editFilter({
      filterId: filter.filterId,
      name, // TODO: remove name
      value,
      operation: operator,
      min,
      max
    } as Filter)
    setOpen(false)
    reset(getValues())
  })

  return (
    <Popover onOpenChange={setOpen} open={open}>
      <PopoverTrigger>
        <Button
          as="div"
          className="gap-1 pr-1"
          size="small"
          tabIndex={0}
          variant="outline"
        >
          <PropertyName
            iconSize={12}
            name={filter.name ?? ''}
            type={field?.type}
          />

          <SearchOperationIcon
            className="text-content3"
            operation={filter.operation}
          />

          <span
            className="max-w-[140px] truncate"
            title={rightPart?.toString()}
          >
            {rightPart}
          </span>

          <IconButton
            onClick={() => {
              onRemove(filter)
            }}
            aria-label="Remove filter"
            as="div"
            size="xsmall"
            variant="ghost"
          >
            <X />
          </IconButton>
        </Button>
      </PopoverTrigger>
      <PopoverContent>
        <form
          onReset={() => {
            reset()
          }}
          className="flex min-w-[360px] flex-col px-3 pb-3 pt-3"
          onSubmit={submit}
        >
          <div className="grid grid-cols-2 gap-3">
            <SelectField
              control={control}
              fields={fields}
              name="field"
              setValue={setValue}
              value={field}
            />
            <SelectSearchOperator
              control={control}
              name="operator"
              setValue={setValue}
            />
            <div className="col-span-2 grid gap-[inherit]">
              <SelectFilterValues
                control={control}
                field={field}
                setValue={setValue}
                watch={watch}
              />
            </div>
          </div>
          <div className="flex justify-end gap-3 pt-5">
            {isDirty && (
              <Button size="small" type="reset" variant="secondary">
                Reset
              </Button>
            )}
            <Button size="small" type="submit" variant="primary">
              Save
            </Button>
          </div>
        </form>
      </PopoverContent>
    </Popover>
  )
}
