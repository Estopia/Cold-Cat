use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::protocol::{CommandRequest, CommandResponse};
use crate::{daemon_addr, ColdCatError, Result};

pub fn send_command(request: CommandRequest) -> Result<CommandResponse> {
    let address = daemon_addr();
    send_command_to(&address, request, Duration::from_secs(2))
}

pub fn send_command_to(
    address: &str,
    request: CommandRequest,
    timeout: Duration,
) -> Result<CommandResponse> {
    let addr = address.to_socket_addrs()?.next().ok_or_else(|| {
        ColdCatError::Protocol(format!("no daemon address resolved for {address}"))
    })?;

    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|err| ColdCatError::Io(format!("connecting to daemon at {address}: {err}")))?;
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.trim().is_empty() {
        return Err(ColdCatError::Protocol(
            "daemon returned an empty response".to_string(),
        ));
    }

    Ok(serde_json::from_str(&line)?)
}
