import { Book, Database, Key, Settings, UploadIcon } from 'lucide-react'

import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath } from '~/lib/router'

import type { Project } from '../types'

const getTabs = (projectId: Project['id']) => {
  return [
    // {
    //   href: getRoutePath('project', { id: projectId }),
    //   icon: <LayoutDashboard />,
    //   label: 'Overview'
    // },
    {
      href: getRoutePath('project', { id: projectId }),
      icon: <Database />,
      label: 'Records'
    },
    // {
    //   label: 'Users',
    //   icon: <Users />,
    //   href: getRoutePath('projectUsers', { id: projectId })
    // }
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
    },
    {
      href: getRoutePath('projectSettings', {
        id: projectId
      }),
      icon: <Settings />,
      label: 'Settings'
    },
    {
      href: getRoutePath('projectHelp', { id: projectId }),
      icon: <Book />,
      label: 'Help'
    }
  ]
}

export function ProjectTabs({ projectId }: { projectId: Project['id'] }) {
  const tabs = getTabs(projectId)

  return (
    <PageTabs>
      {tabs.map((item) => (
        <PageTab {...item} key={item.href} />
      ))}
    </PageTabs>
  )
}
