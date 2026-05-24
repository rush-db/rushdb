import { Book, Database, Key, Search, Settings, UploadIcon, Waypoints } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'

import type { Project } from '../types'
import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user.ts'
import type { JSX } from 'react'
import { useMemo } from 'react'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'

export function ProjectTabs({ project }: { project: Project }) {
  const currentUser = useStore($user)
  const isOwner = currentUser.currentScope?.role === 'owner'

  const { data: platformSettings } = usePlatformSettings()

  const projectIsInactive = project.status === 'pending' || project.status === 'provisioning'

  const tabs = useMemo(() => {
    const projectsTabs: {
      href: ReturnType<typeof getRoutePath>
      icon: JSX.Element
      label: string
      dataTour?: string
    }[] =
      projectIsInactive ?
        []
      : [
          {
            href: getRoutePath('project', { id: project.id }),
            icon: <Database />,
            label: 'Records'
          },
          {
            href: getRoutePath('projectRelationships', {
              id: project.id
            }),
            icon: <Waypoints />,
            label: 'Relationships'
          }
        ]

    if (platformSettings?.embeddingEnabled && !projectIsInactive) {
      projectsTabs.push({
        href: getRoutePath('projectIndexes', {
          id: project.id
        }),
        icon: <Search />,
        label: 'Indexes'
      })
    }

    projectsTabs.push(
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
    )

    if (isOwner) {
      projectsTabs.push({
        href: getRoutePath('projectSettings', {
          id: project.id
        }),
        icon: <Settings />,
        label: 'Settings'
      })
    }

    if (!projectIsInactive) {
      projectsTabs.push({
        href: getRoutePath('projectHelp', { id: project.id }),
        icon: <Book />,
        label: 'Help'
      })
    }

    return projectsTabs
  }, [project, projectIsInactive, isOwner, platformSettings])

  return (
    <PageTabs>
      {tabs.map(({ href, icon, label, dataTour }) => (
        <PageTab key={href} href={href} icon={icon} label={label} dataTour={dataTour} />
      ))}
    </PageTabs>
  )
}
