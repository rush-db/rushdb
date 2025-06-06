import { SearchDto } from '@/core/search/dto/search.dto'

export const singleLabelPart = (labels?: SearchDto['labels']) =>
  labels && labels.length === 1 ? `:\`${labels?.[0]}\`` : ''
