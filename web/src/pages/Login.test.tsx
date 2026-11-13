import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { Login } from "./Login"

function mockFetch(body: unknown, ok: boolean) {
  const fn = vi.fn().mockResolvedValue({
    ok,
    status: ok ? 200 : 401,
    json: async () => body,
  })
  vi.stubGlobal("fetch", fn)
  return fn
}

test("posts the credentials and does not render the csrf token", async () => {
  const fetchMock = mockFetch({ csrf_token: "csrf-abc", user_id: "u7" }, true)
  render(<Login />)

  fireEvent.change(screen.getByLabelText("Email"), { target: { value: "user@riehub.dev" } })
  fireEvent.change(screen.getByLabelText("Password"), { target: { value: "hunter2" } })
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }))

  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/auth/login",
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        body: JSON.stringify({ email: "user@riehub.dev", password: "hunter2" }),
      }),
    ),
  )
  expect(await screen.findByText(/user u7/)).toBeInTheDocument()
  expect(screen.queryByText(/csrf-abc/)).not.toBeInTheDocument()
})

test("shows the server error on a bad password", async () => {
  mockFetch({ message: "wrong password" }, false)
  render(<Login />)

  fireEvent.change(screen.getByLabelText("Email"), { target: { value: "user@riehub.dev" } })
  fireEvent.change(screen.getByLabelText("Password"), { target: { value: "nope" } })
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }))

  expect(await screen.findByRole("alert")).toHaveTextContent("wrong password")
})
