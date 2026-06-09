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
      nextShouldBeManuallySet: true,
      waitForManualAction: true
    }
  },
  projectSdkTokenOverview: {
    target: '[data-tour="project-help-sdk-input"]',
    placement: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Your First SDK Key</h3>
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
        <h3 className="text-content text-lg font-bold">Push Any JSON</h3>
        <p className="text-content2">
          RushDB turns raw JSON into queryable graph records and relationships without requiring a schema
          upfront.
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
    placement: 'bottom',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Preview Your Data</h3>
        <p className="text-content2">
          In this example, the JSON contains agents, evaluation runs, conversations, and tool calls. Import it
          to see how nested data becomes a graph.
        </p>
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
          complete, your data appears instantly, ready to query. Press Import JSON to continue.
        </p>
      </div>
    ),
    data: {
      route: 'projectImportData',
      key: 'projectImportIngest',
      nextShouldBeManuallySet: true,
      noNext: true,
      waitForManualAction: true
    }
  },
  projectIndexSuggestions: {
    target: '[data-tour="project-index-suggestions"]',
    placement: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Suggested Semantic Indexes</h3>
        <p className="text-content2">
          RushDB inspected your imported ontology and found text fields that are useful for semantic search.
        </p>
      </div>
    ),
    data: {
      route: 'projectIndexes',
      key: 'projectIndexSuggestions',
      noBack: true
    }
  },
  projectIndexCreate: {
    target: '[data-tour="project-index-suggested-create"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Create an Index</h3>
        <p className="text-content2">
          Index one long-text field so agents can later retrieve similar responses, decisions, and rationales.
        </p>
      </div>
    ),
    data: {
      route: 'projectIndexes',
      key: 'projectIndexCreate',
      nextShouldBeManuallySet: true,
      noNext: true,
      waitForManualAction: true
    }
  },
  projectRelationshipAnalyze: {
    target: '[data-tour="project-relationships-analyze"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Find Relationship Patterns</h3>
        <p className="text-content2">
          Analyze the ontology to find implicit joins, including runs that share an agentId with agents.
        </p>
      </div>
    ),
    data: {
      route: 'projectRelationships',
      key: 'projectRelationshipAnalyze',
      nextShouldBeManuallySet: true,
      noNext: true
    }
  },
  projectRelationshipApprove: {
    target: '[data-tour="project-relationships-approve"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Approve the Agent Run Pattern</h3>
        <p className="text-content2">
          Approving this materializes the inferred relationship now and keeps applying it to future writes.
        </p>
      </div>
    ),
    data: {
      route: 'projectRelationships',
      key: 'projectRelationshipApprove',
      nextShouldBeManuallySet: true,
      noNext: true,
      waitForManualAction: true
    }
  },
  recordGraphView: {
    target: 'body',
    placement: 'center',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">See the Graph</h3>
        <p className="text-content2">
          The imported structure and approved pattern are now visible together as a graph of agents, runs,
          conversations, decisions, and tools.
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'recordGraphView',
      noBack: true
    }
  },
  recordRawApiMode: {
    target: '[data-tour="records-table-view-mode"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Switch to Raw API</h3>
        <p className="text-content2">
          Open Raw API mode to run a ready-made select query over the imported agent evaluation runs.
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'recordRawApiMode',
      nextShouldBeManuallySet: true,
      noNext: true,
      clickTarget: '[data-tour="records-table-view-mode"] [aria-label="raw-api"]'
    }
  },
  rawApiSelectQuery: {
    target: '[data-tour="raw-api-payload"]',
    placement: 'right',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Select Metrics Query</h3>
        <p className="text-content2">
          This query uses select aggregations to compute run counts, token usage, latency, and evaluation
          scores per agent.
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'rawApiSelectQuery',
      noBack: true
    }
  },
  rawApiRunQuery: {
    target: '[data-tour="raw-api-run-query"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Run the Query</h3>
        <p className="text-content2">
          Execute it to see how RushDB turns graph records into aggregate agent-run metrics.
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'rawApiRunQuery',
      nextShouldBeManuallySet: true,
      noNext: true,
      waitForManualAction: true
    }
  },
  rawApiResults: {
    target: '[data-tour="raw-api-result"]',
    placement: 'left',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Agent Run Stats</h3>
        <p className="text-content2">
          The result gives you immediate operational stats without defining a schema or building a pipeline.
        </p>
      </div>
    ),
    data: {
      route: 'project',
      key: 'rawApiResults',
      redirectTo: 'projectHelp'
    }
  },
  projectGettingStartedFinish: {
    target: '[data-tour="project-getting-started-finish"]',
    placement: 'top',
    content: (
      <div className="space-y-4">
        <h3 className="text-content text-lg font-bold">Connect Your Apps</h3>
        <p className="text-content2">
          You have imported data, indexed text, approved a relationship, visualized the graph, and queried
          metrics. Use Getting Started when you are ready to connect your tools, apps, and workflows.
        </p>
      </div>
    ),
    data: {
      route: 'projectHelp',
      key: 'projectGettingStartedFinish',
      noBack: true
    }
  }
}

export const steps: Step[] = Object.values(stepDefinitions)
export const keys = Object.keys(stepDefinitions) as TourStepKey[]
