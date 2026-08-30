import { PageContent } from '~/elements/PageHeader'
import { NewImport } from '~/features/imports/components/NewImport'

export function ProjectImportsNew() {
  return (
    <PageContent className="gap-6" contained>
      <NewImport />
    </PageContent>
  )
}
