import { Outlet } from '@tanstack/react-router'
import { Toaster } from '@/components/ui/toaster'
import { Header } from '@/components/layout/Header'
import { AppSidebar } from '@/components/layout/AppSidebar'
import { GitOpsProvider } from '@/contexts/GitOpsContext'
import { GitProgressProvider } from '@/contexts/GitProgressContext'
import { JobsProvider } from '@/contexts/JobsContext'
import { SseProvider } from '@/contexts/SseContext'
import { GitInitializeBanner } from '@/components/gitops/GitInitializeBanner'
import { ConflictDialog } from '@/components/gitops/ConflictDialog'
import { useState } from 'react'
import { SidebarProvider, SidebarInset } from '@/components/ui/sidebar'
import { ThemeProvider } from '@/components/theme-provider'
import type { InitializeResult } from '@/types/gitops'

export function RootLayout() {
  const [showConflictDialog, setShowConflictDialog] = useState(false)
  const [conflictResult, setConflictResult] = useState<InitializeResult | null>(null)

  const handleConflicts = (result: InitializeResult) => {
    setConflictResult(result)
    setShowConflictDialog(true)
  }

  return (
    <ThemeProvider defaultTheme="system" storageKey="paporg-theme">
      <SseProvider>
        <GitProgressProvider>
          <JobsProvider>
            <GitOpsProvider>
              <SidebarProvider defaultOpen={true}>
                <AppSidebar />
                <SidebarInset className="bg-white/70 dark:bg-neutral-900/70">
                  <Header />
                  <main className="flex-1 overflow-visible">
                    <div className="container mx-auto py-6 px-4">
                      <GitInitializeBanner onConflicts={handleConflicts} />
                      <Outlet />
                    </div>
                  </main>
                </SidebarInset>

                <ConflictDialog
                  open={showConflictDialog}
                  onOpenChange={setShowConflictDialog}
                  result={conflictResult}
                />
                <Toaster />
              </SidebarProvider>
            </GitOpsProvider>
          </JobsProvider>
        </GitProgressProvider>
      </SseProvider>
    </ThemeProvider>
  )
}
