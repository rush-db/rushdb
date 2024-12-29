import { useStore } from '@nanostores/react'
import { Building2 } from 'lucide-react'
import { Fragment } from 'react'

import { Divider } from '~/elements/Divider'
import { Menu, MenuButton, MenuItem, MenuTitle } from '~/elements/Menu'
import { Skeleton } from '~/elements/Skeleton'

import {
  $currentWorkspace,
  setCurrentWorkspace
} from '../stores/current-workspace'
import { $workspacesList } from '../stores/workspaces'

export function ChangeWorkspaceMenu() {
  const { data: currentWorkspace, error, loading } = useStore($currentWorkspace)
  const { data: list } = useStore($workspacesList)

  const showNew = !currentWorkspace && !error && !loading

  const triggerText = showNew
    ? 'Create new'
    : currentWorkspace?.name ?? 'Loading...'

  // TODO:
  // const pastOrgsLimit = false

  return (
    <Menu
      trigger={
        <MenuButton>
          <Skeleton enabled={loading && !currentWorkspace}>
            {triggerText}
          </Skeleton>
        </MenuButton>
      }
      align="start"
    >
      <MenuTitle>Workspaces</MenuTitle>

      {list?.map((workspace, idx) => (
        <Fragment key={workspace.id}>
          {idx !== 0 && <Divider />}
          <MenuItem
            icon={<Building2 />}
            onClick={() => setCurrentWorkspace(workspace.id)}
          >
            {workspace.name}
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
