import { fireEvent, render, screen } from "@testing-library/react"
import { Button } from "./Button"

test("renders its label, applies the variant and fires onClick", () => {
  const onClick = vi.fn()
  render(
    <Button variant="secondary" onClick={onClick}>
      Save changes
    </Button>,
  )
  const button = screen.getByRole("button", { name: "Save changes" })
  expect(button).toHaveClass("btn", "btn-secondary")
  fireEvent.click(button)
  expect(onClick).toHaveBeenCalledOnce()
})
