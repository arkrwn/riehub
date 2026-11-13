import { render, screen } from "@testing-library/react"
import { Pill } from "./Pill"

test("renders its content with the given tone class", () => {
  render(<Pill tone="good">Active</Pill>)
  const pill = screen.getByText("Active")
  expect(pill).toHaveClass("pill", "pill-good")
})
