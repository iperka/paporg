import { createRouter, createRootRoute, createRoute, redirect } from '@tanstack/react-router'
import { z } from 'zod'
import { RootLayout } from '@/routes/__root'
import { DashboardPage } from '@/routes/dashboard'
import { SettingsPage } from '@/routes/settings'
import { GitSyncPage } from '@/routes/git-sync'
import { SourcesPage } from '@/routes/sources/index'
import { SourceEditPage } from '@/routes/sources/$name'
import { VariablesPage } from '@/routes/variables/index'
import { VariableEditPage } from '@/routes/variables/$name'
import { RulesPage } from '@/routes/rules/index'
import { RuleEditPage } from '@/routes/rules/$name'
import { LogsPage } from '@/routes/logs'
import { HelpPage } from '@/routes/help'
import { JobsPage } from '@/pages/JobsPage'

// Root route
const rootRoute = createRootRoute({
  component: RootLayout,
})

// Index route - redirect to /dashboard
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  beforeLoad: () => {
    throw redirect({ to: '/dashboard' })
  },
})

// Dashboard route
const dashboardRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/dashboard',
  component: DashboardPage,
})

// Settings route
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/settings',
  component: SettingsPage,
})

// Git Sync route
const gitSyncRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/git-sync',
  component: GitSyncPage,
})

// Sources routes
const sourcesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/sources',
  component: SourcesPage,
})

// Search schema for edit routes with optional folder param
const editSearchSchema = z.object({
  folder: z.string().optional(),
})

const sourceEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/sources/$name',
  component: SourceEditPage,
  validateSearch: editSearchSchema,
})

// Variables routes
const variablesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/variables',
  component: VariablesPage,
})

const variableEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/variables/$name',
  component: VariableEditPage,
  validateSearch: editSearchSchema,
})

// Rules routes
const rulesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/rules',
  component: RulesPage,
})

const ruleEditRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/rules/$name',
  component: RuleEditPage,
  validateSearch: editSearchSchema,
})

// Logs route
const logsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/logs',
  component: LogsPage,
})

// Jobs route
const jobsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/jobs',
  component: JobsPage,
})

// Help route
const helpRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/help',
  component: HelpPage,
})

// Route tree
const routeTree = rootRoute.addChildren([
  indexRoute,
  dashboardRoute,
  settingsRoute,
  gitSyncRoute,
  sourcesRoute,
  sourceEditRoute,
  variablesRoute,
  variableEditRoute,
  rulesRoute,
  ruleEditRoute,
  logsRoute,
  jobsRoute,
  helpRoute,
])

// Create router
export const router = createRouter({
  routeTree,
  defaultPreload: 'intent',
})

// Type declaration for type-safe routing
declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}
