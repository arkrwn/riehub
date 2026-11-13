import { useState, type FormEvent } from "react"
import { recover } from "../api/client"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"
import { Toast } from "../components/Toast"

export function Recover() {
  const [email, setEmail] = useState("")
  const [recoveryKey, setRecoveryKey] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [done, setDone] = useState(false)
  const [busy, setBusy] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      await recover(email, recoveryKey, newPassword)
      setDone(true)
    } catch (err) {
      setError(err instanceof Error ? err.message : "recovery failed")
    } finally {
      setBusy(false)
    }
  }

  if (done) {
    return (
      <Card className="auth-card" title="Password updated">
        <Toast message="Your password has been changed" tone="good" />
        <p className="muted small">Sign in with the new password.</p>
      </Card>
    )
  }

  return (
    <Card className="auth-card" title="Recover access">
      <form onSubmit={handleSubmit} className="field auth-fields">
        <Input
          label="Email"
          type="email"
          name="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
        />
        <Input
          label="Recovery key"
          name="recovery_key"
          value={recoveryKey}
          onChange={(e) => setRecoveryKey(e.target.value)}
          required
        />
        <Input
          label="New password"
          type="password"
          name="new_password"
          value={newPassword}
          onChange={(e) => setNewPassword(e.target.value)}
          required
        />
        <Button type="submit" disabled={busy}>
          Set new password
        </Button>
      </form>
      {error ? <ErrorState message={error} onRetry={() => setError(null)} /> : null}
    </Card>
  )
}
