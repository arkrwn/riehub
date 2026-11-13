import { fireEvent, render, screen } from "@testing-library/react"
import { ErrorState } from "./ErrorState"

test("announces the message and offers a retry when asked", () => {
  const onRetry = vi.fn()
  render(<ErrorState message="server unreachable" onRetry={onRetry} />)
  expect(screen.getByRole("alert")).toHaveTextContent("server unreachable")
  const retry = screen.getByRole("button", { name: "Try again" })
  retry.focus()
  expect(retry).toHaveFocus()
  fireEvent.click(retry)
  expect(onRetry).toHaveBeenCalledOnce()
})
