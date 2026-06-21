import { Book, Database, Key, Search, Settings, UploadIcon, Waypoints } from 'lucide-react'

import { LabelColorIcon } from '~/features/labels/components/LabelColorIcon'
import { useProjectLabelsQuery } from '~/features/projects/hooks/useProjectQueries'
import {
  $activeLabels,
  $currentProjectRecordsSkip,
  $recordView
} from '~/features/projects/stores/current-project'
import { PageTab, PageTabs } from '~/layout/RootLayout/PageTabs'
import { getRoutePath, $router } from '~/lib/router'
import { numberCompact } from '~/lib/formatters'
import { cn } from '~/lib/utils'

import type { Project } from '../types'
import { useStore } from '@nanostores/react'
import { $user } from '~/features/auth/stores/user.ts'
import type { JSX, ReactNode } from 'react'
import { useMemo } from 'react'
import { usePlatformSettings } from '~/features/auth/hooks/useAuthQueries'

type ProjectTab = {
  href: ReturnType<typeof getRoutePath>
  icon: JSX.Element
  label: string
  dataTour?: string
}

export function ProjectTabs({
  collapsed = false,
  project,
  variant = 'top'
}: {
  collapsed?: boolean
  project: Project
  variant?: 'top' | 'sidebar'
}) {
  const currentUser = useStore($user)
  const page = useStore($router)
  const activeLabels = useStore($activeLabels)
  const isOwner = currentUser.currentScope?.role === 'owner'
  const sidebar = variant === 'sidebar'

  const { data: platformSettings } = usePlatformSettings()

  const projectIsInactive = project.status === 'pending' || project.status === 'provisioning'
  const recordsRouteActive = page?.route === 'project' && page.params.id === project.id
  const showRecordLabels = sidebar && !collapsed && !projectIsInactive && recordsRouteActive
  const { data: labels } = useProjectLabelsQuery({ enabled: showRecordLabels })
  const labelEntries = useMemo(() => Object.entries(labels ?? {}), [labels])

  const tabs = useMemo<ProjectTab[]>(() => {
    const projectsTabs: ProjectTab[] =
      projectIsInactive ?
        []
      : [
          {
            href: getRoutePath('project', { id: project.id }),
            icon: <Database />,
            label: 'Records'
          },
          {
            href: getRoutePath('projectRelationships', { id: project.id }),
            icon: <Waypoints />,
            label: 'Relationships'
          }
        ]

    if (platformSettings?.embeddingEnabled && !projectIsInactive) {
      projectsTabs.push({
        href: getRoutePath('projectIndexes', { id: project.id }),
        icon: <Search />,
        label: 'Indexes'
      })
    }

    projectsTabs.push(
      {
        href: getRoutePath('projectImportData', { id: project.id }),
        icon: <UploadIcon />,
        label: 'Import Data',
        dataTour: 'project-import-data-chip'
      },
      {
        href: getRoutePath('projectTokens', { id: project.id }),
        icon: <Key />,
        label: 'API Keys',
        dataTour: 'project-token-chip'
      }
    )

    if (isOwner) {
      projectsTabs.push({
        href: getRoutePath('projectSettings', { id: project.id }),
        icon: <Settings />,
        label: 'Settings'
      })
    }

    return projectsTabs
  }, [project, projectIsInactive, isOwner, platformSettings])

  // Getting Started is pinned to the bottom of the sidebar, separate from the
  // main tab group.
  const helpTab: ProjectTab | null =
    projectIsInactive ? null : (
      {
        href: getRoutePath('projectHelp', { id: project.id }),
        icon: <Book />,
        label: 'Getting Started'
      }
    )

  const renderTab = ({ href, icon, label, dataTour }: ProjectTab): ReactNode[] => {
    const recordsTab = label === 'Records'
    const importTab = label === 'Import Data'

    const nodes: ReactNode[] = [
      <PageTab
        key={`${href}:tab`}
        href={href}
        icon={icon}
        label={label}
        dataTour={dataTour}
        onClick={
          recordsTab ?
            () => {
              // Clicking "Records" clears any label drill-down so the full record set shows.
              $activeLabels.set([])
              $currentProjectRecordsSkip.set(0)
            }
          : undefined
        }
      />
    ]

    if (recordsTab && showRecordLabels && labelEntries.length > 0) {
      nodes.push(
        <div
          className="border-stroke mb-1 ml-[1.2rem] mt-0.5 flex flex-col gap-0.5 border-l pl-2"
          key={`${href}:labels`}
        >
          {labelEntries.map(([recordLabel, quantity], idx) => {
            const active = activeLabels.includes(recordLabel)

            return (
              <a
                className={cn(
                  'text-content2 hover:bg-fill3 hover:text-content flex h-7 min-w-0 items-center gap-2 rounded px-2 text-sm transition',
                  active && 'bg-secondary text-content'
                )}
                href={getRoutePath('project', { id: project.id })}
                key={recordLabel}
                onClick={() => {
                  $activeLabels.set([recordLabel])
                  $currentProjectRecordsSkip.set(0)
                  $recordView.set('table')
                }}
              >
                <LabelColorIcon idx={idx} label={recordLabel} />
                <span className="min-w-0 flex-1 truncate">{recordLabel}</span>
                <span className="text-content3 font-mono text-xs tabular-nums">
                  {numberCompact.format(quantity)}
                </span>
              </a>
            )
          })}
        </div>
      )
    }

    // Separate the data tabs (Records … Import Data) from config (API Keys, Settings).
    if (sidebar && importTab) {
      nodes.push(<div aria-hidden className="bg-stroke my-1 h-px w-full" key={`${href}:divider`} />)
    }

    return nodes
  }

  if (sidebar) {
    return (
      <div className="flex min-h-0 flex-1 flex-col gap-2">
        <PageTabs collapsed={collapsed} variant="sidebar">
          {tabs.flatMap(renderTab)}
        </PageTabs>
        {helpTab && (
          <PageTabs className="mt-auto" collapsed={collapsed} variant="sidebar">
            <PageTab href={helpTab.href} icon={helpTab.icon} label={helpTab.label} />
          </PageTabs>
        )}
      </div>
    )
  }

  return (
    <PageTabs collapsed={collapsed} variant={variant}>
      {[...tabs, ...(helpTab ? [helpTab] : [])].flatMap(renderTab)}
    </PageTabs>
  )
}
