import { Book, Database, Key, LayoutDashboard, Settings, SettingsIcon, UploadIcon, Users } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'

import type { Project } from '../types'
import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user.ts'
import { useState } from 'react'

export function ProjectTabs({ projectId }: { projectId: Project['id'] }) {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  const [tabs, setTabs] = useState(() => {
    const workspaceTabs = [
      {
        href: getRoutePath('project', { id: projectId }),
        icon: <Database />,
        label: 'Records'
      },

      {
        href: getRoutePath('projectImportData', {
          id: projectId
        }),
        icon: <UploadIcon />,
        label: 'Import Data'
      },
      {
        href: getRoutePath('projectTokens', {
          id: projectId
        }),
        icon: <Key />,
        label: 'API Keys'
      }
    ]

    if (isOwner) {
      workspaceTabs.push({
        href: getRoutePath('projectSettings', {
          id: projectId
        }),
        icon: <Settings />,
        label: 'Settings'
      })
    }

    workspaceTabs.push({
      href: getRoutePath('projectHelp', { id: projectId }),
      icon: <Book />,
      label: 'Help'
    })

    return workspaceTabs
  })

  return (
    <PageTabs>
      {tabs.map((item) => (
        <PageTab {...item} key={item.href} />
      ))}
    </PageTabs>
  )
}
