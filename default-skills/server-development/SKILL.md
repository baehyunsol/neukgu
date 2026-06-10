---
name: server-development
description: This skill describes how to develop and test a server program with `<run>` tool. A server is a program that lives long in the background, accepts requests (usually https, but can be anything).
---

# Server

## Limitations

You're neukgu, an AI coding harness. The harness has `<run>` tool, which spawns a process, waits until the process dies and collects stdout/stderr of the process. This is perfect for simple CLI programs, but not for server programs because,

1. `<run>` tool can spawn a single process at a time, and you cannot use multiple `<run>` tools at the same time. When testing a server program, you usually need 2 processes: server & client. But you can't spawn both at the same time.
2. `<run>` tool waits until the process dies. But usually, a server program doesn't die unless it crashes.

## Workarounds

You need to write a helper script that spawns multiple processes and collects their outputs.

Let's say you have a server written in Rust, in `server/` and a test client `test-client.py`. The client sends requests to a given port and asserts the responses. You have to write a helper script that

1. Spawns the server
2. Runs the test client and checks if the test passes
3. Dumps the test result to stdout
  - Or, you can dump the result to wherever other tools can read. For example, you can dump the result to files and read the files with the `<read>` tool.
4. Kills the server
  - This is very important. `<run>` tool assumes that every process dies before the tool call ends. So you have to make sure that you kill all the processes you spawned.

I recommend you write the helper script in Python because the harness supports Python well.

```py
# helper.py

# 1. Spawn the server
os.chdir("path-of-your-server")
server_process = subprocess.Popen(["cargo", "run", "--", "--port", "9000"])  # Let's say the server is written in Rust.

# 2. Run the test client
os.chdir("path-of-your-test-client")
subprocess.run(["python3", "test-client.py", "--port", "9000", "--output", "test-result.json"])

# 3. It'll dump the result at `test-result.json`. You can later use the `<read>` tool to check the results.

# 4. Make sure to kill the server processes!!
server_process.terminate()
server_process.wait()
```

Then, you can run `<run><command>python3 helper.py</command></run>` to test your server.
