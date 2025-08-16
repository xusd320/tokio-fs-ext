use std::{
    io::{self, SeekFrom},
    path::Path,
    pin::Pin,
    sync::Mutex,
    task::{Context, Poll},
};

use futures::io::{AsyncRead, AsyncSeek, AsyncWrite};
use send_wrapper::SendWrapper;
use web_sys::{FileSystemReadWriteOptions, FileSystemSyncAccessHandle};

use super::{
    OpenOptions,
    metadata::{FileType, Metadata},
    opfs::{OpfsError, SyncAccessMode, open_file},
};

#[derive(Debug)]
pub struct File {
    pub(super) sync_access_handle: SendWrapper<FileSystemSyncAccessHandle>,
    pub(super) pos: Mutex<u64>,
}

impl File {
    pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
        open_file(
            path,
            super::opfs::CreateFileMode::Create,
            SyncAccessMode::Readonly,
            true,
        )
        .await
    }

    pub async fn create_new<P: AsRef<Path>>(path: P) -> std::io::Result<File> {
        open_file(
            &path,
            super::opfs::CreateFileMode::CreateNew,
            SyncAccessMode::Readonly,
            false,
        )
        .await
    }

    pub async fn metadata(&self) -> io::Result<Metadata> {
        Ok(Metadata {
            file_type: FileType::File,
            file_size: self.size()?,
        })
    }

    pub async fn open(path: impl AsRef<Path>) -> io::Result<File> {
        open_file(
            path,
            super::opfs::CreateFileMode::NotCreate,
            SyncAccessMode::Readonly,
            false,
        )
        .await
    }

    #[must_use]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    pub async fn sync_all(&self) -> io::Result<()> {
        self.flush()
    }

    pub async fn sync_data(&self) -> io::Result<()> {
        self.flush()
    }

    pub fn size(&self) -> io::Result<u64> {
        self.sync_access_handle.get_size().map_or_else(
            |err| Err(OpfsError::from(err).into_io_err()),
            |size| Ok(size as u64),
        )
    }
}

impl File {
    pub(crate) fn read_with_buf(&self, buf: &mut [u8]) -> io::Result<u64> {
        let options = FileSystemReadWriteOptions::new();
        options.set_at(*self.pos.lock().unwrap() as f64);
        let size = self
            .sync_access_handle
            .read_with_u8_array_and_options(buf, &options)
            .map_err(|err| OpfsError::from(err).into_io_err())? as u64;
        Ok(size)
    }

    pub(crate) fn write_with_buf(&self, buf: &[u8]) -> io::Result<u64> {
        let options = FileSystemReadWriteOptions::new();
        options.set_at(*self.pos.lock().unwrap() as f64);
        let size = self
            .sync_access_handle
            .write_with_u8_array_and_options(buf.as_ref(), &options)
            .map_err(|err| OpfsError::from(err).into_io_err())? as u64;
        Ok(size)
    }

    pub(crate) fn flush(&self) -> io::Result<()> {
        self.sync_access_handle
            .flush()
            .map_err(|err| OpfsError::from(err).into_io_err())
    }

    pub(crate) fn close(&self) {
        self.sync_access_handle.close();
    }
}

impl Drop for File {
    fn drop(&mut self) {
        self.close();
    }
}

impl AsyncRead for File {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let size = self.read_with_buf(buf)?;

        let mut pos = self.pos.lock().unwrap();
        *pos += size;

        Poll::Ready(Ok(size as usize))
    }
}

impl AsyncWrite for File {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let offset = self.write_with_buf(buf)?;

        let mut pos = self.pos.lock().unwrap();
        *pos += offset;

        Poll::Ready(Ok(offset as usize))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.flush()?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.close();
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for File {
    fn poll_seek(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        position: SeekFrom,
    ) -> Poll<io::Result<u64>> {
        let mut pos = self.pos.lock().unwrap();
        match position {
            SeekFrom::Start(offset) => {
                *pos = offset;
            }
            SeekFrom::End(offset) => {
                *pos = self
                    .size()?
                    .checked_add_signed(offset)
                    .ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
            }
            SeekFrom::Current(offset) => {
                *pos = pos
                    .checked_add_signed(offset)
                    .ok_or(io::Error::from(io::ErrorKind::InvalidInput))?;
            }
        }
        Poll::Ready(Ok(*pos))
    }
}
