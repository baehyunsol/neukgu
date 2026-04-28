use crate::{
    Config,
    Context,
    ContextJson,
    MockState,
    Error,
    TurnId,
    copy_recursive,
    load_json,
};
use ragit_fs::{
    WriteMode,
    create_dir,
    exists,
    join3,
    join4,
    remove_dir_all,
    write_string,
};
use serde::{Deserialize, Serialize};

type Snapshots = Vec<Snapshot>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Snapshot {
    pub seq: usize,
    pub turn: TurnId,
    pub context: ContextJson,
    pub config: Config,
    pub mock_state: Option<MockState>,
}

impl Context {
    // Creates a snapshot every 6 turns
    pub fn has_to_create_snapshot(&self) -> bool {
        self.history.len() % 6 == 1
    }

    pub fn create_snapshot(&mut self) -> Result<(), Error> {
        let snapshots_at = join3(&self.working_dir, ".neukgu", "snapshots.json")?;
        let mut snapshots: Snapshots = load_json(&snapshots_at)?;

        // It only keeps at most 5 snapshots. -> The very first snapshot of the session and the most recent 4 snapshots.
        if snapshots.len() >= 5 {
            for snapshot in &snapshots[1..(snapshots.len() - 3)] {
                let old_snapshot_at = join4(&self.working_dir, ".neukgu", "snapshots", &snapshot.turn.0)?;
                remove_dir_all(&old_snapshot_at)?;
            }

            while snapshots.len() >= 5 {
                snapshots.remove(1);
            }

            write_string(
                &snapshots_at,
                &serde_json::to_string_pretty(&snapshots)?,
                WriteMode::Atomic,
            )?;
        }

        let turn = self.history.last().unwrap();
        let snapshot_at = join4(&self.working_dir, ".neukgu", "snapshots", &turn.id.0)?;

        if exists(&snapshot_at) {
            return Ok(());
        }

        let mock_state_at = join3(&self.working_dir, ".neukgu", "mock.json")?;

        let mock_state: Option<MockState> = if exists(&mock_state_at) {
            Some(load_json(&mock_state_at)?)
        } else {
            None
        };

        create_dir(&snapshot_at)?;
        copy_recursive(&self.working_dir, &snapshot_at, true, false)?;

        snapshots.push(Snapshot {
            seq: snapshots.last().map(|snapshot| snapshot.seq + 1).unwrap_or(0),
            turn: turn.id.clone(),
            context: self.to_json(),
            config: Config::load(&self.working_dir)?,
            mock_state,
        });
        write_string(
            &snapshots_at,
            &serde_json::to_string_pretty(&snapshots)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }
}

pub fn clean_dangling_snapshots(seq: usize, working_dir: &str) -> Result<(), Error> {
    let snapshots_at = join3(working_dir, ".neukgu", "snapshots.json")?;
    let mut snapshots: Snapshots = load_json(&snapshots_at)?;

    for snapshot in snapshots.iter() {
        if snapshot.seq > seq {
            let dangling_snapshot_at = join4(working_dir, ".neukgu", "snapshots", &snapshot.turn.0)?;
            remove_dir_all(&dangling_snapshot_at)?;
        }
    }

    snapshots = snapshots.into_iter().filter(|snapshot| snapshot.seq <= seq).collect();
    write_string(
        &snapshots_at,
        &serde_json::to_string_pretty(&snapshots)?,
        WriteMode::Atomic,
    )?;
    Ok(())
}

pub fn check_snapshot(id: &TurnId, working_dir: &str) -> Result<bool, Error> {
    let snapshot_at = join4(working_dir, ".neukgu", "snapshots", &id.0)?;
    Ok(exists(&snapshot_at))
}
