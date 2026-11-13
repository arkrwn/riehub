import { CircleHelp, LayoutDashboard, LogOut, Moon, Sun } from "lucide-react"
import { useEffect, useState } from "react"
import { Navigate, Outlet, Route, Routes } from "react-router"
import { IconButton } from "./components/IconButton"
import { NavPills } from "./components/NavPills"
import { Rail } from "./components/Rail"
import { Card } from "./components/Card"
import { EmptyState } from "./components/EmptyState"
import { AcceptInvite } from "./pages/AcceptInvite"
import { Login } from "./pages/Login"
import { Recover } from "./pages/Recover"
import { Setup } from "./pages/Setup"

/*
  The theme preference is the only thing kept in localStorage. Nothing
  else ever goes there, sessions live in the HttpOnly cookie.
*/
const THEME_KEY = "riehub-theme"

type Theme = "light" | "dark"

const NAV_ITEMS = [
  { id: "overview", label: "Overview" },
  { id: "chat", label: "Chat" },
  { id: "router", label: "Router" },
  { id: "tools", label: "Tools" },
  { id: "automation", label: "Automation" },
  { id: "usage", label: "Usage" },
]

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
  const [active, setActive] = useState("overview")
  return (
    <header className="topbar">
      <LogoPill />
      <NavPills items={NAV_ITEMS} active={active} onSelect={setActive} />
      <span className="user-pill">
        <span className="avatar" aria-hidden="true">
          R
        </span>
        <span>riehub</span>
      </span>
    </header>
  )
}

function SignedInShell({ theme, toggle }: { theme: Theme; toggle: () => void }) {
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
            <IconButton label="Log out">
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
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}
