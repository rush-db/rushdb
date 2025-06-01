import React from 'react'
import type { Step } from 'react-joyride'
import type { TourStepKey } from '~/features/tour/types'
import { getRoutePath } from '~/lib/router'

export const stepDefinitions: Record<TourStepKey, Step> = {
  welcome: {
    target: 'body',
    placement: 'center',
    content: (
      <div className="space-y-4">
        <h2 className="text-content text-2xl font-bold">Welcome to RushDB!</h2>
        <p className="text-content2">
          Congratulations on creating your account and first workspace. Let’s walk through the key features.
        </p>
      </div>
    ),
    data: {
      route: 'home',
      key: 'welcome'
    }
  },
  homeNewProject: {
    target: '[data-tour="new-project-btn"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Create a Project</h3>
        <p className="text-content2 font-bold">Projects help you separate and manage your data.</p>
        <p className="text-content2">
          Each project acts as its own data space — perfect for staging vs. production, or isolated tenants.
          Click to create your first one — we’ll guide you from there. See{' '}
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/get-started/quick-tutorial"
            target="_blank"
            rel="noreferrer"
          >
            our Quick Tutorial{' '}
          </a>
          to get started faster.
        </p>
      </div>
    ),
    data: {
      route: 'home',
      redirectTo: 'newProject',
      key: 'homeNewProject'
    }
  },
  newProjectName: {
    target: '[data-tour="project-name-input"]',
    placement: 'right',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Enter Project Name</h3>
        <p className="text-content2">Give your project a descriptive name so you can find it later.</p>
      </div>
    ),
    data: {
      route: 'newProject',
      key: 'newProjectName'
    }
  },
  newProjectCustomDb: {
    target: '[data-tour="custom-neo4j-container"]',
    placement: 'right',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Connect Custom Neo4j</h3>
        <p className="text-content2">
          On a paid plan you can attach your own Neo4j instance for full data isolation. Learn more on
          <a
            className="text-accent ml-1 underline"
            href={getRoutePath('workspaceBilling')}
            target="_blank"
            rel="noreferrer"
          >
            billing page
          </a>
          .
        </p>
      </div>
    ),
    data: {
      route: 'newProject',
      key: 'newProjectCustomDb'
    }
  },
  newProjectCreate: {
    target: '[data-tour="create-project-btn"]',
    placement: 'right',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Create Your Project</h3>
        <p className="text-content2">Everything’s set—click “Create project” to finish the setup.</p>
      </div>
    ),
    data: {
      route: 'newProject',
      key: 'newProjectCreate',
      noNext: true,
      nextShouldBeManuallySet: true
    }
  },
  projectSdkTokenOverview: {
    target: '[data-tour="project-help-sdk-input"]',
    placement: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Your First SDK Token</h3>
        <p className="text-content2">
          We’ve generated an API key so you can immediately start using our
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/get-started/quick-tutorial/"
            target="_blank"
            rel="noreferrer"
          >
            SDKs
          </a>
          .
        </p>
      </div>
    ),
    data: {
      route: 'projectHelp',
      key: 'projectSdkTokenOverview',
      noBack: true
    }
  },
  projectSdkTokenTabInfo: {
    target: '[data-tour="project-token-chip"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">API Keys Tab</h3>
        <p className="text-content2">Create or revoke API tokens to control secure access to your project.</p>
      </div>
    ),
    data: {
      route: 'projectHelp',
      key: 'projectSdkTokenTabInfo'
    }
  },
  projectImportDataTab: {
    target: '[data-tour="project-import-data-chip"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Import Data Tab</h3>
        <p className="text-content2">
          Upload your JSON or NDJSON — RushDB will handle the rest. We’ll guide you through it shortly.
        </p>
      </div>
    ),
    data: {
      route: 'projectHelp',
      key: 'projectImportDataTab',
      redirectTo: 'projectImportData'
    }
  },
  projectImportRadio: {
    target: '[data-tour="project-import-data-radio"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Choose Your Method</h3>
        <p className="text-content2">
          Pick “Use test dataset” for a quick start or upload your own JSON/NDJSON.
        </p>
      </div>
    ),
    data: {
      route: 'projectImportData',
      key: 'projectImportRadio',
      nextShouldBeManuallySet: true,
      noNext: true
    }
  },
  projectImportOverview: {
    target: '[data-tour="project-import-data-overview"]',
    placement: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Preview Your Data</h3>
        <p className="text-content2">Here’s the JSON you selected. Click “Import data” when you’re ready.</p>
      </div>
    ),
    data: {
      route: 'projectImportData',
      key: 'projectImportOverview',
      noBack: true
    }
  },
  projectImportIngest: {
    target: '[data-tour="project-import-data-ingest"]',
    placement: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Import Your Data</h3>
        <p className="text-content2">
          Upload your JSON — RushDB automatically transforms it into graph records and relationships. Once
          complete, your data appears instantly, ready to query.
        </p>
      </div>
    ),
    data: {
      route: 'projectImportData',
      key: 'projectImportIngest',
      nextShouldBeManuallySet: true,
      noNext: true
    }
  },
  recordTableOverview: {
    target: '[data-tour="records-table-overview"]',
    placementBeacon: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">View Your Records</h3>
        <p className="text-content2">
          Browse your imported data as records in the graph. Click any row to explore its full properties and
          relationships.
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'recordTableOverview'
    }
  },
  recordTableSearchInput: {
    target: '[data-tour="records-table-search-input"]',
    placement: 'right',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Search & Filter</h3>
        <p className="text-content2">
          Use powerful filters to query by properties, relationships, and labels — no schema required. Learn
          more in our
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/concepts/search/introduction"
            target="_blank"
            rel="noreferrer"
          >
            Search Guide
          </a>
          .
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'recordTableSearchInput'
    }
  },
  recordTableViewMode: {
    target: '[data-tour="records-table-view-mode"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Switch View Modes</h3>
        <p className="text-content2">
          Toggle between Table, Graph, and Raw API views — pick the one that fits your workflow best.
        </p>
        <p className="text-content2">
          To explore queries directly, check out our
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/concepts/search/introduction"
            target="_blank"
            rel="noreferrer"
          >
            Search Guide
          </a>
          .
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'recordTableViewMode'
    }
  }
}

export const steps: Step[] = Object.values(stepDefinitions)
export const keys = Object.keys(stepDefinitions) as TourStepKey[]
