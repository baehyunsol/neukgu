use crate::{Context, Error, subprocess};
use ragit_fs::{
    exists,
    join3,
};
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

pub fn load_available_binaries() -> Result<Vec<String>, Error> {
    let mut available_binaries = vec![];
    let mut unavailable_binaries = vec![];
    let bin_list: Vec<(&str, &[&str], &str)> = vec![
        ("git", &["version"], r".*git.*\d+\.\d+.+"),
        ("cargo", &["version"], r".*cargo.*\d+\.\d+.+"),
        ("python3", &["-c", "print(3162277660168379331998 * 3162277660168379331998)"], ".*9999999999999999999994348728804092706672004.*"),
        ("rg", &["--version"], r".*ripgrep.*\d+\.\d+.+"),
    ];

    for (bin, args, checker) in bin_list.iter() {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();

        match subprocess::run(bin.to_string(), &args, ".", 1) {
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
            Err(_) => {
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
