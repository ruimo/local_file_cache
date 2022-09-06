use std::{path::{Path, PathBuf}, fs::{File, self}, io::Error};
use std::io::Read;
use std::io::Write;

use dirs;

pub struct LocalFileCache {
    dir: PathBuf,
}

impl LocalFileCache {
    pub fn new<P: AsRef<Path>>(sub_path: P) -> Option<Self> {
        dirs::cache_dir().map(|mut base_dir| {
            base_dir.push(sub_path);
            Self {
                dir: base_dir
            }
        })
    }

    pub fn flush(&self) -> Result<(), Error> {
        match fs::remove_dir_all(&self.dir) {
            Ok(_) => {
                fs::create_dir(&self.dir)
            },
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => return Err(e),
            }
        }
    }

    pub fn or_insert_with<K, F>(&self, k: K, f: F) -> Result<Vec<u8>, Error> where K: AsRef<Path>, F: FnOnce() -> Vec<u8> {
        let mut buf = PathBuf::new();
        buf.push(&self.dir);
        fs::create_dir_all(buf.as_path())?;

        buf.push(k);
        let path = buf.as_path();
    
        let mut fh = match File::open(path) {
            Ok(fh) => Ok(fh),
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    let r = f();
                    Self::save_to(path, &r)?;
                    return Ok(r);
                }
                _ => Err(e),
            },
        }?;
        let mut buffer: Vec<u8> = vec![0; fh.metadata()?.len() as usize];
        fh.read_exact(&mut buffer)?;
        Ok(buffer)
    }

    fn save_to(path: &Path, bytes: &[u8]) -> Result<(), Error> {
        let mut f = File::create(path)?;
        f.write_all(bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_cache() {
        let cache = LocalFileCache::new("my_test").unwrap();
        cache.flush().unwrap();
        let mut called = false;
        let ret = cache.or_insert_with("data0", || {
            called = true;
            vec![123u8]
        }).unwrap();

        assert_eq!(ret, vec![123u8]);
        assert!(called);

        called = false;
        let ret = cache.or_insert_with("data0", || {
            vec![234u8]
        }).unwrap();

        assert_eq!(ret, vec![123u8]);
    }
}
