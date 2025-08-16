use std::{io, path::Path};

use web_sys::FileSystemHandleKind;

use super::opfs::{OpfsError, open_dir, open_file};

/// Symlink is not supported.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    #[default]
    File,
    Directory,
    // TODO:
    Symlink,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        *self == Self::Directory
    }

    pub fn is_file(&self) -> bool {
        *self == Self::File
    }

    pub fn is_symlink(&self) -> bool {
        *self == Self::Symlink
    }
}

impl From<&FileSystemHandleKind> for FileType {
    fn from(handle: &FileSystemHandleKind) -> Self {
        match handle {
            FileSystemHandleKind::File => FileType::File,
            FileSystemHandleKind::Directory => FileType::Directory,
            _ => todo!(),
        }
    }
}

impl From<FileSystemHandleKind> for FileType {
    fn from(handle: FileSystemHandleKind) -> Self {
        (&handle).into()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    pub(crate) file_type: FileType,
    pub(crate) file_size: u64,
}

impl Metadata {
    pub fn is_dir(&self) -> bool {
        self.file_type.is_dir()
    }

    pub fn is_file(&self) -> bool {
        self.file_type.is_file()
    }

    pub fn is_symlink(&self) -> bool {
        self.file_type.is_symlink()
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.file_size
    }
}

pub async fn metadata(path: impl AsRef<Path>) -> io::Result<Metadata> {
    match open_file(
        &path,
        super::opfs::CreateFileMode::NotCreate,
        super::opfs::SyncAccessMode::Readonly,
        false,
    )
    .await
    {
        Ok(file) => {
            let len = file
                .sync_access_handle
                .get_size()
                .map_err(|err| OpfsError::from(err).into_io_err())? as u64;
            Ok(Metadata {
                file_type: FileType::File,
                file_size: len,
            })
        }
        Err(_) => Ok(open_dir(path, super::opfs::OpenDirType::NotCreate)
            .await
            .map(|_| Metadata {
                file_type: FileType::Directory,
                file_size: 0,
            })?),
    }
}
