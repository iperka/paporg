import { Link, useLocation } from '@tanstack/react-router'
import { Briefcase, FileText, FolderInput, GitBranch, HelpCircle, LayoutDashboard, ScrollText, Settings, Variable, AlertTriangle } from 'lucide-react'
import { ModeToggle } from '@/components/mode-toggle'
import { useGitStatus } from '@/queries/use-git-status'
import { Badge } from '@/components/ui/badge'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
  SidebarTrigger,
} from '@/components/ui/sidebar'

export function AppSidebar() {
  const location = useLocation()
  const { data: gitStatus } = useGitStatus()

  // Use boundary-aware matching to prevent partial path matches (e.g., /jobs matching /jobs-other)
  const isActive = (path: string) => {
    if (location.pathname === path) return true
    // Ensure we match at path boundaries only
    return location.pathname.startsWith(path + '/')
  }

  // Calculate git sync status
  const isGitRepo = Boolean(gitStatus?.isRepo)
  const changeCount = gitStatus?.files?.length || 0
  const hasConflicts = gitStatus?.files?.some(f => f.status === 'U' || f.status === 'UU') || false

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem className="flex items-center gap-2">
            <SidebarMenuButton size="lg" asChild className="flex-1">
              <Link to="/jobs">
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-primary text-primary-foreground">
                  <FileText className="size-4" />
                </div>
                <div className="flex flex-col gap-0.5 leading-none">
                  <span className="font-semibold">Paporg</span>
                  <span className="text-xs text-muted-foreground">Document Manager</span>
                </div>
              </Link>
            </SidebarMenuButton>
            <SidebarTrigger className="shrink-0" />
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={isActive('/dashboard')} tooltip="Dashboard">
                  <Link to="/dashboard">
                    <LayoutDashboard />
                    <span>Dashboard</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={isActive('/jobs')} tooltip="Jobs">
                  <Link to="/jobs">
                    <Briefcase />
                    <span>Jobs</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={isActive('/sources')} tooltip="Sources">
                  <Link to="/sources">
                    <FolderInput />
                    <span>Sources</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={isActive('/variables')} tooltip="Variables">
                  <Link to="/variables">
                    <Variable />
                    <span>Variables</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={isActive('/rules')} tooltip="Rules">
                  <Link to="/rules">
                    <FileText />
                    <span>Rules</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>

              <SidebarMenuItem>
                <SidebarMenuButton asChild isActive={isActive('/git-sync')} tooltip="Git Sync">
                  <Link to="/git-sync" className="relative">
                    {hasConflicts ? (
                      <AlertTriangle className="text-destructive" />
                    ) : (
                      <GitBranch />
                    )}
                    <span>Git Sync</span>
                    {isGitRepo && changeCount > 0 && !hasConflicts && (
                      <Badge
                        className="ml-auto h-5 min-w-5 px-1.5 text-xs font-semibold bg-amber-500 text-white hover:bg-amber-600"
                      >
                        {changeCount}
                      </Badge>
                    )}
                    {hasConflicts && (
                      <Badge
                        variant="destructive"
                        className="ml-auto h-5 px-1.5 text-xs font-semibold"
                      >
                        Conflict
                      </Badge>
                    )}
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter>

        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton asChild isActive={isActive('/settings')} tooltip="Settings">
              <Link to="/settings">
                <Settings />
                <span>Settings</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton asChild isActive={isActive('/logs')} tooltip="Logs">
              <Link to="/logs">
                <ScrollText />
                <span>Logs</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton asChild isActive={isActive('/help')} tooltip="Help">
              <Link to="/help">
                <HelpCircle />
                <span>Help</span>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <ModeToggle />
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>
  )
}
