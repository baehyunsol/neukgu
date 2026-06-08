https://github.com/iced-rs/iced -> This is an awesome Rust gui library!

I'm writing a complex GUI application using the library, and I want to know the limits of the library. I wanna know how performant it is.

1. What graphic backend does it use? Does it use CPU or GPU to render graphics?
  - If it uses CPU, is it single-threaded?
2. As far as I know, the library calculates the differences bewteen the previous frame's elements and the current frame's elements. It only re-draws the changed elements.
  - If it's true, there're a lot of room for optimizations in this logic. For example, if the library is dumb and re-draws redundant elements every frame, that'd be too slow. I wanna know what kinda optimization techniques it uses.
  - If it's not true, then does it mean it draws every elements every frame? That must be extremely slow. Figure out how it deals with such work-load (if it does).
3. I use `iced::advanced::widget::Text` and `iced::widget::TextEditor` (with syntax highlighting) to render long texts. I want it to be performant.
  - How do the widgets render long text? Is it capable of rendering very long text (e.g. millions of characters).

Clone the repository, inspect the source code and answer my questions. Write a detailed report at `docs/iced-widget.md`
