import { fireEvent, render, screen } from "@testing-library/react"
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
