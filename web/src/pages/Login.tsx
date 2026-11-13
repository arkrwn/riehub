import { useState, type FormEvent } from "react"
import { login, type LoginResponse } from "../api/client"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"

export function Login() {
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<LoginResponse | null>(null)
  const [busy, setBusy] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      setResult(await login(email, password))
    } catch (err) {
      setError(err instanceof Error ? err.message : "login failed")
    } finally {
      setBusy(false)
    }
  }

  if (result) {
    return (
      <Card className="auth-card" title="Signed in">
        <p className="muted small">Session created for user {result.user_id}</p>
        <p className="muted small tnum">csrf token {result.csrf_token}</p>
      </Card>
    )
  }

  return (
    <Card className="auth-card" title="Sign in">
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
          Sign in
        </Button>
      </form>
      {error ? <ErrorState message={error} onRetry={() => setError(null)} /> : null}
    </Card>
  )
}
