use crate::{Context, Error, subprocess};
use ragit_fs::{exists, join, join3};
use regex::Regex;

impl Context {
    // For example, if `bin` is `"git"`, it'll just call `git` and rust's `std::process::Command`
    // will find `/usr/bin/git` using PATH. If the user provided `eval` in `bins/`, then it'll
    // execute `bins/eval`.
    pub fn get_bin_path(&self, sandbox_at: &str, bin: &str) -> Result<String, Error> {
        let real_bin = join3(sandbox_at, "bins", bin)?;

        if exists(&real_bin) {
            Ok(real_bin)
        } else {
            Ok(bin.to_string())
        }
    }
}

pub fn load_available_binaries(working_dir: &str) -> Result<Vec<String>, Error> {
    let mut available_binaries = vec![];
    let mut unavailable_binaries = vec![];
    let bin_list: Vec<(&str, &[&str], &str)> = vec![
        ("git", &["version"], r".*git.*\d+\.\d+.+"),
        ("cargo", &["version"], r".*cargo.*\d+\.\d+.+"),
        // ("python3", &[""], ""),
        ("rg", &["--version"], r".*ripgrep.*\d+\.\d+.+"),
    ];

    match try_init_python_venv(working_dir) {
        Ok(_) => {
            available_binaries.push(String::from("python3"));
            available_binaries.push(String::from("pip"));
        },
        Err(e) => {
            eprintln!("{e:?}");
            unavailable_binaries.push(String::from("python3"));
            unavailable_binaries.push(String::from("pip"));
        },
    }

    for (bin, args, checker) in bin_list.iter() {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();

        match subprocess::run(bin.to_string(), &args, &[], ".", 1, "", false) {
            Ok(o) => {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                let checker = Regex::new(checker).unwrap();

                if checker.is_match(&stdout) {
                    available_binaries.push(bin.to_string());
                }

                else {
                    unavailable_binaries.push(bin.to_string());
                }
            },
            Err(e) => {
                eprintln!("{e:?}");
                unavailable_binaries.push(bin.to_string());
            },
        }
    }

    if unavailable_binaries.is_empty() {
        Ok(available_binaries)
    }

    else {
        Err(Error::UnavailableBinaries(unavailable_binaries))
    }
}

fn try_init_python_venv(working_dir: &str) -> Result<(), Error> {
    let py_venv = join3(working_dir, ".neukgu", "py-venv")?;
    let python3_link = join3(&py_venv, "bin", "python3")?;

    if exists(&python3_link) {
        return Ok(());
    }

    let output = subprocess::run(
        String::from("python3"),
        &[String::from("-m"), String::from("venv"), String::from("py-venv")],
        &[],
        &join(working_dir, ".neukgu")?,
        5,
        working_dir,
        false,
    )?;

    if output.timeout || !exists(&python3_link) || !exists(&join3(&py_venv, "bin", "pip")?) {
        return Err(Error::FailedToInitPythonVenv);
    }

    Ok(())
}

// Python venv doesn't work on some platforms (e.g. python 3.9 on my Mac book).
// So it checks whether python3 & pip are alive in the sandbox.
pub fn check_python_venv(
    env: &[(&str, String)],
    sandbox_at: &str,
    working_dir: &str,
) -> Result<(), Error> {
    let pip_result = subprocess::run(
        String::from("pip"),
        &["help"].iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
        env,
        sandbox_at,
        3,
        working_dir,
        true,
    )?;

    if !pip_result.stdout.windows(7).any(|w| w == b"install") || !pip_result.stdout.windows(8).any(|w| w == b"download") {
        eprintln!("---- failed to init python venv ----");
        eprintln!("<command>");
        eprintln!("pip help");
        eprintln!("</command>");
        eprintln!("<stdout>");
        eprintln!("{}", String::from_utf8_lossy(&pip_result.stdout));
        eprintln!("</stdout>");
        eprintln!("<stderr>");
        eprintln!("{}", String::from_utf8_lossy(&pip_result.stderr));
        eprintln!("</stderr>");
        return Err(Error::FailedToInitPythonVenv);
    }

    let py_result = subprocess::run(
        String::from("python3"),
        &["-c", "print(3162277660168379 * 3162277660168379)"].iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
        env,
        sandbox_at,
        3,
        working_dir,
        true,
    )?;

    if !py_result.stdout.windows(31).any(|w| w == b"9999999999999997900254631487641") {
        eprintln!("---- failed to init python venv ----");
        eprintln!("<command>");
        eprintln!("python -c \"print(3162277660168379 * 3162277660168379)\"");
        eprintln!("</command>");
        eprintln!("<stdout>");
        eprintln!("{}", String::from_utf8_lossy(&py_result.stdout));
        eprintln!("</stdout>");
        eprintln!("<stderr>");
        eprintln!("{}", String::from_utf8_lossy(&py_result.stderr));
        eprintln!("</stderr>");
        return Err(Error::FailedToInitPythonVenv);
    }

    Ok(())
}
