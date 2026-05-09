import { useStore } from '@nanostores/react'
import { Folder, FolderPlus } from 'lucide-react'
import { Fragment } from 'react'

import { Divider } from '~/elements/Divider'
import { Menu, MenuButton, MenuItem, MenuTitle } from '~/elements/Menu'
import { Skeleton } from '~/elements/Skeleton'
import { getRoutePath } from '~/lib/router'

import { useWorkspaceProjectsQuery } from '../../workspaces/hooks/useWorkspaceQueries'
import { useCurrentProjectQuery } from '../hooks/useProjectQueries'

export function ChangeProjectMenu() {
  const { isPending: loading, data: currentProject } = useCurrentProjectQuery()
  const { data: projectsList } = useWorkspaceProjectsQuery()

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

      {projectsList?.map((project: (typeof projectsList)[number], idx: number) => (
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
