//! Speedtest configuration: engine parameters, app settings and server list.

use crate::http::Http;
use crate::json::{self, Value};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Server {
    pub id: i64,
    pub host: String, // host:port
    pub name: String,
    pub country: String,
    pub sponsor: String,
    pub sponsored: bool,
    pub host_functional: i64,
}

impl Server {
    pub fn host_only(&self) -> String {
        match self.host.find(':') {
            Some(pos) => self.host[..pos].to_string(),
            None => self.host.clone(),
        }
    }
    pub fn port(&self) -> u16 {
        match self.host.find(':') {
            Some(pos) => self.host[pos + 1..].parse().unwrap_or(0),
            None => 8080,
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Server {
            id: 0,
            host: String::new(),
            name: String::new(),
            country: String::new(),
            sponsor: String::new(),
            sponsored: false,
            host_functional: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Engine {
    pub is_upload_first: bool,
    pub test_duration_seconds: i32,
    pub thread_count: i32,
    pub packet_size_bytes: i64,
}

impl Default for Engine {
    fn default() -> Self {
        Engine {
            is_upload_first: false,
            test_duration_seconds: 15,
            thread_count: 4,
            packet_size_bytes: 32_000_000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dynamic {
    pub stable_stop_enabled: bool,
    pub is_scaling_enabled: bool,
    pub max_connections: i32,
}

impl Default for Dynamic {
    fn default() -> Self {
        Dynamic {
            stable_stop_enabled: true,
            is_scaling_enabled: true,
            max_connections: 32,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedLatency {
    pub enable_udp: bool,
    pub enable_tcp: bool,
}

impl Default for LoadedLatency {
    fn default() -> Self {
        LoadedLatency { enable_udp: true, enable_tcp: true }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub engine: Engine,
    pub dynamic: Dynamic,
    pub loaded_latency: LoadedLatency,
    pub latency_ping_count: i32,
    pub is_server_upload_enabled: bool,
    pub is_client_primary_measure_method: bool,
    pub isp_name: String,
    pub trace_level: i32,
    pub license_key: String,
    pub license_message: String,
    pub license_version: String,
    pub save_test_result_url: String,
    pub result_format: String,
    pub servers: Vec<Server>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            engine: Engine::default(),
            dynamic: Dynamic::default(),
            loaded_latency: LoadedLatency::default(),
            latency_ping_count: 5,
            is_server_upload_enabled: true,
            is_client_primary_measure_method: false,
            isp_name: String::new(),
            trace_level: 0,
            license_key: String::new(),
            license_message: String::new(),
            license_version: String::new(),
            save_test_result_url: String::new(),
            result_format: "json".to_string(),
            servers: Vec::new(),
        }
    }
}

pub fn parse_config(root: &Value) -> Config {
    let mut c = Config::default();
    if let Some(suite) = root.find("suite") {
        if let Some(g) = suite.find("global") {
            if let Some(eng) = g.find("engine") {
                c.engine.is_upload_first =
                    eng.find("isUploadFirst").map(|v| v.as_bool()).unwrap_or(c.engine.is_upload_first);
                c.engine.test_duration_seconds =
                    eng.get_i64("testDurationSeconds", c.engine.test_duration_seconds as i64) as i32;
                c.engine.thread_count =
                    eng.get_i64("threadCount", c.engine.thread_count as i64) as i32;
                c.engine.packet_size_bytes =
                    eng.get_i64("packetSizeBytes", c.engine.packet_size_bytes);
            }
            if let Some(dynv) = g.find("dynamic") {
                if let Some(ss) = dynv.find("stableStop") {
                    c.dynamic.stable_stop_enabled =
                        ss.find("isEnabled").map(|v| v.as_bool()).unwrap_or(c.dynamic.stable_stop_enabled);
                }
                if let Some(dl) = dynv.find("download") {
                    c.dynamic.is_scaling_enabled = dl
                        .find("isScalingEnabled")
                        .map(|v| v.as_bool())
                        .unwrap_or(c.dynamic.is_scaling_enabled);
                    c.dynamic.max_connections =
                        dl.get_i64("maxConnections", c.dynamic.max_connections as i64) as i32;
                }
            }
            if let Some(ll) = g.find("loadedLatency") {
                c.loaded_latency.enable_udp =
                    ll.find("enableUdp").map(|v| v.as_bool()).unwrap_or(c.loaded_latency.enable_udp);
                c.loaded_latency.enable_tcp =
                    ll.find("enableTcp").map(|v| v.as_bool()).unwrap_or(c.loaded_latency.enable_tcp);
            }
        }
        if let Some(ts) = suite.find("testStage") {
            if let Some(lat) = ts.find("latency") {
                c.latency_ping_count = lat.get_i64("pingCount", c.latency_ping_count as i64) as i32;
            }
            if let Some(up) = ts.find("upload") {
                c.is_server_upload_enabled = up
                    .find("isServerUploadEnabled")
                    .map(|v| v.as_bool())
                    .unwrap_or(c.is_server_upload_enabled);
                c.is_client_primary_measure_method = up
                    .find("isClientPrimaryMeasureMethod")
                    .map(|v| v.as_bool())
                    .unwrap_or(c.is_client_primary_measure_method);
            }
        }
    }
    if let Some(app) = root.find("app") {
        c.isp_name = app.get_string("ispName", &c.isp_name);
        c.trace_level = app.get_i64("traceLevel", c.trace_level as i64) as i32;
        c.license_key = app.get_string("licenseKey", &c.license_key);
        if let Some(lic) = app.find("license") {
            c.license_message = lic.get_string("message", &c.license_message);
            c.license_version = lic.get_string("version", &c.license_version);
        }
        c.save_test_result_url = app.get_string("saveTestResultUrl", &c.save_test_result_url);
        c.result_format = app.get_string("resultFormat", &c.result_format);
    }
    if let Some(servers) = root.find("servers") {
        if servers.is_array() {
            for s in servers.as_array() {
                let mut srv = Server::default();
                srv.id = s.get_i64("id", 0);
                srv.host = s.get_string("host", "");
                srv.name = s.get_string("name", "");
                srv.country = s.get_string("country", "");
                srv.sponsor = s.get_string("sponsor", "");
                srv.sponsored = s.find("sponsored").map(|v| v.as_bool()).unwrap_or(false);
                srv.host_functional = s.get_i64("host_functional", 1);
                if !srv.host.is_empty() {
                    c.servers.push(srv);
                }
            }
        }
    }
    c
}

pub fn fetch_config(http: &Http, url: &str) -> Result<Config, String> {
    let (code, body) = http
        .get_string(url, Duration::from_secs(20))
        .map_err(|e| format!("could not retrieve or read configuration: {e}"))?;
    if !(200..300).contains(&code) {
        return Err(format!("configuration request returned HTTP {code}"));
    }
    let root = json::parse(&body).map_err(|e| format!("engine config json parsing failed: {e}"))?;
    Ok(parse_config(&root))
}
