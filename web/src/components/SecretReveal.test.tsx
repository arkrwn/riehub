import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { SecretReveal } from "./SecretReveal"

test("shows the value once and copies it to the clipboard", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined)
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
  })
  const onClose = vi.fn()
  render(<SecretReveal open value="rk-1234-abcd" title="Recovery key" onClose={onClose} />)

  expect(screen.getByText("rk-1234-abcd")).toBeInTheDocument()
  fireEvent.click(screen.getByRole("button", { name: "Copy" }))
  await waitFor(() => {
    expect(writeText).toHaveBeenCalledWith("rk-1234-abcd")
    expect(screen.getByRole("button", { name: "Copied" })).toBeInTheDocument()
  })

  fireEvent.click(screen.getByRole("button", { name: "Done" }))
  expect(onClose).toHaveBeenCalledOnce()
})
