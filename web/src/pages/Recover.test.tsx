import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { Recover } from "./Recover"

function mockFetch(body: unknown, ok: boolean) {
  const fn = vi.fn().mockResolvedValue({
    ok,
    status: ok ? 200 : 400,
    json: async () => body,
  })
  vi.stubGlobal("fetch", fn)
  return fn
}

test("posts the recovery data and confirms the change", async () => {
  const fetchMock = mockFetch({ ok: true }, true)
  render(<Recover />)

  fireEvent.change(screen.getByLabelText("Email"), { target: { value: "user@riehub.dev" } })
  fireEvent.change(screen.getByLabelText("Recovery key"), {
    target: { value: "rk-1234-abcd" },
  })
  fireEvent.change(screen.getByLabelText("New password"), { target: { value: "new-pass-9" } })
  fireEvent.click(screen.getByRole("button", { name: "Set new password" }))

  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/auth/recover",
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        body: JSON.stringify({
          email: "user@riehub.dev",
          recovery_key: "rk-1234-abcd",
          new_password: "new-pass-9",
        }),
      }),
    ),
  )
  expect(await screen.findByText("Your password has been changed")).toBeInTheDocument()
})

test("shows the server error when recovery fails", async () => {
  mockFetch({ message: "recovery key does not match" }, false)
  render(<Recover />)

  fireEvent.change(screen.getByLabelText("Email"), { target: { value: "user@riehub.dev" } })
  fireEvent.change(screen.getByLabelText("Recovery key"), { target: { value: "rk-bad" } })
  fireEvent.change(screen.getByLabelText("New password"), { target: { value: "new-pass-9" } })
  fireEvent.click(screen.getByRole("button", { name: "Set new password" }))

  expect(await screen.findByRole("alert")).toHaveTextContent("recovery key does not match")
})
