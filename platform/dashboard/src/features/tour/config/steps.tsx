import type { Step } from 'react-joyride'
import { TourStepKey } from '~/features/tour/stores/tour'
import { getRoutePath } from '~/lib/router.ts'

export const stepDefinitions: Record<TourStepKey, Step> = {
  welcome: {
    target: 'body',
    placement: 'center',
    content: (
      <div className="space-y-4 text-center">
        <h2 className="text-content text-2xl font-bold">Welcome to RushDB!</h2>
        <p className="text-content2">
          Congratulations on creating your account and first workspace. Let’s walk through the key features.
        </p>
      </div>
    ),
    data: { route: 'home', key: 'welcome' }
  },
  homeNewProject: {
    target: '[data-tour="new-project-btn"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Create a Project</h3>
        <p className="text-content2">
          Projects let you organize your data. Click here to get started, then we’ll guide you to
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/"
            target="_blank"
            rel="noreferrer"
          >
            our Quick Tutorial
          </a>{' '}
          .
        </p>
      </div>
    ),
    data: { route: 'home', redirectTo: 'newProject', key: 'homeNewProject' }
  },
  newProjectName: {
    target: '[data-tour="project-name-input"]',
    placement: 'right',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Enter Project Name</h3>
        <p className="text-content2">Give your project a descriptive name so you can find it later.</p>
      </div>
    ),
    data: { route: 'newProject', key: 'newProjectName' }
  },
  newProjectCustomDb: {
    target: '[data-tour="custom-neo4j-container"]',
    placement: 'right',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Connect Custom Neo4j</h3>
        <p className="text-content2">
          On a paid plan you can attach your own Neo4j instance for full data isolation. Learn more on
          <a
            className="text-accent ml-1 underline"
            href={getRoutePath('workspaceBilling')}
            target="_blank"
            rel="noreferrer"
          >
            billing page
          </a>{' '}
          .
        </p>
      </div>
    ),
    data: { route: 'newProject', key: 'newProjectCustomDb' }
  },
  newProjectCreate: {
    target: '[data-tour="create-project-btn"]',
    placement: 'right',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Create Your Project</h3>
        <p className="text-content2">Everything’s set—click “Create project” to finish the setup.</p>
      </div>
    ),
    data: {
      route: 'newProject',
      key: 'newProjectCreate',
      nextShouldBeManuallySet: true,
      noNext: true
    }
  },
  projectSdkTokenOverview: {
    target: '[data-tour="project-help-sdk-input"]',
    placement: 'top',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Your First SDK Token</h3>
        <p className="text-content2">
          We’ve generated an API key so you can immediately start using our
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/get-started/quick-tutorial/"
            target="_blank"
            rel="noreferrer"
          >
            SDKs
          </a>{' '}
          .
        </p>
      </div>
    ),
    data: { route: 'projectHelp', key: 'projectSdkTokenOverview', noBack: true }
  },
  projectSdkTokenTabInfo: {
    target: '[data-tour="project-token-chip"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">API Keys Tab</h3>
        <p className="text-content2">Here you can create, rotate, and revoke tokens at any time.</p>
      </div>
    ),
    data: { route: 'projectHelp', key: 'projectSdkTokenTabInfo' }
  },
  projectImportDataTab: {
    target: '[data-tour="project-import-data-chip"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Import Data Tab</h3>
        <p className="text-content2">Load JSON/NDJSON data into RushDB. We’ll show you how in a moment.</p>
      </div>
    ),
    data: { route: 'projectHelp', key: 'projectImportDataTab', redirectTo: 'projectImportData' }
  },
  projectImportRadio: {
    target: '[data-tour="project-import-data-radio"]',
    placement: 'bottom',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Choose Your Method</h3>
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
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Preview Your Data</h3>
        <p className="text-content2">Here’s the JSON you selected. Click “Ingest data” when you’re ready.</p>
      </div>
    ),
    data: { route: 'projectImportData', key: 'projectImportOverview', noBack: true }
  },
  projectImportIngest: {
    target: '[data-tour="project-import-data-ingest"]',
    placement: 'top',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Ingest Your Data</h3>
        <p className="text-content2">
          RushDB will parse and store your JSON. Once it finishes, you’ll see your records in real time.
        </p>
      </div>
    ),
    data: {
      route: 'projectImportData',
      key: 'projectImportIngest',
      nextShouldBeManuallySet: true,
      noBack: true,
      noNext: true
    }
  },
  recordTableOverview: {
    target: '[data-tour="records-table-overview"]',
    placement: 'top',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">View Your Records</h3>
        <p className="text-content2">
          All your ingested data appears here. Click a row to inspect full properties.
        </p>
      </div>
    ),
    data: { route: 'project', key: 'recordTableOverview' }
  },
  recordTableSearchInput: {
    target: '[data-tour="records-table-search-input"]',
    placement: 'right',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Search & Filter</h3>
        <p className="text-content2">
          Use filters to quickly find the data you need. See our
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/concepts/search/introduction"
            target="_blank"
            rel="noreferrer"
          >
            Search Guide
          </a>{' '}
          .
        </p>
      </div>
    ),
    data: { route: 'project', key: 'recordTableSearchInput' }
  },
  recordTableViewMode: {
    target: '[data-tour="records-table-view-mode"]',
    placement: 'left',
    content: (
      <div className="space-y-4 text-center">
        <h3 className="text-content text-xl font-bold">Switch View Modes</h3>
        <p className="text-content2">Table, Graph or Raw API view—choose whatever fits your workflow.</p>
        <p className="text-content2">
          Read our
          <a
            className="text-accent ml-1 underline"
            href="https://docs.rushdb.com/concepts/search/introduction"
            target="_blank"
            rel="noreferrer"
          >
            Search Guide
          </a>{' '}
          to work with Raw API view
        </p>
      </div>
    ),
    data: { route: 'project', key: 'recordTableViewMode' }
  }
}

export const steps: Step[] = Object.values(stepDefinitions)
export const keys = Object.keys(stepDefinitions) as TourStepKey[]
