import { render, screen } from "@testing-library/react"
import { Toast } from "./Toast"

test("shows the message with a tone dot, announced as a status", () => {
  render(<Toast message="Saved" tone="good" />)
  const toast = screen.getByRole("status")
  expect(toast).toHaveTextContent("Saved")
  expect(toast.querySelector(".toast-dot")).toBeInTheDocument()
})
