import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { MemoryRouter, Route, Routes } from "react-router"
import { AcceptInvite } from "./AcceptInvite"

function mockFetch(body: unknown, ok: boolean) {
  const fn = vi.fn().mockResolvedValue({
    ok,
    status: ok ? 200 : 400,
    json: async () => body,
  })
  vi.stubGlobal("fetch", fn)
  return fn
}

test("posts the password to the invite endpoint and confirms", async () => {
  const fetchMock = mockFetch({ ok: true }, true)
  render(
    <MemoryRouter initialEntries={["/invite/inv-token-1"]}>
      <Routes>
        <Route path="/invite/:token" element={<AcceptInvite />} />
      </Routes>
    </MemoryRouter>,
  )

  fireEvent.change(screen.getByLabelText("Password"), { target: { value: "new-pass-9" } })
  fireEvent.click(screen.getByRole("button", { name: "Create account" }))

  await waitFor(() =>
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/invites/inv-token-1",
      expect.objectContaining({
        method: "POST",
        credentials: "include",
        body: JSON.stringify({ password: "new-pass-9" }),
      }),
    ),
  )
  expect(await screen.findByText("Your account is ready")).toBeInTheDocument()
})

test("redirects to login when there is no token", () => {
  render(
    <MemoryRouter initialEntries={["/invite/"]}>
      <Routes>
        <Route path="/invite/:token?" element={<AcceptInvite />} />
        <Route path="/login" element={<p>sign in page</p>} />
      </Routes>
    </MemoryRouter>,
  )
  expect(screen.getByText("sign in page")).toBeInTheDocument()
  expect(screen.queryByRole("heading", { name: "Accept invite" })).not.toBeInTheDocument()
})
