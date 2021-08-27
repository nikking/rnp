use crate::*;
use async_trait::async_trait;
use socket2::{Domain, SockAddr, Socket, Type};
use std::io;
use std::mem::MaybeUninit;
use std::net::{Shutdown, SocketAddr};
use std::time::{Duration, Instant};

pub struct PingClientTcp {
    config: PingClientConfig,
}

impl PingClientTcp {
    pub fn new(config: &PingClientConfig) -> PingClientTcp {
        return PingClientTcp { config: config.clone() };
    }

    #[tracing::instrument(name = "Running TCP ping in ping client", level = "debug", skip(self))]
    fn ping_target(&self, source: &SocketAddr, target: &SocketAddr) -> PingClientResult<PingClientPingResultDetails> {
        let socket = self.prepare_socket_for_ping(source).map_err(|e| PingClientError::PreparationFailed(Box::new(e)))?;

        let start_time = Instant::now();
        let connect_result = socket.connect_timeout(&SockAddr::from(target.clone()), self.config.wait_timeout);
        let rtt = Instant::now().duration_since(start_time);
        match connect_result {
            // Timeout is an expected value instead of an actual failure, so here we should return Ok.
            Err(e) if e.kind() == io::ErrorKind::TimedOut => return Ok(PingClientPingResultDetails::new(None, rtt, true, None)),
            Err(e) => return Err(PingClientError::PingFailed(Box::new(e))),
            Ok(()) => (),
        }
        let local_addr = socket.local_addr();

        // Check closing connection as well as opening connection
        let mut warning: Option<PingClientWarning> = None;
        if self.config.check_disconnect {
            warning = match self.shutdown_connection(socket) {
                Err(e) => Some(PingClientWarning::DisconnectFailed(Box::new(e))),
                Ok(_) => None,
            }
        }

        // If getting local address failed, we ignore it.
        // The worse case we can get is to output a 0.0.0.0 as source IP, which is not critical to what we are trying to do.
        return match local_addr {
            Ok(addr) => Ok(PingClientPingResultDetails::new(Some(addr.as_socket().unwrap()), rtt, false, warning)),
            Err(_) => Ok(PingClientPingResultDetails::new(None, rtt, false, warning)),
        };
    }

    fn prepare_socket_for_ping(&self, source: &SocketAddr) -> io::Result<Socket> {
        let socket_domain = if source.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
        let socket = Socket::new(socket_domain, Type::STREAM, None)?;
        socket.bind(&SockAddr::from(source.clone()))?;
        socket.set_read_timeout(Some(self.config.wait_timeout))?;
        if !self.config.check_disconnect {
            socket.set_linger(Some(Duration::from_secs(0)))?;
        }
        if let Some(ttl) = self.config.time_to_live {
            socket.set_ttl(ttl)?;
        }

        return Ok(socket);
    }

    fn shutdown_connection(&self, socket: Socket) -> io::Result<()> {
        socket.shutdown(Shutdown::Write)?;

        // Try to read until recv returns nothing, which indicates shutdown is succeeded.
        let mut buf: [MaybeUninit<u8>; 128] = unsafe { MaybeUninit::uninit().assume_init() };
        while socket.recv(&mut buf)? > 0 {
            continue;
        }

        return Ok(());
    }
}

#[async_trait]
impl PingClient for PingClientTcp {
    fn protocol(&self) -> &'static str {
        "TCP"
    }

    async fn prepare_ping(&mut self, _: &SocketAddr) -> Result<(), PingClientError> {
        Ok(())
    }

    async fn ping(&self, source: &SocketAddr, target: &SocketAddr) -> PingClientResult<PingClientPingResultDetails> {
        return self.ping_target(source, target);
    }
}

#[cfg(test)]
mod tests {
    use crate::ping_clients::ping_client_test_common::*;
    use crate::stub_servers::stub_server_factory;
    use crate::{ping_clients::ping_client_factory, PingClientConfig, RnpStubServerConfig, RnpSupportedProtocol};
    use futures_intrusive::sync::ManualResetEvent;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::runtime::Runtime;

    #[test]
    fn ping_client_tcp_should_work() {
        let rt = Runtime::new().unwrap();

        let ready_event = Arc::new(ManualResetEvent::new(false));
        let ready_event_clone = ready_event.clone();
        let _server = rt.spawn(async move {
            let stub_server_config = RnpStubServerConfig {
                protocol: RnpSupportedProtocol::TCP,
                server_address: "127.0.0.1:11337".parse().unwrap(),
                sleep_before_write: Duration::from_millis(0),
                write_chunk_size: 1024,
                write_count_limit: Some(0),
                report_interval: Duration::from_secs(1),
            };
            let stub_server = stub_server_factory::run(&stub_server_config, Arc::new(ManualResetEvent::new(false)), ready_event_clone);
            return stub_server.await;
        });
        rt.block_on(ready_event.wait());

        rt.block_on(async {
            let config = PingClientConfig {
                wait_timeout: Duration::from_millis(300),
                time_to_live: None,
                check_disconnect: false,
                server_name: None,
                log_tls_key: false,
                alpn_protocol: None,
                use_timer_rtt: false,
            };
            let mut ping_client = ping_client_factory::new_ping_client(&RnpSupportedProtocol::TCP, &config, None);

            // When connecting to a non-existing port, on windows, it will timeout, but on other *nix OS, it will reject the connection.
            let expected_results = ExpectedPingClientTestResults {
                timeout_min_time: Duration::from_millis(200),
                ping_non_existing_host_result: ExpectedTestCaseResult::Timeout,
                ping_non_existing_port_result: if cfg!(windows) {
                    ExpectedTestCaseResult::Timeout
                } else {
                    ExpectedTestCaseResult::Failed("connection refused")
                },
                binding_invalid_source_ip_result: ExpectedTestCaseResult::Failed(
                    "The requested address is not valid in its context. (os error 10049)",
                ),
                binding_unavailable_source_port_result: ExpectedTestCaseResult::Failed(
                    "Only one usage of each socket address (protocol/network address/port) is normally permitted. (os error 10048)",
                ),
            };

            run_ping_client_tests(&mut ping_client, &"127.0.0.1:11337".parse().unwrap(), &expected_results).await;
        });
    }
}