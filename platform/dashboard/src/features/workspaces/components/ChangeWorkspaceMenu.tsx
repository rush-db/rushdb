import { useStore } from '@nanostores/react'
import { Building2 } from 'lucide-react'
import { Fragment } from 'react'

import { Divider } from '~/elements/Divider'
import { Menu, MenuButton, MenuItem, MenuTitle } from '~/elements/Menu'
import { Skeleton } from '~/elements/Skeleton'

import { $currentWorkspace, setCurrentWorkspace } from '../stores/current-workspace'
import { $workspacesList } from '../stores/workspaces'
import { Label } from '~/elements/Label.tsx'

export function ChangeWorkspaceMenu() {
  const { data: currentWorkspace, error, loading } = useStore($currentWorkspace)
  const { data: list } = useStore($workspacesList)

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

      {/*{!pastOrgsLimit && (*/}
      {/*  <MenuItem*/}
      {/*    as={'a'}*/}
      {/*    asChild*/}
      {/*    href={getRoutePath('newWorkspace')}*/}
      {/*    icon={<Plus />}*/}
      {/*    variant="accent"*/}
      {/*  >*/}
      {/*    New Workspace*/}
      {/*  </MenuItem>*/}
      {/*)}*/}
    </Menu>
  )
}
