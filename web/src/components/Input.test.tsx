import { fireEvent, render, screen } from "@testing-library/react"
import { Input } from "./Input"

test("associates the label with the input and accepts typed text", () => {
  render(<Input label="Email" type="email" />)
  const input = screen.getByLabelText("Email")
  expect(input).toHaveAttribute("type", "email")
  fireEvent.change(input, { target: { value: "user@example.com" } })
  expect(input).toHaveValue("user@example.com")
})
