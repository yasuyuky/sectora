use std::fs::{create_dir_all, remove_file, File, OpenOptions};
use std::io::{BufReader, Error, Read, Seek, SeekFrom, Write};

const RUN_DIR: &str = "/var/run/sectora";

pub fn create(pid: u32) -> Result<File, Error> {
    create_dir_all(RUN_DIR)?;
    let mut idx_file: File = File::create(format!("{}/{}.index", RUN_DIR, pid))?;
    idx_file.write_all(b"0")?;
    Ok(File::create(format!("{}/{}.list", RUN_DIR, pid))?)
}

pub fn open(pid: u32) -> Result<(usize, File, BufReader<File>), Error> {
    let mut idx_file: File = OpenOptions::new().read(true)
                                               .write(true)
                                               .open(format!("{}/{}.index", RUN_DIR, pid))?;
    let mut idx_string = String::new();
    idx_file.read_to_string(&mut idx_string)?;
    let idx: usize = idx_string.parse().unwrap();
    let list = BufReader::new(File::open(format!("{}/{}.list", RUN_DIR, pid))?);
    Ok((idx, idx_file, list))
}

pub fn cleanup(pid: u32) -> Result<(), Error> {
    remove_file(format!("{}/{}.index", RUN_DIR, pid))?;
    remove_file(format!("{}/{}.list", RUN_DIR, pid))?;
    Ok(())
}

pub fn increment(idx: usize, mut idx_file: File) {
    idx_file.seek(SeekFrom::Start(0)).unwrap();
    idx_file.write_all(format!("{}", idx + 1).as_bytes()).unwrap();
}
