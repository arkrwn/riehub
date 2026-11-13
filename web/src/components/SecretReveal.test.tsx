import { fireEvent, render, screen, waitFor } from "@testing-library/react"
import { useState } from "react"
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
  const copy = screen.getByRole("button", { name: "Copy" })
  expect(copy).toHaveFocus()
  fireEvent.click(copy)
  await waitFor(() => {
    expect(writeText).toHaveBeenCalledWith("rk-1234-abcd")
    expect(screen.getByRole("button", { name: "Copied" })).toBeInTheDocument()
  })

  fireEvent.click(screen.getByRole("button", { name: "Done" }))
  expect(onClose).toHaveBeenCalledOnce()
})

test("clears the secret from the page when the dialog closes", () => {
  function Harness() {
    const [open, setOpen] = useState(true)
    const [value, setValue] = useState("rk-once-only")
    return (
      <SecretReveal
        open={open}
        value={value}
        title="Recovery key"
        onClose={() => {
          setValue("")
          setOpen(false)
        }}
      />
    )
  }

  render(<Harness />)
  expect(screen.getByText("rk-once-only")).toBeInTheDocument()
  fireEvent.click(screen.getByRole("button", { name: "Done" }))
  expect(screen.queryByText("rk-once-only")).not.toBeInTheDocument()
})
