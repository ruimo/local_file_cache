use std::{path::{Path, PathBuf}, fs::{File, self, OpenOptions}, io::{Error, self}};
use std::io::Read;
use std::io::Write;

use dirs;

pub struct LocalFileCache<T> {
    dir: PathBuf,
    to_u8: Box<dyn Fn(&T) -> Option<Vec<u8>>>,
    from_u8: Box<dyn Fn(&[u8]) -> T>,
}

impl<T> LocalFileCache<T> {
    pub fn new<P: AsRef<Path>>(sub_path: P, to_u8: Box<dyn Fn(&T) -> Option<Vec<u8>>>, from_u8: Box<dyn Fn(&[u8]) -> T>) -> Option<Self> {
        dirs::cache_dir().map(|mut base_dir| {
            base_dir.push(sub_path);
            Self {
                dir: base_dir,
                to_u8, from_u8,
            }
        })
    }

    pub fn invalidate<P: AsRef<Path>>(sub_path: P) -> Option<io::Result<()>> {
        dirs::cache_dir().map(|mut base_dir| {
            base_dir.push(sub_path);
            fs::remove_dir_all(&base_dir)
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
                    if let Some(bin) = (self.to_u8)(&r) {
                        Self::save_to(path, &bin)?;
                    }

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
        // More than one program may save the same cache entry simultaneously.
        // 1) Save file named "xxx.save" with create_new(true). It will be failed if the file with the same name already exists.
        // 2) If the same named file already exists, just skip this method.
        // 3) Otherwise, rename "xxx.save" to "xxx".

        let mut save_path = PathBuf::new();
        save_path.push(path);
        save_path.set_extension("save");

        let mut f = match OpenOptions::new().write(true).create_new(true).open(&save_path) {
            Ok(file) => Ok(file),
            Err(e) => if e.kind() == std::io::ErrorKind::AlreadyExists {
                return Ok(());
            } else { Err(e) }
        }?;
        f.write_all(bytes)?;
        fs::rename(&save_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use rand;

    #[test]
    fn can_cache() {
        let rand: u128 = rand::random();
        let path = format!("local_file_cache_test-{}", rand);

        let test_result = std::panic::catch_unwind(|| {
            let cache = LocalFileCache::<String>::new(&path,
                Box::new(|bin| {
                    Some(vec![bin.parse::<u8>().unwrap()])
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
        });

        LocalFileCache::<()>::invalidate(&path);

        if let Err(e) = test_result {
            std::panic::resume_unwind(e);
        }
    }

    fn read_all_bytes<P: AsRef<Path>>(path: P) -> Vec<u8> {
        let mut f = File::open(path).unwrap();
        let len = f.metadata().unwrap().len();
        let mut buf = vec![0; len as usize];
        f.read_exact(&mut buf).unwrap();
        buf
    }

    #[test]
    fn save_to_skips_if_same_name_exists() {
        let dir = tempdir().unwrap();
        let mut path = dir.path().to_owned();
        path.push("test");

        let mut save_path = dir.path().to_owned();
        save_path.push("test.save");

        LocalFileCache::<()>::save_to(&path, &[12u8]).unwrap();
        assert_eq!(read_all_bytes(&path), vec![12u8]);

        LocalFileCache::<()>::save_to(&path, &[23u8]).unwrap();
        assert_eq!(read_all_bytes(&path), vec![23u8]);

        LocalFileCache::<()>::save_to(&save_path, &[123u8]).unwrap();
        LocalFileCache::<()>::save_to(&path, &[34u8]).unwrap();
        assert_eq!(read_all_bytes(&path), vec![23u8]);
    }
}
