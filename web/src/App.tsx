import { CircleHelp, LayoutDashboard, LogOut, Moon, Sun } from "lucide-react"
import { useEffect, useState } from "react"
import { Link, Navigate, Outlet, Route, Routes, useNavigate } from "react-router"
import { logout, me } from "./api/client"
import { IconButton } from "./components/IconButton"
import { Rail } from "./components/Rail"
import { Card } from "./components/Card"
import { EmptyState } from "./components/EmptyState"
import { AcceptInvite } from "./pages/AcceptInvite"
import { Admin } from "./pages/Admin"
import { Login } from "./pages/Login"
import { Recover } from "./pages/Recover"
import { Setup } from "./pages/Setup"
import { Security } from "./pages/Security"

/*
  The theme preference is the only thing kept in localStorage. Nothing
  else ever goes there, sessions live in the HttpOnly cookie.
*/
const THEME_KEY = "riehub-theme"

type Theme = "light" | "dark"

function useTheme() {
  const [theme, setTheme] = useState<Theme>(() =>
    localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light",
  )
  useEffect(() => {
    document.documentElement.dataset.theme = theme
    localStorage.setItem(THEME_KEY, theme)
  }, [theme])
  return { theme, toggle: () => setTheme((t) => (t === "dark" ? "light" : "dark")) }
}

function LogoPill() {
  return <span className="logo-pill">riehub</span>
}

function TopBar() {
  return (
    <header className="topbar">
      <LogoPill />
      <nav className="topbar-links" aria-label="Workspace">
        <Link to="/">Overview</Link>
        <Link to="/account/security">Security</Link>
        <Link to="/admin">Administration</Link>
      </nav>
      <span className="user-pill">
        <span className="avatar" aria-hidden="true">
          R
        </span>
        <span>Account</span>
      </span>
    </header>
  )
}

function SignedInShell({ theme, toggle }: { theme: Theme; toggle: () => void }) {
  const navigate = useNavigate()
  const [authenticated, setAuthenticated] = useState<boolean | null>(null)
  useEffect(() => {
    let active = true
    void me().then(() => { if (active) setAuthenticated(true) }).catch(() => { if (active) setAuthenticated(false) })
    return () => { active = false }
  }, [])

  if (authenticated === null) return <div className="auth-shell" role="status">Loading your workspace…</div>
  if (!authenticated) return <Navigate to="/login" replace />

  async function handleLogout() {
    try {
      await logout()
    } finally {
      navigate("/login", { replace: true })
    }
  }

  return (
    <div className="app-shell">
      <Rail
        top={
          <IconButton
            label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
            onClick={toggle}
          >
            {theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
          </IconButton>
        }
        middle={
          <IconButton label="Overview" className="rail-active">
            <LayoutDashboard size={18} />
          </IconButton>
        }
        bottom={
          <>
            <IconButton label="Help">
              <CircleHelp size={18} />
            </IconButton>
            <IconButton label="Log out" onClick={() => void handleLogout()}>
              <LogOut size={18} />
            </IconButton>
          </>
        }
      />
      <div className="app-main">
        <TopBar />
        <main className="content-grid">
          <Outlet />
        </main>
      </div>
    </div>
  )
}

function Home() {
  return (
    <Card className="full-width">
      <h1 className="greeting">Good morning</h1>
      <EmptyState title="Nothing to show yet" hint="Set up connections to see activity here" />
    </Card>
  )
}

function SignedOutShell() {
  return (
    <div className="auth-shell">
      <LogoPill />
      <Outlet />
    </div>
  )
}

export function App() {
  const { theme, toggle } = useTheme()
  return (
    <Routes>
      <Route element={<SignedOutShell />}>
        <Route path="/setup" element={<Setup />} />
        <Route path="/login" element={<Login />} />
        <Route path="/recover" element={<Recover />} />
        <Route path="/invite/:token" element={<AcceptInvite />} />
      </Route>
      <Route element={<SignedInShell theme={theme} toggle={toggle} />}>
        <Route path="/" element={<Home />} />
        <Route path="/account/security" element={<Security />} />
        <Route path="/admin" element={<Admin />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}
