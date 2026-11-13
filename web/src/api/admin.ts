import { request } from "./client"

export type AdminUser = {
  id: string
  email: string
  display_name: string | null
  status: "active" | "disabled" | "pending" | string
  roles: string[]
}

export type AdminAuditEntry = {
  seq: number
  ts: number
  kind: string
  event: unknown
}

export async function listAdminUsers(): Promise<AdminUser[]> {
  const result = await request<{ items?: AdminUser[] }>({ method: "GET", path: "/admin/users" })
  return result.items ?? []
}

export function createAdminUser(email: string, display_name?: string): Promise<{ user_id?: string; invite_token?: string }> {
  return request({
    method: "POST",
    path: "/admin/users",
    body: { email, ...(display_name ? { display_name } : {}) },
  })
}

export function createAdminInvite(email: string, display_name?: string): Promise<{ invite_token?: string }> {
  return request({
    method: "POST",
    path: "/admin/invites",
    body: { email, ...(display_name ? { display_name } : {}) },
  })
}

export function patchAdminUser(
  id: string,
  patch: { roles?: string[]; status?: "active" | "disabled" | "pending" },
): Promise<{ status?: string }> {
  return request({ method: "PATCH", path: `/admin/users/${encodeURIComponent(id)}`, body: patch })
}

export function deleteAdminUser(id: string): Promise<{ status?: string }> {
  return request({ method: "DELETE", path: `/admin/users/${encodeURIComponent(id)}` })
}

export async function listAdminAudit(): Promise<AdminAuditEntry[]> {
  const result = await request<{ items?: AdminAuditEntry[] }>({ method: "GET", path: "/admin/audit" })
  return result.items ?? []
}
