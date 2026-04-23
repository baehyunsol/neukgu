// The entire working directory is cloned to sandbox every turn (while waiting for LLM response).
// It has 2 purposes:
//     1. If there's an unrecoverable error, it restores the working directory using the backup.
//     2. Whenever the agent runs code, the code runs in sandbox.
//
// Be careful not to place sandbox-root inside working-dir.

use crate::{Error, load_json};
use ragit_fs::{
    FileError,
    WriteMode,
    basename,
    copy_file,
    create_dir,
    exists,
    into_abs_path,
    is_dir,
    join,
    join3,
    normalize,
    parent,
    read_dir,
    remove_dir_all,
    write_string,
};
use std::collections::HashSet;

pub fn export_to_sandbox(sandbox_root: &str, working_dir: &str) -> Result<String, Error> {
    let sandbox_at = create_sandbox(sandbox_root, working_dir)?;
    copy_recursive(working_dir, &sandbox_at, true, true)?;
    Ok(sandbox_at)
}

pub fn import_from_sandbox(sandbox_at: &str, working_dir: &str, copy_index_dir: bool) -> Result<(), FileError> {
    copy_recursive(sandbox_at, working_dir, true, copy_index_dir)
}

pub fn clean_sandbox(sandbox_root: &str, sandbox_at: &str, working_dir: &str) -> Result<(), Error> {
    let mut sandbox_root = normalize(&into_abs_path(sandbox_root)?)?;
    let mut curr_dir = normalize(&into_abs_path(sandbox_at)?)?;

    // There's a bug in `normalize` :(
    if sandbox_root.ends_with("/") {
        sandbox_root.pop();
    }

    if curr_dir.ends_with("/") {
        curr_dir.pop();
    }

    while curr_dir != sandbox_root {
        remove_dir_all(&curr_dir)?;
        remove_wal(&curr_dir, working_dir)?;
        curr_dir = parent(&curr_dir)?;

        if curr_dir.ends_with("/") {
            curr_dir.pop();
        }
    }

    Ok(())
}

pub fn clean_dangling_sandboxes(working_dir: &str) -> Result<(), Error> {
    let mut wal: HashSet<String> = load_json(&join3(working_dir, ".neukgu", "wal")?)?;

    for path in wal.drain() {
        if exists(&path) {
            remove_dir_all(&path)?;
        }
    }

    write_string(
        &join3(working_dir, ".neukgu", "wal")?,
        &serde_json::to_string_pretty(&wal)?,
        WriteMode::Atomic,
    )?;
    Ok(())
}

fn create_sandbox(sandbox_root: &str, working_dir: &str) -> Result<String, Error> {
    let mut curr_dir = sandbox_root.to_string();

    for i in 0..(rand::random::<u32>() % 4 + 4) {
        let id = format!("{:032x}", rand::random::<u128>());
        curr_dir = join(&curr_dir, &id)?;

        // If wal exists, but a corresponding sandbox dir doesn't, that's fine.
        // If a sandbox dir exists, but corresponding wal doesn't, that's a problem.
        // So we add before `create_dir`.
        if i == 0 {
            add_wal(&curr_dir, working_dir)?;
        }

        create_dir(&curr_dir)?;
    }

    Ok(curr_dir)
}

fn copy_recursive(
    src: &str,
    dst: &str,
    is_at_top_level: bool,
    copy_index_dir: bool,
) -> Result<(), FileError> {
    for e in read_dir(src, false)? {
        let e_base = basename(&e)?;
        let dst_e = join(dst, &e_base)?;

        if e_base == ".neukgu" && is_at_top_level && !copy_index_dir {
            continue;
        }

        if is_dir(&e) {
            if is_at_top_level && exists(&dst_e) {
                remove_dir_all(&dst_e)?;
            }

            create_dir(&dst_e)?;
            copy_recursive(&e, &dst_e, false, copy_index_dir)?;
        }

        else {
            copy_file(&e, &dst_e)?;
        }
    }

    Ok(())
}

fn add_wal(sandbox_at: &str, working_dir: &str) -> Result<(), Error> {
    let mut wal: HashSet<String> = load_json(&join3(working_dir, ".neukgu", "wal")?)?;
    wal.insert(sandbox_at.to_string());
    write_string(
        &join3(working_dir, ".neukgu", "wal")?,
        &serde_json::to_string_pretty(&wal)?,
        WriteMode::Atomic,
    )?;
    Ok(())
}

fn remove_wal(sandbox_at: &str, working_dir: &str) -> Result<(), Error> {
    let mut wal: HashSet<String> = load_json(&join3(working_dir, ".neukgu", "wal")?)?;
    wal.remove(sandbox_at);
    write_string(
        &join3(working_dir, ".neukgu", "wal")?,
        &serde_json::to_string_pretty(&wal)?,
        WriteMode::Atomic,
    )?;
    Ok(())
}
