https://github.com/iced-rs/iced -> This is an awesome Rust gui library!

I wanna know about its rendering process.

Its architecture is very simple. There's a Context (a Rust struct), a Message (a Rust enum), update function (`fn update(&mut context, message)`) and view function (`fn view(&context) -> Element`).
Element is the collection of widgets, and the library renders the Element.
An Element can create Messages which updates the Context.

So, I tell the library what to draw, not how to draw. I want to know how the library calculates how to draw the elements.

1. As far as I know, the library calculates the difference of the previous frame's Element and the current frame's Element. It only draws the updated widgets, if any.
  - Is it true?
  - If so, how does it calculate the difference? I don't think it would naively do `if prev_element != curr_element`
2. There's a widget called `Scrollable`. It allows you to scroll the inner widget. When the ui changes a lot, the scroll state is sometimes reset. For example, there's a scrollable and a popup. I was scrolling. When I close the popup, the scrollable is suddenly scrolled to top.
  - I think it's related to the first question. Maybe some internal state is reset and affects the scroll state?
  - Anyway, I want you to figure out the mechanism behind this.

First, clone the repository.
Second, inspect the source code and collect information that is needed to answer my question.
Third, write a detailed report at `docs/iced.md`.
