use crate::ping_clients::ping_client::PingClientError::{self, PingFailed, PreparationFailed};
use chrono::{offset::Utc, DateTime};
use std::{net::SocketAddr, time::Duration};

#[derive(Debug)]
pub struct PingResult {
    ping_time: DateTime<Utc>,
    worker_id: u32,
    protocol: &'static str,
    target: SocketAddr,
    source: SocketAddr,
    is_warmup: bool,
    round_trip_time: Duration,
    is_timed_out: bool,
    error: Option<PingClientError>,
}

impl PingResult {
    pub fn new(
        time: &DateTime<Utc>,
        worker_id: u32,
        protocol: &'static str,
        target: SocketAddr,
        source: SocketAddr,
        is_warmup: bool,
        round_trip_time: Duration,
        is_timed_out: bool,
        error: Option<PingClientError>,
    ) -> PingResult {
        PingResult {
            ping_time: time.clone(),
            worker_id,
            protocol,
            target,
            source,
            is_warmup,
            round_trip_time,
            is_timed_out,
            error,
        }
    }

    pub fn ping_time(&self) -> &DateTime<Utc> {
        &self.ping_time
    }
    pub fn worker_id(&self) -> u32 {
        self.worker_id
    }
    pub fn protocol(&self) -> &'static str {
        self.protocol
    }
    pub fn target(&self) -> SocketAddr {
        self.target
    }
    pub fn source(&self) -> SocketAddr {
        self.source
    }
    pub fn is_warmup(&self) -> bool {
        self.is_warmup
    }
    pub fn round_trip_time(&self) -> Duration {
        self.round_trip_time
    }
    pub fn is_timed_out(&self) -> bool {
        self.is_timed_out
    }
    pub fn error(&self) -> &Option<PingClientError> {
        &self.error
    }
    pub fn is_preparation_error(&self) -> bool {
        if let Some(PreparationFailed(_)) = self.error() { true } else { false }
    }

    pub fn format_as_console_log(&self) -> String {
        let warmup_sign = if self.is_warmup() { " (warmup)" } else { "" };

        if self.is_timed_out() {
            return format!(
                "Reaching {} {} from {}{} failed: Timed out, RTT = {:.2}ms",
                self.protocol(),
                self.target(),
                self.source(),
                warmup_sign,
                self.round_trip_time().as_micros() as f64 / 1000.0,
            );
        }

        if let Some(error) = self.error() {
            return match error {
                PreparationFailed(e) => {
                    format!(
                            "Unable to perform ping to {} {} from {}{}, because failing to prepare local socket: Error = {}",
                            self.protocol(),
                            self.target(),
                            self.source(),
                            warmup_sign,
                            e)
                }

                PingFailed(e) => {
                    format!(
                        "Reaching {} {} from {}{} failed: {}",
                        self.protocol(),
                        self.target(),
                        self.source(),
                        warmup_sign,
                        e,
                    )
                }
            };
        }

        return format!(
            "Reaching {} {} from {}{} succeeded: RTT={:.2}ms",
            self.protocol(),
            self.target(),
            self.source(),
            warmup_sign,
            self.round_trip_time().as_micros() as f64 / 1000.0,
        );
    }

    pub fn format_as_json_string(&self) -> String {
        let error_message = match self.error() {
            Some(e) => format!("{}", e.to_string()),
            None => String::from(""),
        };

        let json = format!(
            "{{\"utcTime\":\"{:?}\",\"protocol\":\"{}\",\"workerId\":{},\"targetIP\":\"{}\",\"targetPort\":\"{}\",\"sourceIP\":\"{}\",\"sourcePort\":\"{}\",\"isWarmup\":\"{}\",\"roundTripTimeInMs\":{:.2},\"isTimedOut\":\"{}\",\"error\":\"{}\",\"isPreparationError\":\"{}\"}}",
            self.ping_time(),
            self.protocol(),
            self.worker_id(),
            self.target().ip(),
            self.target().port(),
            self.source().ip(),
            self.source().port(),
            self.is_warmup(),
            self.round_trip_time().as_micros() as f64 / 1000.0,
            self.is_timed_out(),
            error_message,
            self.is_preparation_error(),
        );

        return json;
    }

    pub fn format_as_csv_string(&self) -> String {
        let error_message = match self.error() {
            Some(e) => format!("{}", e.to_string()),
            None => String::from(""),
        };

        let csv = format!(
            "{:?},{},{},{},{},{},{},{},{:.2},{},\"{}\",{}",
            self.ping_time(),
            self.worker_id(),
            self.protocol(),
            self.target().ip(),
            self.target().port(),
            self.source().ip(),
            self.source().port(),
            self.is_warmup(),
            self.round_trip_time().as_micros() as f64 / 1000.0,
            self.is_timed_out(),
            error_message,
            self.is_preparation_error(),
        );

        return csv;
    }
}

#[cfg(test)]
mod tests {
    use crate::ping_result::PingResult;
    use chrono::prelude::*;
    use chrono::Utc;
    use pretty_assertions::assert_eq;
    use std::io;
    use std::net::SocketAddr;
    use std::time::Duration;
    use crate::ping_clients::ping_client::PingClientError::{PreparationFailed, PingFailed};

    #[test]
    fn new_ping_result_should_work() {
        let r = PingResult::new(
            &Utc::now(),
            1,
            "TCP",
            "1.2.3.4:443".parse().unwrap(),
            "5.6.7.8:8080".parse().unwrap(),
            true,
            Duration::from_millis(10),
            false,
            None,
        );

        assert_eq!(1, r.worker_id());
        assert_eq!("TCP", r.protocol());
        assert_eq!("1.2.3.4:443".parse::<SocketAddr>().unwrap(), r.target());
        assert_eq!("5.6.7.8:8080".parse::<SocketAddr>().unwrap(), r.source());
        assert!(r.is_warmup());
        assert_eq!(Duration::from_millis(10), r.round_trip_time());
        assert!(r.error().is_none());
    }

    #[test]
    fn format_ping_result_as_log_should_work() {
        let results = generate_test_samples();
        assert_eq!(
            vec![
                "Reaching TCP 1.2.3.4:443 from 5.6.7.8:8080 (warmup) succeeded: RTT=10.00ms",
                "Reaching TCP 1.2.3.4:443 from 5.6.7.8:8080 failed: Timed out, RTT = 1000.00ms",
                "Reaching TCP 1.2.3.4:443 from 5.6.7.8:8080 failed: connect failed",
                "Unable to perform ping to TCP 1.2.3.4:443 from 5.6.7.8:8080, because failing to prepare local socket: Error = address in use",
            ],
            results
                .into_iter()
                .map(|x| x.format_as_console_log())
                .collect::<Vec<String>>()
        );
    }

    #[test]
    fn format_ping_result_as_json_should_work() {
        let results = generate_test_samples();
        assert_eq!(
            vec![
                "{\"utcTime\":\"2021-07-06T09:10:11.012Z\",\"protocol\":\"TCP\",\"workerId\":1,\"targetIP\":\"1.2.3.4\",\"targetPort\":\"443\",\"sourceIP\":\"5.6.7.8\",\"sourcePort\":\"8080\",\"isWarmup\":\"true\",\"roundTripTimeInMs\":10.00,\"isTimedOut\":\"false\",\"error\":\"\",\"isPreparationError\":\"false\"}",
                "{\"utcTime\":\"2021-07-06T09:10:11.012Z\",\"protocol\":\"TCP\",\"workerId\":1,\"targetIP\":\"1.2.3.4\",\"targetPort\":\"443\",\"sourceIP\":\"5.6.7.8\",\"sourcePort\":\"8080\",\"isWarmup\":\"false\",\"roundTripTimeInMs\":1000.00,\"isTimedOut\":\"true\",\"error\":\"\",\"isPreparationError\":\"false\"}",
                "{\"utcTime\":\"2021-07-06T09:10:11.012Z\",\"protocol\":\"TCP\",\"workerId\":1,\"targetIP\":\"1.2.3.4\",\"targetPort\":\"443\",\"sourceIP\":\"5.6.7.8\",\"sourcePort\":\"8080\",\"isWarmup\":\"false\",\"roundTripTimeInMs\":0.00,\"isTimedOut\":\"false\",\"error\":\"ping failed: connect failed\",\"isPreparationError\":\"false\"}",
                "{\"utcTime\":\"2021-07-06T09:10:11.012Z\",\"protocol\":\"TCP\",\"workerId\":1,\"targetIP\":\"1.2.3.4\",\"targetPort\":\"443\",\"sourceIP\":\"5.6.7.8\",\"sourcePort\":\"8080\",\"isWarmup\":\"false\",\"roundTripTimeInMs\":0.00,\"isTimedOut\":\"false\",\"error\":\"preparation failed: address in use\",\"isPreparationError\":\"true\"}",
            ],
            results.into_iter().map(|x| x.format_as_json_string()).collect::<Vec<String>>()
        );
    }

    #[test]
    fn format_ping_result_as_csv_should_work() {
        let results = generate_test_samples();
        assert_eq!(
            vec![
                "2021-07-06T09:10:11.012Z,1,TCP,1.2.3.4,443,5.6.7.8,8080,true,10.00,false,\"\",false",
                "2021-07-06T09:10:11.012Z,1,TCP,1.2.3.4,443,5.6.7.8,8080,false,1000.00,true,\"\",false",
                "2021-07-06T09:10:11.012Z,1,TCP,1.2.3.4,443,5.6.7.8,8080,false,0.00,false,\"ping failed: connect failed\",false",
                "2021-07-06T09:10:11.012Z,1,TCP,1.2.3.4,443,5.6.7.8,8080,false,0.00,false,\"preparation failed: address in use\",true",
            ],
            results
                .into_iter()
                .map(|x| x.format_as_csv_string())
                .collect::<Vec<String>>()
        );
    }

    fn generate_test_samples() -> Vec<PingResult> {
        vec![
            PingResult::new(
                &Utc.ymd(2021, 7, 6).and_hms_milli(9, 10, 11, 12),
                1,
                "TCP",
                "1.2.3.4:443".parse().unwrap(),
                "5.6.7.8:8080".parse().unwrap(),
                true,
                Duration::from_millis(10),
                false,
                None,
            ),
            PingResult::new(
                &Utc.ymd(2021, 7, 6).and_hms_milli(9, 10, 11, 12),
                1,
                "TCP",
                "1.2.3.4:443".parse().unwrap(),
                "5.6.7.8:8080".parse().unwrap(),
                false,
                Duration::from_millis(1000),
                true,
                None,
            ),
            PingResult::new(
                &Utc.ymd(2021, 7, 6).and_hms_milli(9, 10, 11, 12),
                1,
                "TCP",
                "1.2.3.4:443".parse().unwrap(),
                "5.6.7.8:8080".parse().unwrap(),
                false,
                Duration::from_millis(0),
                false,
                Some(PingFailed(Box::new(io::Error::new(io::ErrorKind::ConnectionRefused, "connect failed")))),
            ),
            PingResult::new(
                &Utc.ymd(2021, 7, 6).and_hms_milli(9, 10, 11, 12),
                1,
                "TCP",
                "1.2.3.4:443".parse().unwrap(),
                "5.6.7.8:8080".parse().unwrap(),
                false,
                Duration::from_millis(0),
                false,
                Some(PreparationFailed(Box::new(io::Error::new(io::ErrorKind::AddrInUse, "address in use")))),
            ),
        ]
    }
}
