use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Clone)]
pub struct TestDirs {
    pub home: Rc<tempfile::TempDir>,
    pub config: PathBuf,
    pub data: PathBuf,
}

impl TestDirs {
    pub fn new(config: impl AsRef<Path>, data: impl AsRef<Path>) -> io::Result<Self> {
        let home = Rc::new(tempfile::tempdir()?);
        let config = home.path().join(config);
        let data = home.path().join(data);
        fs::create_dir_all(&config)?;
        fs::create_dir_all(&data)?;
        Ok(Self { home, config, data })
    }

    pub fn default() -> io::Result<Self> {
        Self::new(".sheldon", ".sheldon")
    }

    pub fn default_xdg() -> io::Result<Self> {
        Self::new(".config/sheldon", ".local/share/sheldon")
    }

    pub fn assert_conforms(&self) {
        assert!(self.config.join("plugins.toml").exists());
        assert!(self.data.join("plugins.lock").exists());
        assert!(self.data.join("repos").exists());
        assert!(self.data.join("downloads").exists());
    }
}
