import { PageModel, PostModel } from '~/models'

export type Post = (typeof PostModel)['recordInstance']
export type Page = (typeof PageModel)['recordInstance']
