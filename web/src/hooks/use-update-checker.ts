import { useState, useEffect, useCallback } from 'react'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'

// TODO: The JS check() API does not support custom endpoints at runtime.
// The endpoint is configured in tauri.conf.json (currently points to latest.json).
// To support the pre-release channel, a custom Rust command using UpdaterExt
// should be added that creates an updater with the appropriate endpoint based
// on the release channel setting. The CI workflow should generate:
// - latest.json: updated only on stable (non-prerelease) GitHub releases
// - latest-prerelease.json: updated on every release (including pre-releases)

export interface UpdateInfo {
  available: boolean
  version?: string
  body?: string
  update?: Update
}

type UpdateStatus = 'idle' | 'checking' | 'downloading' | 'error'

export function useUpdateChecker() {
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo>({ available: false })
  const [status, setStatus] = useState<UpdateStatus>('idle')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false

    async function checkForUpdates() {
      try {
        setStatus('checking')

        const update = await check()

        if (cancelled) return

        if (update) {
          setUpdateInfo({
            available: true,
            version: update.version,
            body: update.body,
            update,
          })
        } else {
          setUpdateInfo({ available: false })
        }
        setStatus('idle')
      } catch (err) {
        if (cancelled) return
        // Don't treat update check failures as critical errors — just log them
        console.warn('Update check failed:', err)
        setStatus('idle')
      }
    }

    checkForUpdates()

    return () => {
      cancelled = true
    }
  }, [])

  const downloadAndInstall = useCallback(async () => {
    if (!updateInfo.update) return

    try {
      setStatus('downloading')
      setError(null)
      await updateInfo.update.downloadAndInstall()
      await relaunch()
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to install update')
      setStatus('error')
    }
  }, [updateInfo.update])

  const dismiss = useCallback(() => {
    setUpdateInfo({ available: false })
  }, [])

  return {
    updateInfo,
    status,
    error,
    downloadAndInstall,
    dismiss,
  }
}
