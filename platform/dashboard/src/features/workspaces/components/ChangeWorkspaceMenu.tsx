import { Building2, Plus } from 'lucide-react'
import { Fragment } from 'react'

import { Divider } from '~/elements/Divider'
import { Menu, MenuButton, MenuItem, MenuTitle } from '~/elements/Menu'
import { Skeleton } from '~/elements/Skeleton'

import { setCurrentWorkspace } from '../stores/current-workspace'
import { useCurrentWorkspaceQuery, useWorkspacesQuery } from '../hooks/useWorkspaceQueries'
import { Label } from '~/elements/Label.tsx'
import { getRoutePath } from '~/lib/router'

export function ChangeWorkspaceMenu() {
  const { data: currentWorkspace, isPending: loading } = useCurrentWorkspaceQuery()
  const { data: list } = useWorkspacesQuery()

  const triggerText = currentWorkspace?.name ?? 'Loading...'

  return (
    <Menu
      trigger={
        <MenuButton>
          <Skeleton enabled={loading && !currentWorkspace}>
            <div className="flex items-center justify-between gap-5">
              <div className="flex flex-col text-left">
                <strong>{triggerText}</strong>
                <p className="text-accent text-xs">
                  {list?.find((workspace) => workspace.id === currentWorkspace?.id)?.role}
                </p>
              </div>
            </div>
          </Skeleton>
        </MenuButton>
      }
      align="start"
    >
      <MenuTitle>Workspaces</MenuTitle>

      {list?.map((workspace, idx) => (
        <Fragment key={workspace.id}>
          {idx !== 0 && <Divider />}
          <MenuItem icon={<Building2 />} onClick={() => setCurrentWorkspace(workspace.id)}>
            <div className="flex items-center justify-between gap-5">
              <div className="flex flex-col text-left">
                <strong>{workspace.name}</strong>
                <p className="text-accent text-xs">{workspace.role}</p>
              </div>
              {workspace.id === currentWorkspace?.id && <Label>current</Label>}
            </div>
          </MenuItem>
        </Fragment>
      ))}

      <Divider />

      <MenuItem as={'a'} asChild href={getRoutePath('newWorkspace')} icon={<Plus />} variant="accent">
        New Workspace
      </MenuItem>
    </Menu>
  )
}
