use crate::{CreateOptions, FileHandle, LocationHandle, WriteOptions};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransferStage {
    Prepare,
    Copy,
    DeleteSource,
}

/// Transfer failure with a handle to any destination that may remain.
/// At `DeleteSource`, the destination is complete; at `Copy`, it may be partial.
#[derive(Debug)]
pub struct TransferError {
    pub stage: TransferStage,
    pub destination: Option<FileHandle>,
    pub cause: anyhow::Error,
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "file transfer failed during {:?}: {}",
            self.stage, self.cause
        )
    }
}

impl std::error::Error for TransferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

impl From<anyhow::Error> for TransferError {
    fn from(cause: anyhow::Error) -> Self {
        Self {
            stage: TransferStage::Prepare,
            destination: None,
            cause,
        }
    }
}

impl FileHandle {
    /// Copy bytes into a newly created destination, using bounded-memory I/O.
    /// Existing files are never replaced; providers may choose a different name.
    /// Awaited failures attempt to delete the new output. Cancellation can leave it behind.
    pub async fn copy_to(
        &self,
        destination: &LocationHandle,
        relative_path: impl Into<String>,
        mut options: CreateOptions,
    ) -> Result<FileHandle, TransferError> {
        let relative_path = relative_path.into();
        crate::location::validate_relative_path(&relative_path)?;
        if options.mime_type.is_none() {
            options.mime_type = self.metadata().await?.mime_type;
        }
        let mut reader = self.open_read().await?;
        let target = destination.create_file(relative_path, options).await?;
        let result: anyhow::Result<()> = async {
            let mut writer = target.open_write(WriteOptions::truncate()).await?;
            let result: anyhow::Result<()> = async {
                while let Some(chunk) = reader.read_chunk().await? {
                    writer.write_all(&chunk).await?;
                }
                Ok(())
            }
            .await;
            match result {
                Ok(()) => writer.close().await,
                Err(error) => match writer.abort().await {
                    Ok(()) => Err(error),
                    Err(cleanup) => {
                        Err(error.context(format!("writer cleanup failed: {cleanup:#}")))
                    }
                },
            }
        }
        .await;
        // Close the input before a subsequent move removes the source (including on Windows).
        let close = reader.close().await;
        let result = match (result, close) {
            (Ok(()), result) => result,
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(close)) => {
                Err(error.context(format!("input cleanup failed: {close:#}")))
            }
        };
        match result {
            Ok(()) => Ok(target),
            Err(error) => match target.delete().await {
                Ok(()) => Err(TransferError {
                    stage: TransferStage::Copy,
                    destination: None,
                    cause: error,
                }),
                Err(cleanup) => Err(TransferError {
                    stage: TransferStage::Copy,
                    destination: Some(target),
                    cause: error.context(format!("destination cleanup failed: {cleanup:#}")),
                }),
            },
        }
    }

    /// Copy and close the destination, then permanently delete the source.
    /// This is not an atomic rename. On delete failure, the completed destination is retained.
    pub async fn move_to(
        &self,
        destination: &LocationHandle,
        relative_path: impl Into<String>,
        options: CreateOptions,
    ) -> Result<FileHandle, TransferError> {
        if !self.can_delete().await? {
            return Err(crate::unsupported("source file does not support deletion").into());
        }
        let target = self.copy_to(destination, relative_path, options).await?;
        self.delete().await.map_err(|cause| TransferError {
            stage: TransferStage::DeleteSource,
            destination: Some(target.clone()),
            cause,
        })?;
        Ok(target)
    }
}
