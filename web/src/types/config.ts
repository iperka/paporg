// Log event type for SSE log streaming
export interface LogEvent {
  timestamp: string
  level: string
  target: string
  message: string
}
