import { MoreVertical, Trash2 } from 'lucide-react'

import type { Project } from '~/features/projects/types'

import { Button } from '~/elements/Button'
import { Menu, MenuItem } from '~/elements/Menu'

import { DeleteProjectDialog } from './DeleteProjectDialog'

type Props = {
  projectId: Project['id'] | undefined
} & TPolymorphicComponentProps<'button'>

export function ProjectMenu({ projectId, ...props }: Props) {
  if (!projectId) {
    return null
  }

  return (
    <Menu
      trigger={
        <Button aria-label="Project menu" {...props}>
          <MoreVertical />
        </Button>
      }
    >
      <DeleteProjectDialog
        trigger={
          <MenuItem
            className="text-accent-red"
            dropdown
            icon={<Trash2 size={20} />}
          >
            Delete
          </MenuItem>
        }
        projectId={projectId}
      />
    </Menu>
  )
}
