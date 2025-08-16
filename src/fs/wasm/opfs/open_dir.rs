use std::{
    borrow::Cow,
    io,
    path::{Component, Path},
};

use send_wrapper::SendWrapper;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{FileSystemDirectoryHandle, FileSystemGetDirectoryOptions};

use super::{
    OpfsError,
    dir_handle_cache::{get_cached_dir_handle, set_cached_dir_handle},
    options::OpenDirType,
    root::root,
    virtualize,
};

pub(crate) async fn open_dir(
    path: impl AsRef<Path>,
    r#type: OpenDirType,
) -> io::Result<SendWrapper<FileSystemDirectoryHandle>> {
    let virt = virtualize::virtualize(path)?;

    tracing::debug!("open_dir: {} {type:?}", virt.to_string_lossy());

    if let Some(handle) = get_cached_dir_handle(&virt) {
        return Ok(handle);
    }

    let components: Vec<Cow<'_, str>> = virt
        .components()
        .filter_map(|c| match c {
            Component::Normal(c) => Some(c.to_string_lossy()),
            _ => None,
        })
        .collect();

    let total_depth = components.len();

    let mut dir_handle = root().await?;

    let mut found = 0_usize;

    for c in components.iter() {
        let cur_virt = virt.join(c.as_ref());
        dir_handle = if let Some(handle) = get_cached_dir_handle(&cur_virt) {
            handle
        } else {
            let dir_handle = get_dir_handle(
                &dir_handle,
                c,
                matches!(r#type, OpenDirType::Create | OpenDirType::CreateRecursive),
            )
            .await?;

            set_cached_dir_handle(cur_virt, dir_handle.clone());
            dir_handle
        };

        found += 1;
    }

    if found != total_depth {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }

    set_cached_dir_handle(virt, dir_handle.clone());

    Ok(dir_handle)
}

async fn get_dir_handle(
    parent: &SendWrapper<FileSystemDirectoryHandle>,
    path: &str,
    create: bool,
) -> io::Result<SendWrapper<FileSystemDirectoryHandle>> {
    let options = SendWrapper::new(FileSystemGetDirectoryOptions::new());
    options.set_create(create);

    let dir_handle = SendWrapper::new(JsFuture::from(
        parent.get_directory_handle_with_options(path, &options),
    ))
    .await
    .map_err(|err| OpfsError::from(err).into_io_err())?
    .unchecked_into::<FileSystemDirectoryHandle>();
    Ok(SendWrapper::new(dir_handle))
}
