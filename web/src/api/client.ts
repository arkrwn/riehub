/*
  Plain fetch wrappers for the signed out API surface. Every request
  goes to the same origin and sends the HttpOnly session cookie. No
  token is ever stored in the browser.
*/

import type { paths } from "./types"

export type SetupResponse = paths["/api/v1/setup"]["post"]["responses"][200]["content"]["application/json"]
export type LoginResponse = paths["/api/v1/auth/login"]["post"]["responses"][200]["content"]["application/json"]
export type RecoverResponse = paths["/api/v1/auth/recover"]["post"]["responses"][200]["content"]["application/json"]
export type AcceptInviteResponse = paths["/api/v1/invites/{token}"]["post"]["responses"][200]["content"]["application/json"]
export type MeResponse = paths["/api/v1/me"]["get"]["responses"][200]["content"]["application/json"]

const BASE = "/api/v1"
let csrfToken: string | null = null

export class ApiError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "ApiError"
    this.status = status
  }
}

export interface RequestOptions {
  method: string
  path: string
  body?: unknown
}

export async function request<T>({ method, path, body }: RequestOptions): Promise<T> {
  const headers: Record<string, string> = {}
  if (body !== undefined) headers["Content-Type"] = "application/json"
  if (method !== "GET" && csrfToken) headers["x-csrf-token"] = csrfToken
  const res = await fetch(BASE + path, {
    method,
    credentials: "include",
    headers: Object.keys(headers).length ? headers : undefined,
    body: body === undefined ? undefined : JSON.stringify(body),
  })

  if (!res.ok) {
    const data = (await res.json().catch(() => null)) as { message?: string; detail?: string; title?: string } | null
    const message = data?.message ?? data?.detail ?? data?.title ?? `request failed with status ${res.status}`
    throw new ApiError(message, res.status)
  }

  return (await res.json()) as T
}

export async function setup(email: string, password: string): Promise<SetupResponse> {
  return request<SetupResponse>({
    method: "POST",
    path: "/setup",
    body: { email, password },
  })
}

export async function login(email: string, password: string): Promise<LoginResponse> {
  const result = await request<LoginResponse>({
    method: "POST",
    path: "/auth/login",
    body: { email, password },
  })
  csrfToken = result.csrf_token ?? null
  return result
}

export function recover(
  email: string,
  recovery_key: string,
  new_password: string,
): Promise<RecoverResponse> {
  return request<RecoverResponse>({
    method: "POST",
    path: "/auth/recover",
    body: { email, recovery_key, new_password },
  })
}

export function acceptInvite(token: string, password: string): Promise<AcceptInviteResponse> {
  return request<AcceptInviteResponse>({
    method: "POST",
    path: `/invites/${encodeURIComponent(token)}`,
    body: { password },
  })
}

export function me(): Promise<MeResponse> {
  return request<MeResponse>({ method: "GET", path: "/me" }).then((result) => {
    csrfToken = result.csrf_token ?? null
    return result
  })
}

export function logout(): Promise<{ status?: string }> {
  return request<{ status?: string }>({ method: "POST", path: "/auth/logout" }).finally(() => {
    csrfToken = null
  })
}

export type SessionInfo = { session_id: string; sid: string; created_at: number; last_seen: number; expires_at: number }
export type HubKeyInfo = { id: string; scopes: string[]; acl_prefixes: string[]; expires_at: number | null }

export async function listSessions(): Promise<SessionInfo[]> {
  const result = await request<{ items?: SessionInfo[] }>({ method: "GET", path: "/me/sessions" })
  return result.items ?? []
}

export async function endSession(id: string): Promise<void> {
  await request({ method: "DELETE", path: `/me/sessions/${encodeURIComponent(id)}` })
}

export async function listHubKeys(): Promise<HubKeyInfo[]> {
  const result = await request<{ items?: HubKeyInfo[] }>({ method: "GET", path: "/me/keys" })
  return result.items ?? []
}

export function createHubKey(label: string): Promise<{ id?: string; token?: string }> {
  return request({ method: "POST", path: "/me/keys", body: { label, scopes: ["ai"] } })
}

export async function revokeHubKey(id: string): Promise<void> {
  await request({ method: "DELETE", path: `/me/keys/${encodeURIComponent(id)}` })
}

export function changePassword(current_password: string, new_password: string): Promise<{ status?: string }> {
  return request({ method: "POST", path: "/me/password", body: { current_password, new_password } })
}

export function rotateRecoveryKey(): Promise<{ recovery_key?: string }> {
  return request({ method: "POST", path: "/me/recovery-key/rotate" })
}
