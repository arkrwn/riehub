import { useCallback, useEffect, useState, type FormEvent } from "react"
import { useNavigate } from "react-router"
import { changePassword, createHubKey, endSession, listHubKeys, listSessions, me, revokeHubKey, rotateRecoveryKey, type HubKeyInfo, type SessionInfo } from "../api/client"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"
import { SecretReveal } from "../components/SecretReveal"

function timestamp(value: number) {
  return new Date(value).toLocaleString()
}

export function Security() {
  const navigate = useNavigate()
  const [sessions, setSessions] = useState<SessionInfo[]>([])
  const [currentSid, setCurrentSid] = useState<string | null>(null)
  const [keys, setKeys] = useState<HubKeyInfo[]>([])
  const [label, setLabel] = useState("")
  const [currentPassword, setCurrentPassword] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [secret, setSecret] = useState<{ title: string; value: string } | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  const refresh = useCallback(async () => {
    const [identity, sessionItems, keyItems] = await Promise.all([me(), listSessions(), listHubKeys()])
    setCurrentSid(identity.session_id?.replace(/^sess_/, "") ?? null)
    setSessions(sessionItems)
    setKeys(keyItems)
  }, [])

  useEffect(() => {
    void refresh().catch((err: unknown) => setError(err instanceof Error ? err.message : "Could not load account security"))
  }, [refresh])

  async function perform(action: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    try {
      await action()
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : "Request failed")
    } finally {
      setBusy(false)
    }
  }

  async function submitPassword(event: FormEvent) {
    event.preventDefault()
    await perform(async () => {
      await changePassword(currentPassword, newPassword)
      setCurrentPassword("")
      setNewPassword("")
    })
  }

  async function submitKey(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const result = await createHubKey(label)
      setLabel("")
      await refresh()
      if (result.token) setSecret({ title: "Hub key", value: result.token })
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create key")
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="account-page">
      <header>
        <p className="eyebrow">Account</p>
        <h1 className="section-title">Security</h1>
        <p className="muted">Manage your password, recovery key, hub keys and active sessions.</p>
      </header>
      {error ? <ErrorState message={error} onRetry={() => { setError(null); void refresh() }} /> : null}
      <Card title="Password">
        <form className="account-form" onSubmit={(event) => void submitPassword(event)}>
          <Input label="Current password" type="password" autoComplete="current-password" value={currentPassword} onChange={(event) => setCurrentPassword(event.target.value)} required />
          <Input label="New password" type="password" autoComplete="new-password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} minLength={12} required />
          <Button type="submit" disabled={busy}>Change password</Button>
        </form>
      </Card>
      <Card title="Recovery key">
        <p className="muted">Rotate your recovery key if it may have been exposed. The new key is shown once.</p>
        <Button variant="secondary" disabled={busy} onClick={() => void perform(async () => {
          const result = await rotateRecoveryKey()
          if (result.recovery_key) setSecret({ title: "Recovery key", value: result.recovery_key })
        })}>Rotate recovery key</Button>
      </Card>
      <Card title="Hub keys">
        <form className="account-inline-form" onSubmit={(event) => void submitKey(event)}>
          <Input label="Key label" value={label} onChange={(event) => setLabel(event.target.value)} required maxLength={80} />
          <Button type="submit" disabled={busy}>Create key</Button>
        </form>
        {keys.length ? <ul className="account-list">{keys.map((key) => <li key={key.id}>
          <span><strong>{key.id}</strong><small className="muted">Scopes: {key.scopes.join(", ") || "none"}</small></span>
          <Button variant="danger" disabled={busy} onClick={() => void perform(() => revokeHubKey(key.id))}>Revoke</Button>
        </li>)}</ul> : <p className="muted">No hub keys yet.</p>}
      </Card>
      <Card title="Active sessions">
        {sessions.length ? <ul className="account-list">{sessions.map((session) => <li key={session.session_id}>
          <span><strong>Session {session.sid.slice(0, 8)}</strong><small className="muted">Last active {timestamp(session.last_seen)} · Expires {timestamp(session.expires_at)}</small></span>
          <Button variant="danger" disabled={busy} onClick={() => {
            if (session.sid === currentSid) {
              void endSession(session.sid).then(() => navigate("/login", { replace: true })).catch((err: unknown) => setError(err instanceof Error ? err.message : "Could not end session"))
            } else {
              void perform(() => endSession(session.sid))
            }
          }}>End session</Button>
        </li>)}</ul> : <p className="muted">No active sessions.</p>}
      </Card>
      <Button variant="ghost" onClick={() => navigate("/")}>Back to overview</Button>
      <SecretReveal open={secret !== null} value={secret?.value ?? ""} title={secret?.title} onClose={() => setSecret(null)} />
    </div>
  )
}
