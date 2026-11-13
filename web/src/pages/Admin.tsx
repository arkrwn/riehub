import { useCallback, useEffect, useState, type FormEvent } from "react"
import {
  createAdminUser,
  deleteAdminUser,
  listAdminAudit,
  listAdminUsers,
  patchAdminUser,
  type AdminAuditEntry,
  type AdminUser,
} from "../api/admin"
import { Button } from "../components/Button"
import { Card } from "../components/Card"
import { DataTable, type DataTableColumn } from "../components/DataTable"
import { Dialog } from "../components/Dialog"
import { ErrorState } from "../components/ErrorState"
import { Input } from "../components/Input"
import { SecretReveal } from "../components/SecretReveal"
import { StatusDot } from "../components/StatusDot"

function statusTone(status: string) {
  if (status === "active") return "good" as const
  if (status === "disabled") return "bad" as const
  if (status === "pending") return "warn" as const
  return "neutral" as const
}

function dateTime(timestamp: number) {
  return new Date(timestamp).toLocaleString()
}

export function Admin() {
  const [users, setUsers] = useState<AdminUser[]>([])
  const [audit, setAudit] = useState<AdminAuditEntry[]>([])
  const [email, setEmail] = useState("")
  const [displayName, setDisplayName] = useState("")
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [secret, setSecret] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<AdminUser | null>(null)

  const refresh = useCallback(async () => {
    const [nextUsers, nextAudit] = await Promise.all([listAdminUsers(), listAdminAudit()])
    setUsers(nextUsers)
    setAudit(nextAudit)
  }, [])

  useEffect(() => {
    void refresh().catch((err: unknown) => setError(err instanceof Error ? err.message : "Could not load admin data"))
  }, [refresh])

  async function run(action: () => Promise<unknown>) {
    setBusy(true)
    setError(null)
    try {
      await action()
      await refresh()
    } catch (err) {
      setError(err instanceof Error ? err.message : "Admin request failed")
    } finally {
      setBusy(false)
    }
  }

  async function invite(event: FormEvent) {
    event.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const result = await createAdminUser(email, displayName || undefined)
      setEmail("")
      setDisplayName("")
      await refresh()
      if (result.invite_token) setSecret(result.invite_token)
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not create invite")
    } finally {
      setBusy(false)
    }
  }

  async function confirmDelete() {
    if (!deleteTarget) return
    const target = deleteTarget
    setDeleteTarget(null)
    await run(() => deleteAdminUser(target.id))
  }

  const userColumns: DataTableColumn<AdminUser>[] = [
    {
      key: "user",
      header: "User",
      render: (user) => <span className="grid gap-1"><strong>{user.display_name || user.email}</strong>{user.display_name ? <small className="text-muted">{user.email}</small> : null}</span>,
    },
    {
      key: "status",
      header: "Status",
      render: (user) => <StatusDot label={user.status} tone={statusTone(user.status)} />,
    },
    {
      key: "roles",
      header: "Roles",
      render: (user) => user.roles.join(", ") || "member",
    },
    {
      key: "actions",
      header: "Actions",
      render: (user) => {
        const isOwner = user.roles.includes("owner")
        const isAdmin = user.roles.includes("admin") || isOwner
        const nextRoles = isAdmin ? user.roles.filter((role) => role !== "admin") : [...user.roles, "admin"]
        return <div className="flex flex-wrap gap-2">
          {!isOwner ? <>
            <Button variant="secondary" disabled={busy} onClick={() => void run(() => patchAdminUser(user.id, { status: user.status === "disabled" ? "active" : "disabled" }))}>
              {user.status === "disabled" ? "Enable" : "Disable"}
            </Button>
            <Button variant="ghost" disabled={busy} onClick={() => void run(() => patchAdminUser(user.id, { roles: nextRoles }))}>
              {isAdmin ? "Remove admin" : "Make admin"}
            </Button>
            <Button variant="danger" disabled={busy} onClick={() => setDeleteTarget(user)}>Delete</Button>
          </> : <span className="muted small">Owner protected</span>}
        </div>
      },
    },
  ]

  const auditColumns: DataTableColumn<AdminAuditEntry>[] = [
    { key: "sequence", header: "Event", render: (entry) => `#${entry.seq} · ${entry.kind}` },
    { key: "time", header: "Time", render: (entry) => dateTime(entry.ts) },
  ]

  return (
    <div className="account-page">
      <header>
        <p className="eyebrow">Workspace</p>
        <h1 className="section-title">Administration</h1>
        <p className="muted">Manage accounts and review security event metadata.</p>
      </header>
      {error ? <ErrorState message={error} onRetry={() => { setError(null); void refresh() }} /> : null}
      <Card title="Invite a user">
        <form className="account-form" onSubmit={(event) => void invite(event)}>
          <Input label="Email" type="email" autoComplete="off" value={email} onChange={(event) => setEmail(event.target.value)} required />
          <Input label="Display name" value={displayName} onChange={(event) => setDisplayName(event.target.value)} maxLength={120} />
          <Button type="submit" disabled={busy}>Create invite</Button>
        </form>
      </Card>
      <Card title="Users">
        <DataTable rows={users} columns={userColumns} rowKey={(user) => user.id} caption="Workspace users" emptyTitle="No users yet" />
      </Card>
      <Card title="Audit log">
        <DataTable rows={audit} columns={auditColumns} rowKey={(entry) => String(entry.seq)} caption="Audit events" emptyTitle="No audit events yet" />
      </Card>
      <SecretReveal open={secret !== null} value={secret ?? ""} title="Invite token" onClose={() => setSecret(null)} />
      <Dialog open={deleteTarget !== null} title="Delete user" onClose={() => setDeleteTarget(null)}>
        <p>Delete {deleteTarget?.email}? This action cannot be undone.</p>
        <div className="dialog-actions">
          <Button variant="secondary" onClick={() => setDeleteTarget(null)}>Cancel</Button>
          <Button variant="danger" disabled={busy} onClick={() => void confirmDelete()}>Delete user</Button>
        </div>
      </Dialog>
    </div>
  )
}
