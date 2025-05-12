import type { DBRecordTarget } from '../sdk'
import type { SDKConfig } from '../sdk/types.js'
import type { PropertyDraft, PropertyType } from '../types/index.js'

import { PROPERTY_TYPES } from '../common/constants.js'
import { isObject, isPropertyValue, isString } from '../common/utils.js'
import { DBRecordInstance } from '../sdk'
import { Transaction } from '../sdk/transaction.js'
import { DEFAULT_BASE_PATH, DEFAULT_HOST, DEFAULT_PORT, DEFAULT_PROTOCOL } from './constants.js'

export const buildTransactionHeader = (txId?: string) =>
  txId ?
    {
      ['x-transaction-id']: txId
    }
  : undefined

export const isTransaction = (input: any): input is Transaction | string =>
  isString(input) || input instanceof Transaction

export const pickTransactionId = (input: any) =>
  isTransaction(input) ?
    input instanceof Transaction ?
      input.id
    : input
  : undefined

export const isUUID = (value: any) => {
  const regex = /^[0-9a-fA-F]{8}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{4}\b-[0-9a-fA-F]{12}$/
  return regex.test(value)
}

export const pickRecordId = (input: DBRecordTarget) => {
  if (isString(input) && isUUID(input)) {
    return input
  } else if (input instanceof DBRecordInstance && input.data) {
    return input.data.__id
  } else if (isObject(input) && '__id' in input && isUUID(input.__id)) {
    return input.__id
  }
  return undefined
}

export const isPropertyDraft = (obj: any): obj is PropertyDraft => {
  if (!isObject(obj)) {
    return false
  }

  if (isObject(obj) && 'name' in obj && !isString(obj.name)) {
    return false
  }

  if (
    isObject(obj) &&
    'type' in obj &&
    (!isString(obj.type) || !PROPERTY_TYPES.includes(obj.type as PropertyType))
  ) {
    return false
  }

  if (!('value' in obj) || !isPropertyValue(obj.value)) {
    return false
  }

  if ('metadata' in obj && !isString(obj.metadata)) {
    return false
  }

  if ('valueSeparator' in obj && !isString(obj.valueSeparator)) {
    return false
  }

  return true
}

export const buildUrl = (props: SDKConfig): string => {
  let protocol = DEFAULT_PROTOCOL
  let host = DEFAULT_HOST
  let port = DEFAULT_PORT
  let basePath = DEFAULT_BASE_PATH

  if ('url' in props) {
    const url = new URL(props.url!)
    protocol = url.protocol.replace(':', '')
    host = url.hostname
    port = parseInt(
      url.port ||
        (protocol === 'http' ? '80'
        : protocol === 'https' ? '443'
        : '')
    )
  }

  if ('host' in props && 'port' in props && 'protocol' in props) {
    protocol = props.protocol!.replace(':', '')
    host = props.host!
    port = props.port!
  }

  // Ensure the basePath starts with a '/'
  if (basePath && !basePath.startsWith('/')) {
    basePath = '/' + basePath
  }

  // If the port is the default for the protocol (80 for http, 443 for https), it can be omitted
  let portString = ''
  if (!((protocol === 'http' && port === 80) || (protocol === 'https' && port === 443))) {
    portString = ':' + port
  }

  return `${protocol}://${host}${portString}${basePath}`
}

export const generateRandomId = (size: number = 8): string => {
  const characters = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  let id = ''

  for (let i = 0; i < size; i++) {
    const randomIndex = Math.floor(Math.random() * characters.length)
    id += characters[randomIndex]
  }

  return id
}
