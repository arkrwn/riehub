import { fireEvent, render, screen } from "@testing-library/react"
import { useState } from "react"
import { Dialog } from "./Dialog"

test("renders content when open, closes on overlay click and Escape", () => {
  const onClose = vi.fn()
  const { unmount } = render(
    <Dialog open onClose={onClose} title="Secret">
      <p>the value</p>
    </Dialog>,
  )
  const dialog = screen.getByRole("dialog", { name: "Secret" })
  expect(dialog).toBeInTheDocument()
  // A click inside the dialog does not close it, a click on the overlay does.
  fireEvent.mouseDown(dialog)
  expect(onClose).not.toHaveBeenCalled()
  fireEvent.mouseDown(screen.getByText("the value").closest(".dialog-overlay") as Element)
  fireEvent.keyDown(document, { key: "Escape" })
  expect(onClose).toHaveBeenCalledTimes(2)
  unmount()

  render(
    <Dialog open={false} onClose={onClose}>
      <p>hidden</p>
    </Dialog>,
  )
  expect(screen.queryByText("hidden")).not.toBeInTheDocument()
})

test("traps keyboard focus and restores focus when closed", () => {
  function Harness() {
    const [open, setOpen] = useState(false)
    return (
      <>
        <button onClick={() => setOpen(true)}>Open dialog</button>
        <Dialog open={open} onClose={() => setOpen(false)} title="Keyboard dialog">
          <button>First action</button>
          <button>Last action</button>
        </Dialog>
      </>
    )
  }

  render(<Harness />)
  const opener = screen.getByRole("button", { name: "Open dialog" })
  opener.focus()
  fireEvent.click(opener)
  const first = screen.getByRole("button", { name: "First action" })
  const last = screen.getByRole("button", { name: "Last action" })
  expect(first).toHaveFocus()

  fireEvent.keyDown(document, { key: "Tab", shiftKey: true })
  expect(last).toHaveFocus()
  fireEvent.keyDown(document, { key: "Tab" })
  expect(first).toHaveFocus()
  fireEvent.keyDown(document, { key: "Escape" })
  expect(opener).toHaveFocus()
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument()
})
