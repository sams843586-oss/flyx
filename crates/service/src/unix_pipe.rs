//! Local, group-restricted Unix socket. No network listener and no shell input.
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::time::Duration;
use flexconnect_core::ipc::{MAX_REQUEST, SOCKET_PATH};

fn read_request(stream: &mut UnixStream) -> io::Result<String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    BufReader::new(stream.take((MAX_REQUEST + 1) as u64)).read_until(b'\n', &mut bytes)?;
    if bytes.len() > MAX_REQUEST || bytes.last() != Some(&b'\n') {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid or oversized IPC request"));
    }
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn bind(path: &Path) -> io::Result<UnixListener> {
    if let Ok(metadata) = std::fs::symlink_metadata(path) {
        if !metadata.file_type().is_socket() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "socket path is not a socket"));
        }
        match UnixStream::connect(path) {
            Ok(_) => return Err(io::Error::new(io::ErrorKind::AddrInUse, "service already running")),
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => std::fs::remove_file(path)?,
            Err(e) => return Err(e),
        }
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

pub fn serve(stop: Arc<AtomicBool>, handler: impl Fn(&str) -> String + Send + Sync + 'static) -> io::Result<()> {
    let path = Path::new(SOCKET_PATH);
    let listener = bind(path)?;
    let handler = Arc::new(handler);
    let active = Arc::new(AtomicUsize::new(0));
    let result = (|| {
        while !stop.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    if active.load(Ordering::SeqCst) >= 16 { continue; }
                    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                    active.fetch_add(1, Ordering::SeqCst);
                    let (handler, active) = (handler.clone(), active.clone());
                    std::thread::spawn(move || {
                        if let Ok(line) = read_request(&mut stream) {
                            let response = handler(&line);
                            let _ = writeln!(stream, "{response}");
                        }
                        active.fetch_sub(1, Ordering::SeqCst);
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    })();
    drop(listener);
    std::fs::remove_file(path).ok();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_protocol_rejects_oversized_and_incomplete_lines() {
        for (bytes, valid) in [(b"{\"cmd\":\"status\"}\n".to_vec(), true),
                                (b"no newline".to_vec(), false), (vec![b'x'; MAX_REQUEST + 1], false)] {
            let (mut client, mut server) = UnixStream::pair().unwrap();
            let writer = std::thread::spawn(move || {
                client.write_all(&bytes).unwrap();
                client.shutdown(std::net::Shutdown::Write).unwrap();
            });
            assert_eq!(read_request(&mut server).is_ok(), valid);
            writer.join().unwrap();
        }
    }
}
