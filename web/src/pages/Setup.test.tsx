import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { Setup } from "./Setup"

function mockFetch(body: unknown, ok: boolean) {
  const fn = vi.fn().mockResolvedValue({
    ok,
    status: ok ? 200 : 500,
    json: async () => body,
  })
  vi.stubGlobal("fetch", fn)
  return fn
}

test("creates the account and shows the recovery key once", async () => {
  const fetchMock = mockFetch({ user_id: "u1", recovery_key: "rk-1234-abcd" }, true)
  render(<Setup />)

  fireEvent.change(screen.getByLabelText("Email"), { target: { value: "admin@riehub.dev" } })
  fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse" } })
  fireEvent.click(screen.getByRole("button", { name: "Create account" }))

  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/setup",
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        body: JSON.stringify({ email: "admin@riehub.dev", password: "correct horse" }),
      }),
    ),
  )
  const dialog = await screen.findByRole("dialog", { name: "Recovery key" })
  expect(dialog).toHaveTextContent("rk-1234-abcd")

  fireEvent.click(screen.getByRole("button", { name: "Done" }))
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument())
  expect(screen.queryByText("rk-1234-abcd")).not.toBeInTheDocument()
})

test("shows the server error when setup fails", async () => {
  mockFetch({ message: "email already registered" }, false)
  render(<Setup />)

  fireEvent.change(screen.getByLabelText("Email"), { target: { value: "admin@riehub.dev" } })
  fireEvent.change(screen.getByLabelText("Password"), { target: { value: "correct horse" } })
  fireEvent.click(screen.getByRole("button", { name: "Create account" }))

  expect(await screen.findByRole("alert")).toHaveTextContent("email already registered")
})
