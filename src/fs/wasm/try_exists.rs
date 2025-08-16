use std::io;

use super::opfs::{CreateFileMode, SyncAccessMode, open_dir, open_file};

pub async fn try_exists(path: impl AsRef<std::path::Path>) -> io::Result<bool> {
    Ok(open_file(
        &path,
        CreateFileMode::NotCreate,
        SyncAccessMode::Readonly,
        false,
    )
    .await
    .is_ok()
        || open_dir(path, super::opfs::OpenDirType::NotCreate)
            .await
            .is_ok())
}
