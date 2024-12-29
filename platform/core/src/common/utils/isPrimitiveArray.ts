import { isPrimitive } from '@/common/utils/isPrimitive'

export const isPrimitiveArray = (value: unknown) => (value as Array<unknown>).every(isPrimitive)
