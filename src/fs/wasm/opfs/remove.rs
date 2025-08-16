use std::{io, path::Path};

use send_wrapper::SendWrapper;
use wasm_bindgen_futures::JsFuture;
use web_sys::FileSystemRemoveOptions;

use super::{
    OpenDirType, OpfsError, dir_handle_cache::remove_cached_dir_handle, open_dir, root::root,
    virtualize,
};

pub(crate) async fn remove(path: impl AsRef<Path>, recursive: bool) -> io::Result<()> {
    let virt = virtualize::virtualize(path)?;

    tracing::debug!("remove: {} {recursive}", virt.to_string_lossy());

    let parent = virt.parent();

    let name = match virt.file_name() {
        Some(os_str) => Ok(os_str.to_string_lossy()),
        None => Err(io::Error::from(io::ErrorKind::InvalidFilename)),
    }?;

    let dir_entry = match parent {
        Some(parent) => open_dir(parent, OpenDirType::NotCreate).await?,
        None => root().await?,
    };

    let options = SendWrapper::new(FileSystemRemoveOptions::new());
    options.set_recursive(recursive);

    SendWrapper::new(JsFuture::from(
        dir_entry.remove_entry_with_options(&name, &options),
    ))
    .await
    .map_err(|err| OpfsError::from(err).into_io_err())?;

    remove_cached_dir_handle(&virt, recursive);

    Ok(())
}
