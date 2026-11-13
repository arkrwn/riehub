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

export class ApiError extends Error {
  status: number

  constructor(message: string, status: number) {
    super(message)
    this.name = "ApiError"
    this.status = status
  }
}

interface RequestOptions {
  method: string
  path: string
  body?: unknown
}

async function request<T>({ method, path, body }: RequestOptions): Promise<T> {
  const res = await fetch(BASE + path, {
    method,
    credentials: "include",
    headers: body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  })

  if (!res.ok) {
    const data = (await res.json().catch(() => null)) as { message?: string } | null
    const message = data?.message ?? `request failed with status ${res.status}`
    throw new ApiError(message, res.status)
  }

  return (await res.json()) as T
}

export function setup(email: string, password: string): Promise<SetupResponse> {
  return request<SetupResponse>({
    method: "POST",
    path: "/setup",
    body: { email, password },
  })
}

export function login(email: string, password: string): Promise<LoginResponse> {
  return request<LoginResponse>({
    method: "POST",
    path: "/auth/login",
    body: { email, password },
  })
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
  return request<MeResponse>({ method: "GET", path: "/me" })
}
