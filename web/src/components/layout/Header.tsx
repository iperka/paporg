import { useState, useEffect } from 'react'
import { useLocation, Link } from '@tanstack/react-router'
import { ChevronRight, Home } from 'lucide-react'
import { GitProgressIndicator } from '@/components/gitops/GitProgressIndicator'

export function Header() {
  const [isScrolled, setIsScrolled] = useState(false)
  const location = useLocation()

  useEffect(() => {
    const handleScroll = () => {
      setIsScrolled(window.scrollY > 10)
    }
    window.addEventListener('scroll', handleScroll)
    return () => window.removeEventListener('scroll', handleScroll)
  }, [])

  // Generate breadcrumbs from current path
  const pathSegments = location.pathname.split('/').filter(Boolean)
  const breadcrumbs = pathSegments.map((segment, index) => {
    const path = '/' + pathSegments.slice(0, index + 1).join('/')
    const label = segment.charAt(0).toUpperCase() + segment.slice(1).replace(/-/g, ' ')
    return { path, label }
  })

  return (
    <header
      className={`sticky top-0 z-50 flex h-14 shrink-0 items-center gap-2 border-b border-black/5 dark:border-white/10 px-4 transition-colors duration-200 ${
        isScrolled ? 'bg-white/60 dark:bg-neutral-900/60 backdrop-blur-xl' : 'bg-transparent'
      }`}
    >
      {/* Breadcrumbs */}
      <nav className="flex items-center gap-1 text-sm">
        <Link to="/dashboard" className="text-muted-foreground hover:text-foreground transition-colors">
          <Home className="h-4 w-4" />
        </Link>
        {breadcrumbs.map((crumb, index) => (
          <div key={crumb.path} className="flex items-center gap-1">
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
            {index === breadcrumbs.length - 1 ? (
              <span className="font-medium">{crumb.label}</span>
            ) : (
              <Link to={crumb.path} className="text-muted-foreground hover:text-foreground transition-colors">
                {crumb.label}
              </Link>
            )}
          </div>
        ))}
      </nav>

      <div className="flex-1" />

      <div className="flex items-center gap-2 sm:gap-3">
        <GitProgressIndicator />
      </div>
    </header>
  )
}
