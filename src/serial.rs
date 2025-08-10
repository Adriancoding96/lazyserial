use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

pub use serialport::{SerialPort, SerialPortInfo};

#[derive(Debug)]
pub enum SerialEvent {
    Opened,
    Data(Vec<u8>),
    Error(String),
    Closed,
}

pub struct SerialHandle {
    tx: Sender<Vec<u8>>,
    close_tx: Sender<()>,
}

impl SerialHandle {
    pub fn write(&self, data: Vec<u8>) -> Result<()> {
        self.tx
            .send(data)
            .map_err(|e| anyhow::anyhow!("writer disconnected: {e}"))
    }

    pub fn close(self) -> Result<()> {
        let _ = self.close_tx.send(());
        Ok(())
    }
}

pub fn list_ports() -> Result<Vec<SerialPortInfo>> {
    let ports = serialport::available_ports().context("list available ports")?;
    Ok(ports)
}

pub fn open_port(path: &str, baud_rate: u32) -> Result<(SerialHandle, Receiver<SerialEvent>)> {
    let (event_tx, event_rx) = mpsc::channel::<SerialEvent>();
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>();
    let (close_tx, close_rx) = mpsc::channel::<()>();

    let path_string = path.to_string();

    thread::spawn(move || {
        let builder = serialport::new(path_string.clone(), baud_rate)
            .timeout(Duration::from_millis(50));
        match builder.open() {
            Ok(mut port) => {
                let _ = event_tx.send(SerialEvent::Opened);

                loop {
                    match write_rx.try_recv() {
                        Ok(data) => {
                            if let Err(e) = port.write_all(&data) {
                                let _ = event_tx.send(SerialEvent::Error(format!(
                                    "write error: {}",
                                    e
                                )));
                            }
                        }
                        Err(mpsc::TryRecvError::Empty) => {}
                        Err(mpsc::TryRecvError::Disconnected) => break,
                    }

                    let mut buf = [0u8; 4096];
                    match port.read(&mut buf) {
                        Ok(n) if n > 0 => {
                            let _ = event_tx.send(SerialEvent::Data(buf[..n].to_vec()));
                        }
                        Ok(_) => {}
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            let _ = event_tx.send(SerialEvent::Error(format!("read error: {}", e)));
                            break;
                        }
                    }

                    if close_rx.try_recv().is_ok() {
                        break;
                    }
                }

                let _ = event_tx.send(SerialEvent::Closed);
            }
            Err(e) => {
                let _ = event_tx.send(SerialEvent::Error(format!(
                    "failed to open {}: {}",
                    path_string, e
                )));
            }
        }
    });

    let handle = SerialHandle {
        tx: write_tx,
        close_tx,
    };
    Ok((handle, event_rx))
}


