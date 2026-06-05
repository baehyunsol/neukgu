# Install

Read this manual and install the dependencies (and neukgu) one by one.

## 1. git

You must know how to install git, right? Run `git clone https://github.com/baehyunsol/neukgu` to clone the repository.

You need git to be in PATH in order to run neukgu. The agent can run git, and it will find git in your PATH.

## 2. cargo

https://rustup.rs/ -> You can download cargo here.

Once you have cargo, run `cargo build --release` in the repository.
Make sure to give the `--release` flag. Some features (e.g. pdf rendering) are extremely slow in debug mode.

You need cargo to be in PATH.

## 3. python3

Neukgu can write/execute python script. It'll look for `python3` in PATH. If you're not sure, open a terminal and execute `python3 --version`. If it shows you the version, it's fine.

When you initialize a working directory, it will initialize a python venv with `python3` in your PATH. Then, when you run neukgu, it'll use `python3` and `pip` in `py-venv/bin/`.

Sometimes, you might encounter `Error::FailedToInitPythonVenv` while running neukgu. I also have this issue on MacOS (python 3.9). As far as I know, the only fix is to install a newer version of Python.  Install a newer version and re-init your working directory.

If you don't want to re-init your working directory, you can re-init python venv manually. You'll find `.neukgu/py-venv/` in your working directory. That is the PythonVenv and you have to re-init it. Remove the old PythonVenv and run `python3 -m venv .neukgu/py-venv/` with the new version of Python. It might work.

## 4. chrome

When neukgu has to do something with a web browser, it'll look for chrome.

To be honest, I'm not sure how exactly it locates the chrome binary. It uses [headless_chrome](https://github.com/rust-headless-chrome/rust-headless-chrome) library.

Even if you don't have chrome, neukgu will run fine as long as it does not try to use a web browser.

## 5. rg (ripgrep)

Run `cargo install ripgrep`. `rg` must be in your PATH.

## 6. cc

In order for cargo to work, there must be cc in your PATH.
