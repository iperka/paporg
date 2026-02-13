import { useRef, useEffect, useCallback } from 'react'

/**
 * Hook that returns a function to check if the component is still mounted.
 * Useful for preventing state updates on unmounted components.
 */
export function useIsMounted() {
  const mountedRef = useRef(false)

  useEffect(() => {
    mountedRef.current = true
    return () => {
      mountedRef.current = false
    }
  }, [])

  return useCallback(() => mountedRef.current, [])
}
