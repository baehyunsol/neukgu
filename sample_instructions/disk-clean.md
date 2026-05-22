I want to write a program that helps me clean garbages in my disk. This is my current workflow.

1. I got an error message from whatever program, complaining that there's no left space in my disk.
  - This step doesn't have to be automated.
  - I'll fire the cleaner manually when I encounter such error message.
2. I open nushell and run `cd ~; ls -d -a | sort-by -r size | first 64`.
  - I start the workflow at `~`.
  - `ls -d -a`
    - It lists the files/directories in the current directory.
    - `-d` means it shows the disk usage of the directories
    - In nushell, `ls` shows the size of files/directories even if there's no `-l` option
  - `sort-by -r size` sorts the entries by their size. Larger ones come first.
  - `first 64` is like `head -n 64`. There may be a lot of entries, but I don't care about small ones, so it only displays the first 64 entries.
3. I look at the list manually. There must be suspicious entries. Let's say `Documents/` is using 100GiB, `Downloads/` is using 30GiB, `.cache/` is using 29GiB and `.cargo/` is using 7GiB.
  - It's really complicated to tell what's "suspicious" and what's not. We have to discuss about it to make a heuristic that tells what's suspicious.
4. I open notepad and collect the suspicious directories/files.
  - So, I'll write `~/Documents/`, `~/Downloads/` and `~/.cargo/` in this case.
5. I run `cd Documents; ls -d -a | sort-by -r size | first 64`. I do the same thing again. I'll collect suspicious files/directories in DFS way.
6. After scanning the directories manually, I'll have a list of large directories and files. The list would look like this:

```
- ~/Documents/  100 GiB
  - ~/Documents/Rust/  70GiB
    - ~/Documents/Rust/neukgu/  22GiB
    - ~/Documents/Rust/ragit/  7GiB
    - ~/Documents/Rust/Sodigy/  3GiB
  - ~/Documents/Python/  15GiB
    - ~/Documents/Python/h-gpt/  13GiB
    - ~/Documents/Python/reversi/  220MiB
  - ~/Documents/C/  2GiB
- ~/Downloads/  30 GiB
  - ~/Downloads/harry-potter/  8GiB
  - ~/Downloads/kakaotalk/  1GiB
- ~/.cargo/  7GiB
  - ~/.cargo/registry/  7GiB
    - ~/.cargo/registry/src/  7GiB
- ~/.cache/ 29GiB
  - ~/.cache/huggingface/  29GiB
    - ~/.cache/huggingface/hub/  29GiB
      - ~/.cache/huggingface/hub/models--moonshotai--Moonlight-16B-A3B-Instruct/  29GiB
```

7. I review the list. This review process is manual and cannot be automated.
  - In this example, I would run `cargo clean` in the rust projects, consider removing movies, and consider removing the moonshotai model.

I want you to write a program that automates step 2 ~ step 6. When I fire the program, it starts searching in the current directory, and creates an output that looks like step 6.
