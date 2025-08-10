import { Book, Database, Key, Settings, UploadIcon, Wallet2 } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'

import type { Project } from '../types'
import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user.ts'
import { useState } from 'react'
import { $platformSettings } from '~/features/auth/stores/settings.ts'

export function ProjectTabs({ project }: { project: Project }) {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  const { data: platformSettings } = useStore($platformSettings)

  const [tabs, setTabs] = useState(() => {
    const projectsTabs = [
      {
        href: getRoutePath('project', { id: project.id }),
        icon: <Database />,
        label: 'Records'
      },

      {
        href: getRoutePath('projectImportData', {
          id: project.id
        }),
        icon: <UploadIcon />,
        label: 'Import Data',
        dataTour: 'project-import-data-chip'
      },
      {
        href: getRoutePath('projectTokens', {
          id: project.id
        }),
        icon: <Key />,
        label: 'API Keys',
        dataTour: 'project-token-chip'
      }
    ]

    if (isOwner) {
      projectsTabs.push({
        href: getRoutePath('projectSettings', {
          id: project.id
        }),
        icon: <Settings />,
        label: 'Settings'
      })
    }

    projectsTabs.push({
      href: getRoutePath('projectHelp', { id: project.id }),
      icon: <Book />,
      label: 'Help'
    })

    if (!platformSettings?.selfHosted && isOwner && project.managedDb) {
      projectsTabs.push({
        href: getRoutePath('projectBilling', { id: project.id }),
        icon: <Wallet2 />,
        label: 'Subscription'
      })
    }

    return projectsTabs
  })

  return (
    <PageTabs>
      {tabs.map(({ href, icon, label, dataTour }) => (
        <PageTab key={href} href={href} icon={icon} label={label} dataTour={dataTour} />
      ))}
    </PageTabs>
  )
}
