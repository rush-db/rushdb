import { useStore } from '@nanostores/react'
import { Folder, FolderPlus } from 'lucide-react'
import { Fragment } from 'react'

import { Divider } from '~/elements/Divider'
import { Menu, MenuButton, MenuItem, MenuTitle } from '~/elements/Menu'
import { Skeleton } from '~/elements/Skeleton'
import { getRoutePath } from '~/lib/router'

import { $workspaceProjects } from '../../workspaces/stores/projects'
import { $currentProject } from '../stores/current-project'

export function ChangeProjectMenu() {
  const { loading, data: currentProject } = useStore($currentProject)
  const { data: projectsList } = useStore($workspaceProjects)

  return (
    <Menu
      trigger={
        <MenuButton>
          <Skeleton enabled={loading}>{currentProject?.name ?? 'Loading...'}</Skeleton>
        </MenuButton>
      }
      align="start"
    >
      <MenuTitle>Change Project</MenuTitle>

      {projectsList?.map((project, idx) => (
        <Fragment key={project.id}>
          {idx !== 0 && <Divider />}
          <MenuItem as="a" href={getRoutePath('project', { id: project.id })} icon={<Folder />}>
            {project.name}
          </MenuItem>
        </Fragment>
      ))}

      <Divider />

      <MenuItem as="a" asChild href={getRoutePath('newProject')} icon={<FolderPlus />} variant="accent">
        New Project
      </MenuItem>
    </Menu>
  )
}
