use std::{path::{Path, PathBuf}, fs::{File, self}, io::Error};
use std::io::Read;
use std::io::Write;

use dirs;

pub struct LocalFileCache<T> {
    dir: PathBuf,
    to_u8: Box<dyn Fn(&T) -> Vec<u8>>,
    from_u8: Box<dyn Fn(&[u8]) -> T>,
}

impl<T> LocalFileCache<T> {
    pub fn new<P: AsRef<Path>>(sub_path: P, to_u8: Box<dyn Fn(&T) -> Vec<u8>>, from_u8: Box<dyn Fn(&[u8]) -> T>) -> Option<Self> {
        dirs::cache_dir().map(|mut base_dir| {
            base_dir.push(sub_path);
            Self {
                dir: base_dir,
                to_u8, from_u8,
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

    pub fn or_insert_with<K, F>(&self, k: K, f: F) -> Result<T, Error>
        where K: AsRef<Path>, F: FnOnce() -> T
    {
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
                    Self::save_to(path, &(self.to_u8)(&r))?;
                    return Ok(r);
                }
                _ => Err(e),
            },
        }?;
        let mut buffer: Vec<u8> = vec![0; fh.metadata()?.len() as usize];
        fh.read_exact(&mut buffer)?;
        Ok((self.from_u8)(&buffer))
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
        let cache = LocalFileCache::<String>::new("my_test",
            Box::new(|bin| {
                vec![bin.parse::<u8>().unwrap()]
            }),
            Box::new(|data| {
                format!("{}", data[0])
            }),
        ).unwrap();
        cache.flush().unwrap();
        let mut called = false;
        let ret = cache.or_insert_with("data0", || {
            called = true;
            "123".to_owned()
        }).unwrap();

        assert_eq!(ret, "123".to_owned());
        assert!(called);

        called = false;
        let ret = cache.or_insert_with("data0", || {
            "234".to_owned()
        }).unwrap();

        assert_eq!(ret, "123".to_owned());
    }
}
