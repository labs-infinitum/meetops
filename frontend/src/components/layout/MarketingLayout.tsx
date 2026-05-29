import { Link, NavLink } from "react-router-dom"
import { Button } from "@/components/ui/button"
import { Mic2 } from "lucide-react"

const navLinks = [
  { to: "/#features", label: "Features" },
  { to: "/#how-it-works", label: "How it works" },
  { to: "/#pricing", label: "Pricing" },
]

export function MarketingNav() {
  return (
    <header className="border-border/40 bg-background/80 sticky top-0 z-50 border-b backdrop-blur-md">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-4 sm:px-6">
        <Link to="/" className="flex items-center gap-2.5">
          <div className="bg-primary flex size-8 items-center justify-center rounded-lg">
            <Mic2 className="text-primary-foreground size-4" />
          </div>
          <span className="font-semibold tracking-tight">Notetaker</span>
        </Link>

        <nav className="hidden items-center gap-8 md:flex">
          {navLinks.map((link) => (
            <a
              key={link.to}
              href={link.to}
              className="text-muted-foreground hover:text-foreground text-sm transition-colors"
            >
              {link.label}
            </a>
          ))}
        </nav>

        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" asChild>
            <NavLink to="/login">Log in</NavLink>
          </Button>
          <Button size="sm" asChild>
            <NavLink to="/signup">Get started</NavLink>
          </Button>
        </div>
      </div>
    </header>
  )
}

export function MarketingFooter() {
  return (
    <footer className="border-border/40 border-t">
      <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-4 px-4 py-10 sm:flex-row sm:px-6">
        <div className="flex items-center gap-2">
          <div className="bg-primary flex size-7 items-center justify-center rounded-md">
            <Mic2 className="text-primary-foreground size-3.5" />
          </div>
          <span className="text-sm font-medium">AI Meeting Notetaker</span>
        </div>
        <p className="text-muted-foreground text-sm">
          © {new Date().getFullYear()} Notetaker. All rights reserved.
        </p>
      </div>
    </footer>
  )
}