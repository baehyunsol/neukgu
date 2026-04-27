# `Error::FailedToInitPythonVenv`

`python3 -m venv` has issues if you're using older versions of python. I had an issue with python 3.9 on MacOS.

The easiest way to fix is to update python and re-initialize `.neukgu/`.

Neukgu finds `python3` in `PATH`, so make sure that the new version of python is in the `PATH`.
Or, you can do this manually. You can create venv with the new version of python, and locate the venv at `.neukgu/py-venv/`.  You can do it with `python3 -m venv .neukgu/py-venv`.
