import { useState, type FormEvent } from "react"
import { setup } from "../api/client"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"
import { SecretReveal } from "../components/SecretReveal"

export function Setup() {
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const res = await setup(email, password)
      if (!res.recovery_key) throw new Error("server did not return the recovery key")
      setRecoveryKey(res.recovery_key)
    } catch (err) {
      setError(err instanceof Error ? err.message : "setup failed")
    } finally {
      setBusy(false)
    }
  }

  if (error) {
    return (
      <Card className="auth-card" title="Set up riehub">
        <ErrorState message={error} onRetry={() => setError(null)} />
      </Card>
    )
  }

  return (
    <Card className="auth-card" title="Set up riehub">
      <p className="muted small">Create the first admin account.</p>
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
      <SecretReveal
        open={recoveryKey !== null}
        value={recoveryKey ?? ""}
        title="Recovery key"
        onClose={() => setRecoveryKey(null)}
      />
    </Card>
  )
}
