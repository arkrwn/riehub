import { useState, type FormEvent } from "react"
import { useNavigate } from "react-router"
import { login } from "../api/client"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"

export function Login() {
  const navigate = useNavigate()
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const result = await login(email, password)
      if (!result.user_id) throw new Error("login response did not include the user")
      navigate("/", { replace: true })
    } catch (err) {
      setError(err instanceof Error ? err.message : "login failed")
    } finally {
      setPassword("")
      setBusy(false)
    }
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
