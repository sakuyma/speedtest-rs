//! HTTP helpers built on a pooled `ureq` agent.

use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, Instant};

pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub fn new(user_agent: &str) -> Http {
        let agent = ureq::AgentBuilder::new()
            .user_agent(user_agent)
            .max_idle_connections(64)
            .max_idle_connections_per_host(64)
            .build();
        Http { agent }
    }

    pub fn get_string(&self, url: &str, timeout: Duration) -> Result<(u16, String), String> {
        match self.agent.get(url).timeout(timeout).call() {
            Ok(resp) => {
                let code = resp.status();
                let body = resp.into_string().map_err(|e| e.to_string())?;
                Ok((code, body))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                Ok((code, body))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn post_json(&self, url: &str, body: &str, timeout: Duration) -> Result<(u16, String), String> {
        match self
            .agent
            .post(url)
            .set("Content-Type", "application/json")
            .timeout(timeout)
            .send_string(body)
        {
            Ok(resp) => {
                let code = resp.status();
                let text = resp.into_string().map_err(|e| e.to_string())?;
                Ok((code, text))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let text = resp.into_string().unwrap_or_default();
                Ok((code, text))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Repeatedly GET `url`, discarding the body, until `stop` or `deadline`.
    /// `live` is incremented by every received chunk for progress reporting.
    /// Returns the number of body bytes received by this caller.
    pub fn download_for(
        &self,
        url: &str,
        stop: &AtomicBool,
        deadline: Instant,
        live: &AtomicI64,
    ) -> i64 {
        let mut total: i64 = 0;
        // 256 KiB read buffer: fewer syscalls and better throughput on fast links.
        let mut buf = vec![0u8; 1 << 18];
        while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
            let resp = match self.agent.get(url).timeout(Duration::from_secs(20)).call() {
                Ok(r) => r,
                Err(_) => break,
            };
            let mut reader = resp.into_reader();
            let mut got: i64 = 0;
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        got += n as i64;
                        live.fetch_add(n as i64, Ordering::Relaxed);
                    }
                    Err(_) => break,
                }
            }
            total += got;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if got == 0 {
                break;
            }
        }
        total
    }

    /// Repeatedly POST `buffer` to `url` until `stop` or `deadline`.
    /// Returns the number of body bytes sent by this caller.
    pub fn upload_for(
        &self,
        url: &str,
        buffer: &[u8],
        stop: &AtomicBool,
        deadline: Instant,
        live: &AtomicI64,
    ) -> i64 {
        let mut total: i64 = 0;
        while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
            let result = self
                .agent
                .post(url)
                .timeout(Duration::from_secs(30))
                .send_bytes(buffer);
            match result {
                Ok(resp) => {
                    let _ = resp.into_string();
                    total += buffer.len() as i64;
                    live.fetch_add(buffer.len() as i64, Ordering::Relaxed);
                }
                Err(ureq::Error::Status(_, resp)) => {
                    // Server answered (even with an error status); the bytes were sent.
                    let _ = resp.into_string();
                    total += buffer.len() as i64;
                    live.fetch_add(buffer.len() as i64, Ordering::Relaxed);
                }
                Err(_) => break,
            }
        }
        total
    }
}
