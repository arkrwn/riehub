import { render, screen } from "@testing-library/react"
import { StatusDot } from "./StatusDot"

test("renders a readable status and tone marker", () => {
  render(<StatusDot label="Active" tone="good" />)
  expect(screen.getByText("Active")).toBeInTheDocument()
  expect(screen.getByText("Active").previousElementSibling).toHaveClass("bg-good")
})

test("defaults to neutral and hides the decorative marker from assistive technology", () => {
  const { container } = render(<StatusDot label="Unknown" />)
  expect(container.querySelector('[aria-hidden="true"]')).toHaveClass("bg-muted")
})
