//! Output backends for emitting snapshots.

use std::path::PathBuf;

use buswatch_types::Snapshot;

/// Output destination for snapshots.
///
/// Configure where the instrumentor should emit snapshots.
#[derive(Debug, Clone)]
pub enum Output {
    /// Write snapshots to a JSON file.
    ///
    /// The file is overwritten with each snapshot.
    File(PathBuf),

    /// Send snapshots to a TCP server.
    ///
    /// Each snapshot is sent as a newline-delimited JSON message.
    Tcp(String),

    /// Send snapshots through a channel.
    ///
    /// Use `Output::channel()` to create this variant and get the receiver.
    #[cfg(feature = "tokio")]
    Channel(tokio::sync::mpsc::Sender<Snapshot>),
}

impl Output {
    /// Create a file output.
    ///
    /// # Example
    ///
    /// ```rust
    /// use buswatch_sdk::Output;
    ///
    /// let output = Output::file("metrics.json");
    /// ```
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Output::File(path.into())
    }

    /// Create a TCP output.
    ///
    /// # Example
    ///
    /// ```rust
    /// use buswatch_sdk::Output;
    ///
    /// let output = Output::tcp("localhost:9090");
    /// ```
    pub fn tcp(addr: impl Into<String>) -> Self {
        Output::Tcp(addr.into())
    }

    /// Create a channel output and return both the output and receiver.
    ///
    /// This is useful for integrating with your own snapshot handling.
    ///
    /// # Example
    ///
    /// ```rust
    /// use buswatch_sdk::Output;
    ///
    /// let (output, mut rx) = Output::channel(16);
    ///
    /// // Later, receive snapshots
    /// // while let Some(snapshot) = rx.recv().await {
    /// //     println!("Got snapshot with {} modules", snapshot.len());
    /// // }
    /// ```
    #[cfg(feature = "tokio")]
    pub fn channel(buffer: usize) -> (Self, tokio::sync::mpsc::Receiver<Snapshot>) {
        let (tx, rx) = tokio::sync::mpsc::channel(buffer);
        (Output::Channel(tx), rx)
    }

    /// Emit a snapshot to this output.
    #[cfg(feature = "tokio")]
    pub(crate) async fn emit(&self, snapshot: &Snapshot) -> std::io::Result<()> {
        match self {
            Output::File(path) => {
                let json = serde_json::to_string_pretty(snapshot)?;
                tokio::fs::write(path, json).await?;
            }
            Output::Tcp(addr) => {
                use tokio::io::AsyncWriteExt;
                use tokio::net::TcpStream;

                // Try to connect and send (best effort)
                if let Ok(mut stream) = TcpStream::connect(addr).await {
                    let json = serde_json::to_string(snapshot)?;
                    let _ = stream.write_all(json.as_bytes()).await;
                    let _ = stream.write_all(b"\n").await;
                }
            }
            Output::Channel(tx) => {
                // Best effort send (don't block if channel is full)
                let _ = tx.try_send(snapshot.clone());
            }
        }
        Ok(())
    }
}
