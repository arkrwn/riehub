import { fireEvent, render, screen } from "@testing-library/react"
import { IconButton } from "./IconButton"

test("exposes its label to assistive tech and fires onClick", () => {
  const onClick = vi.fn()
  render(
    <IconButton label="Toggle theme" onClick={onClick}>
      <span>icon</span>
    </IconButton>,
  )
  const button = screen.getByRole("button", { name: "Toggle theme" })
  button.focus()
  expect(button).toHaveFocus()
  fireEvent.click(button)
  expect(onClick).toHaveBeenCalledOnce()
})
