import { useStore } from '@nanostores/react'
import { Folder, FolderPlus, LayoutDashboard } from 'lucide-react'
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
  const triggerLabel = currentProject?.name ?? (loading ? 'Loading...' : 'Projects')

  return (
    <Menu
      trigger={
        <MenuButton className="m-0 grid w-full grid-cols-[minmax(0,1fr)_auto]! justify-items-start px-0 leading-3">
          <Skeleton className="block min-w-0" enabled={loading}>
            <span className="block w-full truncate text-left">{triggerLabel}</span>
          </Skeleton>
        </MenuButton>
      }
      align="start"
    >
      <MenuTitle>Change Project</MenuTitle>

      <MenuItem as="a" href={getRoutePath('projects')} icon={<LayoutDashboard />}>
        All Projects
      </MenuItem>

      <Divider />

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
