use std::{
    env, fs::{File, OpenOptions, TryLockError}, io::{self, Read, Seek, Write}, path::PathBuf,
};

use rustix::process::{Pid, Signal, kill_process};

pub struct InstanceGuard {
    file: File,
}

impl InstanceGuard {
    pub fn acquire(name: &str) -> io::Result<Option<Self>> {
        let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "XDG_RUNTIME_DIR is not set",
                )
            })?;

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(runtime_dir.join(format!("{name}.lock")))?;
        let mut guard = Self { file };


        match guard.file.try_lock() {
            Ok(()) => {
                guard.write_pid(std::process::id() as i32)?;
                Ok(Some(guard))
            },
            Err(TryLockError::WouldBlock) => {
                let pid = Self::read_pid(&mut guard)?;
                kill_process(Pid::from_raw(pid).unwrap(), Signal::TERM)?;
                Ok(None)
            },
            Err(TryLockError::Error(error)) => Err(error),
        }
    }

    fn read_pid(&mut self) -> io::Result<i32> {
        self.file.rewind()?;
        let mut contents = String::new();
        self.file.read_to_string(&mut contents)?;
        contents.trim().parse::<i32>().map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid PID"))
    }

    fn write_pid(&mut self, pid: i32) -> io::Result<()> {
        self.file.write_all(&pid.to_string().as_bytes())?;
        Ok(())
    }
}
