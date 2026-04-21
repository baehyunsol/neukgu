// The entire working directory is cloned to sandbox every turn (while waiting for LLM response).
// It has 2 purposes:
//     1. If there's an unrecoverable error, it restores the working directory using the backup.
//
// Be careful not to place sandbox-root inside working-dir.

use ragit_fs::{
    FileError,
    basename,
    copy_file,
    create_dir,
    exists,
    is_dir,
    join,
    read_dir,
    remove_dir_all,
};

pub fn export_to_sandbox(sandbox_root: &str, working_dir: &str) -> Result<String, FileError> {
    let sandbox_at = create_sandbox(sandbox_root)?;
    copy_recursive(working_dir, &sandbox_at, true, true)?;
    Ok(sandbox_at)
}

pub fn import_from_sandbox(sandbox_at: &str, working_dir: &str, copy_index_dir: bool) -> Result<(), FileError> {
    copy_recursive(sandbox_at, working_dir, true, copy_index_dir)
}

fn create_sandbox(sandbox_root: &str) -> Result<String, FileError> {
    let mut curr_dir = sandbox_root.to_string();

    for _ in 0..(rand::random::<u32>() % 4 + 4) {
        let id = format!("{:032x}", rand::random::<u128>());
        curr_dir = join(&curr_dir, &id)?;
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
