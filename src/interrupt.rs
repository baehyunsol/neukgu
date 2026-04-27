use chrono::Local;
use crate::Error;
use ragit_fs::{
    WriteMode,
    basename,
    join3,
    join4,
    read_dir,
    remove_file,
    write_bytes,
};

// How the interruption mechanism works.
//
// 1. There are files in `.neukgu/interruptions/`.
//    - Each file's file name is the timestamp of the file's creation time.
// 2. If the backend sees an interruption file in the directory and the
//    interruption is less than 5 seconds old, the backend immediately
//    halts the current turn.
// 3. Whenever the backend checks the interruption directory, the directory
//    is emptied.

pub fn check_interruption(working_dir: &str) -> Result<bool, Error> {
    let interruption_dir = join3(working_dir, ".neukgu", "interruptions")?;
    let now = Local::now().timestamp_millis();
    let mut files_to_remove = vec![];
    let mut interruption = false;

    for file in read_dir(&interruption_dir, false)? {
        let basename = basename(&file)?;
        files_to_remove.push(file);

        if let Ok(n) = basename.parse::<i64>() {
            if n + 5000 > now {
                interruption = true;
            }
        }
    }

    for file in files_to_remove.iter() {
        remove_file(file)?;
    }

    Ok(interruption)
}

pub fn interrupt_be(working_dir: &str) -> Result<(), Error> {
    let now = Local::now().timestamp_millis().to_string();
    write_bytes(
        &join4(working_dir, ".neukgu", "interruptions", &now)?,
        b"",
        WriteMode::Atomic,
    )?;
    Ok(())
}
