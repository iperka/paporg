import api from '@/api'

export type AnalyticsProps = Record<string, string | number | boolean | null | undefined>

export async function trackEvent(name: string, properties?: AnalyticsProps) {
  try {
    await api.analytics.track(name, properties)
  } catch (err) {
    console.warn('Failed to track event', err)
  }
}
