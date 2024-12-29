import { CreateEntityDto } from '@/core/entity/dto/create-entity.dto'

export type TImportRecordsRelation = {
  from: string
  to: string
}
export type TImportJsonPayload = Record<string, any> | Array<Record<string, any>>
export type TImportOptions = {
  suggestTypes?: boolean
  returnResult?: boolean
}
export type WithId<T> = T & {
  id: string
}

export type TImportQueue = {
  key: string
  value: TImportJsonPayload
  parentId?: string
  target: WithId<CreateEntityDto>
  skip?: boolean
} & TImportOptions
