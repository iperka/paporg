import { useState, useEffect, useCallback } from 'react'
import { CheckCircle, AlertCircle, Loader2, ExternalLink, Copy } from 'lucide-react'
import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
  api,
  type DeviceCodeResponse,
  type TokenStatusResponse,
  type AuthorizationStatusResponse,
} from '@/api'

/** Default expiration time for device code if not specified by server (10 minutes). */
const DEFAULT_EXPIRES_IN_SECONDS = 600
/** Maximum expiration time we'll wait for device code (15 minutes cap). */
const MAX_EXPIRES_IN_SECONDS = 900
/** Default polling interval if not specified by server (5 seconds). */
const DEFAULT_POLL_INTERVAL_SECONDS = 5

interface TokenStatusCardProps {
  sourceName: string
  disabled?: boolean
}

type AuthState = 'idle' | 'loading' | 'pending' | 'success' | 'error'

export function TokenStatusCard({ sourceName, disabled }: TokenStatusCardProps) {
  const [tokenStatus, setTokenStatus] = useState<TokenStatusResponse | null>(null)
  const [authState, setAuthState] = useState<AuthState>('idle')
  const [deviceCode, setDeviceCode] = useState<DeviceCodeResponse | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [isRevoking, setIsRevoking] = useState(false)

  // Fetch initial token status
  const fetchTokenStatus = useCallback(async () => {
    if (!sourceName) return
    setError(null) // Clear any previous error
    try {
      const status = await api.email.getTokenStatus(sourceName)
      setTokenStatus(status)
    } catch (err) {
      // On fetch failure, keep tokenStatus as null to distinguish from "no token"
      // This allows the UI to show "couldn't check" vs "not authorized"
      const errorMessage = err instanceof Error ? err.message : 'Failed to check token status'
      console.error('Failed to fetch token status:', err)
      setError(errorMessage)
      setTokenStatus(null)
      setAuthState('error')
    }
  }, [sourceName])

  useEffect(() => {
    fetchTokenStatus()
  }, [fetchTokenStatus])

  // Poll for authorization completion
  useEffect(() => {
    if (authState !== 'pending' || !deviceCode) return

    // Use AbortController for cleanup
    const abortController = new AbortController()
    let isMounted = true

    // Compute absolute deadline from expiresIn (default 10 min, cap at 15 min)
    const expiresInMs =
      Math.min(deviceCode.expiresIn || DEFAULT_EXPIRES_IN_SECONDS, MAX_EXPIRES_IN_SECONDS) * 1000
    const deadline = Date.now() + expiresInMs

    const poll = async () => {
      while (isMounted && !abortController.signal.aborted) {
        // Check if we've exceeded the deadline
        if (Date.now() >= deadline) {
          if (isMounted) {
            setAuthState('error')
            setError('Authorization timed out. Please try again.')
            setDeviceCode(null)
          }
          break
        }

        // Wait for the specified interval before polling
        const intervalMs = (deviceCode.interval || DEFAULT_POLL_INTERVAL_SECONDS) * 1000
        await new Promise((resolve) => setTimeout(resolve, intervalMs))

        if (!isMounted || abortController.signal.aborted) break

        // Check deadline again after sleep
        if (Date.now() >= deadline) {
          if (isMounted) {
            setAuthState('error')
            setError('Authorization timed out. Please try again.')
            setDeviceCode(null)
          }
          break
        }

        try {
          const status: AuthorizationStatusResponse =
            await api.email.checkStatus(sourceName)

          if (!isMounted) break

          if (status.status === 'authorized') {
            setAuthState('success')
            setDeviceCode(null)
            await fetchTokenStatus()
            break
          } else if (status.status === 'expired' || status.status === 'denied') {
            setAuthState('error')
            setError(status.message)
            setDeviceCode(null)
            break
          } else if (status.status === 'error') {
            setAuthState('error')
            setError(status.message)
            setDeviceCode(null)
            break
          }
          // Otherwise still pending, keep polling
        } catch (err) {
          // Don't stop polling on transient errors
          console.error('Poll error:', err)
        }
      }
    }

    poll()

    return () => {
      isMounted = false
      abortController.abort()
    }
  }, [authState, deviceCode, sourceName, fetchTokenStatus])

  const handleStartAuthorization = async () => {
    setAuthState('loading')
    setError(null)

    try {
      const response = await api.email.startAuthorization(sourceName)
      setDeviceCode(response)
      setAuthState('pending')
    } catch (err) {
      setAuthState('error')
      setError(err instanceof Error ? err.message : 'Failed to start authorization')
    }
  }

  const handleRevoke = async () => {
    setIsRevoking(true)
    try {
      await api.email.revokeToken(sourceName)
      setTokenStatus({ hasToken: false, isValid: false, expiresAt: null, provider: null })
      setAuthState('idle')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to revoke token')
      setAuthState('error')
    } finally {
      setIsRevoking(false)
    }
  }

  const handleCopyCode = async () => {
    if (!deviceCode?.userCode) return

    const text = deviceCode.userCode

    // Try modern clipboard API first
    if (navigator.clipboard?.writeText) {
      try {
        await navigator.clipboard.writeText(text)
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
        return
      } catch (err) {
        console.error('Clipboard API failed, trying fallback:', err)
      }
    }

    // Fallback for older browsers using textarea + execCommand
    try {
      const textarea = document.createElement('textarea')
      textarea.value = text
      textarea.style.position = 'fixed'
      textarea.style.left = '-9999px'
      textarea.style.top = '-9999px'
      document.body.appendChild(textarea)
      textarea.focus()
      textarea.select()
      const success = document.execCommand('copy')
      document.body.removeChild(textarea)
      if (success) {
        setCopied(true)
        setTimeout(() => setCopied(false), 2000)
      } else {
        console.error('execCommand copy failed')
      }
    } catch (err) {
      console.error('Fallback copy failed:', err)
    }
  }

  // Show authorized state
  if (tokenStatus?.hasToken && tokenStatus.isValid && authState !== 'pending') {
    return (
      <Card className="border-green-200 bg-green-50 dark:border-green-900 dark:bg-green-950">
        <CardContent className="flex items-center gap-3 py-4">
          <CheckCircle className="h-5 w-5 text-green-600 dark:text-green-400" />
          <div className="flex-1">
            <p className="font-medium text-green-800 dark:text-green-200">Authorized</p>
            <p className="text-sm text-green-600 dark:text-green-400">
              {tokenStatus.expiresAt
                ? `Token expires: ${new Date(tokenStatus.expiresAt).toLocaleString()}`
                : 'Token is valid'}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={handleRevoke}
            disabled={disabled || isRevoking}
            aria-label="Revoke OAuth token"
            className="border-green-300 text-green-700 hover:bg-green-100 dark:border-green-800 dark:text-green-300 dark:hover:bg-green-900"
          >
            {isRevoking ? 'Revoking...' : 'Revoke'}
          </Button>
        </CardContent>
      </Card>
    )
  }

  // Show pending authorization with device code
  if (authState === 'pending' && deviceCode) {
    return (
      <Card className="border-blue-200 bg-blue-50 dark:border-blue-900 dark:bg-blue-950">
        <CardContent className="py-6 text-center">
          <p className="mb-4 text-sm text-blue-700 dark:text-blue-300">
            Open this link in your browser:
          </p>
          <a
            href={deviceCode.verificationUri}
            target="_blank"
            rel="noopener noreferrer"
            className="mb-4 inline-flex items-center gap-1 text-lg font-mono text-blue-600 underline hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-200"
          >
            {deviceCode.verificationUri}
            <ExternalLink className="h-4 w-4" />
          </a>
          <p className="mt-4 mb-2 text-sm text-blue-700 dark:text-blue-300">
            Then enter this code:
          </p>
          <div className="flex items-center justify-center gap-2">
            <p className="text-3xl font-mono font-bold tracking-wider text-blue-800 dark:text-blue-200">
              {deviceCode.userCode}
            </p>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleCopyCode}
              aria-label={copied ? 'Code copied to clipboard' : 'Copy code to clipboard'}
              className="text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-200"
            >
              {copied ? <CheckCircle className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
            </Button>
          </div>
          <p className="mt-4 flex items-center justify-center gap-2 text-sm text-blue-600 dark:text-blue-400">
            <Loader2 className="h-4 w-4 animate-spin" />
            Waiting for authorization...
          </p>
        </CardContent>
      </Card>
    )
  }

  // Show loading state
  if (authState === 'loading') {
    return (
      <Card>
        <CardContent className="flex items-center justify-center gap-2 py-4">
          <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
          <p className="text-muted-foreground">Starting authorization...</p>
        </CardContent>
      </Card>
    )
  }

  // Show error state
  if (authState === 'error') {
    return (
      <Card className="border-red-200 bg-red-50 dark:border-red-900 dark:bg-red-950">
        <CardContent className="flex items-center gap-3 py-4">
          <AlertCircle className="h-5 w-5 text-red-600 dark:text-red-400" />
          <div className="flex-1">
            <p className="font-medium text-red-800 dark:text-red-200">Authorization Failed</p>
            <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={handleStartAuthorization}
            disabled={disabled}
            className="border-red-300 text-red-700 hover:bg-red-100 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-900"
          >
            Retry
          </Button>
        </CardContent>
      </Card>
    )
  }

  // Show not authorized state (default)
  return (
    <Card className="border-yellow-200 bg-yellow-50 dark:border-yellow-900 dark:bg-yellow-950">
      <CardContent className="flex items-center gap-3 py-4">
        <AlertCircle className="h-5 w-5 text-yellow-600 dark:text-yellow-400" />
        <div className="flex-1">
          <p className="font-medium text-yellow-800 dark:text-yellow-200">Not Authorized</p>
          <p className="text-sm text-yellow-600 dark:text-yellow-400">
            Click to start OAuth2 Device Flow authorization
          </p>
        </div>
        <Button
          onClick={handleStartAuthorization}
          disabled={disabled || !sourceName}
          className="bg-yellow-600 text-white hover:bg-yellow-700 dark:bg-yellow-700 dark:hover:bg-yellow-600"
        >
          Authorize
        </Button>
      </CardContent>
    </Card>
  )
}
