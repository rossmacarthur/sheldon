use std::fs;
use std::path::PathBuf;

use anyhow::{Context as ResultExt, Result};
use url::Url;

use crate::context::{LockContext, LockMode};
use crate::lock::source::LockedSource;
use crate::util;
use crate::util::TempPath;

pub fn lock(ctx: &LockContext, dir: PathBuf, file: PathBuf, url: &Url) -> Result<LockedSource> {
    if matches!(ctx.mode, LockMode::Normal) && file.exists() {
        status!(ctx, "Checked", &url);
        return Ok(LockedSource {
            dir,
            file: Some(file),
        });
    }

    let temp_file =
        TempPath::new_force(&file).context("failed to prepare temporary download directory")?;
    {
        let path = temp_file.path();
        fs::create_dir_all(&dir).with_context(s!("failed to create dir `{}`", dir.display()))?;
        let temp_file_handle =
            fs::File::create(path).with_context(s!("failed to create `{}`", path.display()))?;
        util::download(url.as_ref(), temp_file_handle)
            .with_context(s!("failed to download `{}`", url))?;
    }
    temp_file
        .rename(&file)
        .context("failed to rename temporary download file")?;
    status!(ctx, "Fetched", &url);

    Ok(LockedSource {
        dir,
        file: Some(file),
    })
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time;

    use super::*;

    #[test]
    fn lock_remote_and_reinstall() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().expect("create temporary directory");
        let dir = temp.path();
        let file = dir.join("test.txt");
        let mut ctx = LockContext::testing(dir);
        let url =
            Url::parse("https://github.com/rossmacarthur/sheldon/raw/0.3.0/LICENSE-MIT").unwrap();

        let locked = lock(&ctx, dir.to_path_buf(), file.clone(), &url).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, Some(file.clone()));
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            fs::read_to_string(&manifest_dir.join("LICENSE-MIT")).unwrap()
        );

        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        thread::sleep(time::Duration::from_secs(1));
        ctx.mode = LockMode::Reinstall;
        let locked = lock(&ctx, dir.to_path_buf(), file.clone(), &url).unwrap();

        assert_eq!(locked.dir, dir);
        assert_eq!(locked.file, Some(file.clone()));
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            fs::read_to_string(&manifest_dir.join("LICENSE-MIT")).unwrap()
        );
        assert!(fs::metadata(&file).unwrap().modified().unwrap() > modified)
    }
}
