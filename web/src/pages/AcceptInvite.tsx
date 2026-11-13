import { useState, type FormEvent } from "react"
import { Navigate, useParams } from "react-router"
import { acceptInvite } from "../api/client"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"
import { Toast } from "../components/Toast"

export function AcceptInvite() {
  const { token } = useParams()
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [done, setDone] = useState(false)
  const [busy, setBusy] = useState(false)

  // Without a token there is nothing to accept, so go to sign in.
  if (!token) return <Navigate to="/login" replace />

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await acceptInvite(token as string, password)
      setDone(true)
    } catch (err) {
      setError(err instanceof Error ? err.message : "invite could not be accepted")
    } finally {
      setBusy(false)
    }
  }

  if (done) {
    return (
      <Card className="auth-card" title="Invite accepted">
        <Toast message="Your account is ready" tone="good" />
        <p className="muted small">Sign in with the password you just set.</p>
      </Card>
    )
  }

  return (
    <Card className="auth-card" title="Accept invite">
      <form onSubmit={handleSubmit} className="field auth-fields">
        <Input
          label="Password"
          type="password"
          name="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
        />
        <Button type="submit" disabled={busy}>
          Create account
        </Button>
      </form>
      {error ? <ErrorState message={error} onRetry={() => setError(null)} /> : null}
    </Card>
  )
}
