use std::{io, path::Path, sync::Mutex};

use js_sys::{Function, Promise, Reflect};
use send_wrapper::SendWrapper;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetFileOptions,
    FileSystemSyncAccessHandle,
};

use super::{
    super::File,
    OpenDirType,
    error::OpfsError,
    open_dir,
    options::{CreateFileMode, CreateSyncAccessHandleOptions, SyncAccessMode},
    root::root,
    virtualize,
};

pub(crate) async fn open_file(
    path: impl AsRef<Path>,
    create: CreateFileMode,
    mode: SyncAccessMode,
    truncate: bool,
) -> io::Result<File> {
    let virt = virtualize::virtualize(&path)?;

    tracing::debug!(
        "open file: {} {create:?} {mode:?} truncate: {truncate}",
        virt.to_string_lossy(),
    );

    let parent = virt.parent();

    let name = match virt.file_name() {
        Some(os_str) => Ok(os_str.to_string_lossy()),
        None => Err(io::Error::from(io::ErrorKind::InvalidFilename)),
    }?;

    let dir_entry = match parent {
        Some(parent_path) => open_dir(parent_path, OpenDirType::NotCreate).await?,
        None => root().await?,
    };

    let sync_access_handle = match create {
        CreateFileMode::Create => get_file_handle(&name, &dir_entry, mode, true, truncate).await?,
        CreateFileMode::CreateNew => {
            match get_file_handle(&name, &dir_entry, mode, false, truncate).await {
                Ok(_) => {
                    return Err(io::Error::from(io::ErrorKind::AlreadyExists));
                }
                Err(_) => get_file_handle(&name, &dir_entry, mode, true, truncate).await?,
            }
        }
        CreateFileMode::NotCreate => {
            get_file_handle(&name, &dir_entry, mode, false, truncate).await?
        }
    };
    Ok(File {
        sync_access_handle,
        pos: Mutex::new(0),
    })
}

async fn get_file_handle(
    name: &str,
    dir_entry: &SendWrapper<FileSystemDirectoryHandle>,
    mode: SyncAccessMode,
    create: bool,
    truncate: bool,
) -> Result<SendWrapper<FileSystemSyncAccessHandle>, io::Error> {
    let option = SendWrapper::new(FileSystemGetFileOptions::new());
    option.set_create(create);
    let file_handle = SendWrapper::new(JsFuture::from(
        dir_entry.get_file_handle_with_options(name, &option),
    ))
    .await
    .map_err(|err| OpfsError::from(err).into_io_err())?
    .unchecked_into::<FileSystemFileHandle>();

    let file_handle_js_value = SendWrapper::new(JsValue::from(file_handle));

    let promise = Reflect::get(&file_handle_js_value, &"createSyncAccessHandle".into())
        .map_err(|err| OpfsError::from(err).into_io_err())?
        .unchecked_into::<Function>()
        .call1(
            &file_handle_js_value,
            &CreateSyncAccessHandleOptions::from(mode).into(),
        )
        .map_err(|err| OpfsError::from(err).into_io_err())?
        .unchecked_into::<Promise>();

    let sync_access_handle = SendWrapper::new(JsFuture::from(promise))
        .await
        .map_err(|err| OpfsError::from(err).into_io_err())?
        .unchecked_into::<FileSystemSyncAccessHandle>();

    if truncate {
        sync_access_handle
            .truncate_with_u32(0)
            .map_err(|err| OpfsError::from(err).into_io_err())?;
    }
    Ok(SendWrapper::new(sync_access_handle))
}
