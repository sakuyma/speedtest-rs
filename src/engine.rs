//! Speedtest measurement engine: server selection, latency, download, upload.

use crate::config::{Config, Server};
use crate::http::Http;
use crate::json::Value;
use crate::options::k_app_version;

use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct LatencySet {
    pub iqm: f64,
    pub low: f64,
    pub high: f64,
    pub jitter: f64,
    pub samples: i32,
}

#[derive(Debug, Clone, Default)]
pub struct PingResult {
    pub latency: f64,
    pub jitter: f64,
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ThroughputResult {
    pub bandwidth: i64, // bytes per second
    pub bytes: i64,
    pub elapsed_ms: i64,
    pub latency: LatencySet,
}

#[derive(Debug, Clone, Default)]
pub struct InterfaceInfo {
    pub internal_ip: String,
    pub name: String,
    pub mac_addr: String,
    pub external_ip: String,
    pub is_vpn: bool,
}

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub id: i64,
    pub host: String,
    pub port: u16,
    pub name: String,
    pub location: String,
    pub country: String,
    pub ip: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        ServerInfo {
            id: 0,
            host: String::new(),
            port: 8080,
            name: String::new(),
            location: String::new(),
            country: String::new(),
            ip: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Result {
    pub type_: String,
    pub timestamp: String,
    pub ping: PingResult,
    pub download: ThroughputResult,
    pub upload: ThroughputResult,
    pub packet_loss: f64,
    pub isp: String,
    pub interface: InterfaceInfo,
    pub server: ServerInfo,
    pub result_id: String,
    pub result_url: String,
    pub persisted: bool,
}

impl Default for Result {
    fn default() -> Self {
        Result {
            type_: "result".to_string(),
            timestamp: String::new(),
            ping: PingResult::default(),
            download: ThroughputResult::default(),
            upload: ThroughputResult::default(),
            packet_loss: 0.0,
            isp: String::new(),
            interface: InterfaceInfo::default(),
            server: ServerInfo::default(),
            result_id: String::new(),
            result_url: String::new(),
            persisted: false,
        }
    }
}

/// Optional progress sink for streaming output formats (jsonl, human progress).
pub trait ProgressSink {
    fn on_test_start(&mut self, _r: &Result) {}
    fn on_ping(&mut self, _latency_ms: f64, _jitter_ms: f64, _index: i32, _total: i32) {}
    fn on_stage_start(&mut self, _direction: &str) {}
    fn on_progress(&mut self, _direction: &str, _bytes: i64, _seconds: f64, _progress: f64) {}
    fn on_stage_end(&mut self, _direction: &str, _r: &ThroughputResult) {}
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RunOptions {
    pub server_id: i64,
    pub host: String,
    pub interface_name: String,
    pub bind_ip: String,
    pub ca_bundle: String,
    pub test_duration_seconds: i32,
    pub connections: i32,
    pub ping_count: i32,
    pub external_ip_url: String,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            server_id: 0,
            host: String::new(),
            interface_name: String::new(),
            bind_ip: String::new(),
            ca_bundle: String::new(),
            test_duration_seconds: 0,
            connections: 0,
            ping_count: 0,
            external_ip_url: "https://api.ipify.org".to_string(),
        }
    }
}

// ---- time / ids ------------------------------------------------------------

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

pub fn iso8601_utc_now() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

pub fn uuid4() -> String {
    let mut b = [0u8; 16];
    let mut ok = false;
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(&mut b).is_ok() {
            ok = true;
        }
    }
    if !ok {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = ((nanos >> ((i % 16) * 8)) & 0xff) as u8;
        }
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = |s: usize| format!("{:02x}", b[s]);
    format!(
        "{}{}{}{}-{}{}-{}{}-{}{}-{}{}{}{}{}{}",
        h(0), h(1), h(2), h(3), h(4), h(5), h(6), h(7), h(8), h(9), h(10), h(11), h(12), h(13),
        h(14), h(15)
    )
}

// ---- network helpers -------------------------------------------------------

/// TCP connect time in milliseconds (SYN -> SYN/ACK), a stable RTT proxy used
/// for idle and loaded latency. Returns -1 on failure.
pub fn tcp_connect_ms(host: &str, port: u16) -> f64 {
    let addrs = match (host, port).to_socket_addrs() {
        Ok(it) => it.collect::<Vec<_>>(),
        Err(_) => return -1.0,
    };
    let mut best = -1.0f64;
    for addr in addrs {
        let t0 = Instant::now();
        if TcpStream::connect_timeout(&addr, Duration::from_millis(2000)).is_ok() {
            let dt = t0.elapsed().as_secs_f64() * 1000.0;
            if best < 0.0 || dt < best {
                best = dt;
            }
        }
    }
    best
}

/// Resolve a host to its first IPv4 dotted-quad, empty on failure.
fn resolve_ipv4(host: &str) -> String {
    if let Ok(addrs) = (host, 0u16).to_socket_addrs() {
        for a in addrs {
            if let IpAddr::V4(v4) = a.ip() {
                return v4.to_string();
            }
        }
    }
    String::new()
}

fn trim(s: &str) -> String {
    s.trim_matches(|c| c == ' ' || c == '\t' || c == '\r' || c == '\n').to_string()
}

fn make_upload_buffer(bytes: usize) -> Vec<u8> {
    const PATTERN: &[u8] = b"ABCDEFGHIJ";
    // Fill a single allocation by repeatedly doubling the seeded prefix, so the
    // 16 MiB payload costs O(log n) memcpys instead of one extend per few bytes.
    let mut buf = vec![0u8; bytes];
    let seed = PATTERN.len().min(bytes);
    buf[..seed].copy_from_slice(&PATTERN[..seed]);
    let mut filled = seed;
    while filled < bytes {
        let n = filled.min(bytes - filled);
        let (prefix, rest) = buf.split_at_mut(filled);
        rest[..n].copy_from_slice(&prefix[..n]);
        filled += n;
    }
    buf
}

fn download_url(s: &Server, size: i64) -> String {
    format!("http://{}/download?size={}", s.host, size)
}
fn upload_url(s: &Server) -> String {
    format!("http://{}/upload", s.host)
}

// ---- latency statistics ----------------------------------------------------

pub fn compute_latency_set(mut samples: Vec<f64>) -> LatencySet {
    let mut r = LatencySet::default();
    if samples.is_empty() {
        return r;
    }
    r.samples = samples.len() as i32;
    r.low = samples.iter().cloned().fold(f64::INFINITY, f64::min);
    r.high = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut jsum = 0.0;
    for i in 1..samples.len() {
        jsum += (samples[i] - samples[i - 1]).abs();
    }
    r.jitter = if samples.len() > 1 {
        jsum / (samples.len() as f64 - 1.0)
    } else {
        0.0
    };

    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = samples.len() as i32;
    let mut lo = (n as f64 * 0.25).floor() as i32;
    let mut hi = (n as f64 * 0.75).ceil() as i32 - 1;
    if hi < lo {
        lo = 0;
        hi = n - 1;
    }
    let mut sum = 0.0;
    let mut count = 0;
    let mut i = lo;
    while i <= hi && i < n {
        sum += samples[i as usize];
        count += 1;
        i += 1;
    }
    r.iqm = if count > 0 { sum / count as f64 } else { samples[(n / 2) as usize] };
    r
}

pub fn measure_latency(s: &Server, probes: i32) -> (f64, bool) {
    let mut best = 1e18f64;
    let mut any = false;
    for _ in 0..probes {
        let ms = tcp_connect_ms(&s.host_only(), s.port());
        if ms >= 0.0 {
            any = true;
            if ms < best {
                best = ms;
            }
        }
    }
    (if any { best } else { 0.0 }, any)
}

pub fn select_server(cfg: &Config, ro: &RunOptions) -> std::result::Result<Server, String> {
    if cfg.servers.is_empty() {
        return Err("server list is empty".to_string());
    }
    if ro.server_id != 0 {
        for s in &cfg.servers {
            if s.id == ro.server_id {
                return Ok(s.clone());
            }
        }
        return Err(format!("server with id {} not found", ro.server_id));
    }
    if !ro.host.is_empty() {
        for s in &cfg.servers {
            if s.host_only() == ro.host || s.host == ro.host {
                return Ok(s.clone());
            }
        }
        return Err(format!("server with host '{}' not found", ro.host));
    }

    // Probe the closest candidates concurrently and keep the lowest latency.
    let candidate_count = cfg.servers.len().min(10);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let hw = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let workers = candidate_count.min(hw.max(1));
    let candidates = &cfg.servers[..candidate_count];
    let (tx, rx) = std::sync::mpsc::channel::<(usize, bool, f64)>();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let next = &next;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= candidate_count {
                    break;
                }
                let (ms, ok) = measure_latency(&candidates[i], 2);
                let _ = tx.send((i, ok, ms));
            });
        }
    });
    drop(tx);

    let mut best = vec![1e18f64; candidate_count];
    let mut ok_flag = vec![false; candidate_count];
    for (i, ok, ms) in rx.iter() {
        ok_flag[i] = ok;
        if ok {
            best[i] = ms;
        }
    }

    let mut best_idx = 0usize;
    let mut found = false;
    for i in 0..candidate_count {
        if ok_flag[i] && (!found || best[i] < best[best_idx]) {
            best_idx = i;
            found = true;
        }
    }
    if !found {
        // Nothing responded; fall back to the first advertised server.
        return Ok(cfg.servers[0].clone());
    }
    Ok(cfg.servers[best_idx].clone())
}

pub fn gather_interface_info(server: &Server, ro: &RunOptions, http: &Http) -> InterfaceInfo {
    let mut info = InterfaceInfo::default();
    let host = server.host_only();

    // Determine the local address used to reach the server.
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect((host.as_str(), server.port())).is_ok() {
            if let Ok(local) = sock.local_addr() {
                info.internal_ip = local.ip().to_string();
            }
        }
    }

    // Map local IP -> interface name, then read its MAC address.
    if !info.internal_ip.is_empty() {
        info.name = iface_for_ip(&info.internal_ip);
    }
    if !info.name.is_empty() {
        if let Ok(contents) = std::fs::read_to_string(format!("/sys/class/net/{}/address", info.name))
        {
            info.mac_addr = trim(&contents).to_uppercase();
        }
    }

    // External IP (best effort).
    if !ro.external_ip_url.is_empty() {
        if let Ok((code, body)) = http.get_string(&ro.external_ip_url, Duration::from_secs(5)) {
            if code == 200 {
                info.external_ip = trim(&body);
            }
        }
    }
    info
}

/// Find the interface name owning `ip` using getifaddrs(3).
fn iface_for_ip(ip: &str) -> String {
    unsafe {
        let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
        if libc::getifaddrs(&mut ifap) != 0 {
            return String::new();
        }
        let mut result = String::new();
        let mut cur = ifap;
        while !cur.is_null() {
            let ifa = &*cur;
            if !ifa.ifa_addr.is_null() {
                let fam = (*ifa.ifa_addr).sa_family as i32;
                if fam == libc::AF_INET {
                    let sin = ifa.ifa_addr as *const libc::sockaddr_in;
                    let addr =
                        Ipv4Addr::from(u32::from_be((*sin).sin_addr.s_addr));
                    if addr.to_string() == ip {
                        result = std::ffi::CStr::from_ptr(ifa.ifa_name)
                            .to_string_lossy()
                            .into_owned();
                        break;
                    }
                }
            }
            cur = ifa.ifa_next;
        }
        libc::freeifaddrs(ifap);
        result
    }
}

// ---- result JSON -----------------------------------------------------------

fn throughput_json(t: &ThroughputResult) -> Value {
    let mut v = Value::object();
    v.set("bandwidth", Value::int(t.bandwidth));
    v.set("bytes", Value::int(t.bytes));
    v.set("elapsed", Value::int(t.elapsed_ms));
    let mut lat = Value::object();
    lat.set("iqm", Value::double(t.latency.iqm));
    lat.set("low", Value::double(t.latency.low));
    lat.set("high", Value::double(t.latency.high));
    lat.set("jitter", Value::double(t.latency.jitter));
    v.set("latency", lat);
    v
}

pub fn to_json(r: &Result) -> Value {
    let mut root = Value::object();
    root.set("type", Value::string(r.type_.clone()));
    root.set("timestamp", Value::string(r.timestamp.clone()));

    let mut ping = Value::object();
    ping.set("jitter", Value::double(r.ping.jitter));
    ping.set("latency", Value::double(r.ping.latency));
    ping.set("low", Value::double(r.ping.low));
    ping.set("high", Value::double(r.ping.high));
    root.set("ping", ping);

    root.set("download", throughput_json(&r.download));
    root.set("upload", throughput_json(&r.upload));

    // packetLoss is emitted as an integer when whole.
    if r.packet_loss == r.packet_loss.floor() {
        root.set("packetLoss", Value::int(r.packet_loss as i64));
    } else {
        root.set("packetLoss", Value::double(r.packet_loss));
    }

    root.set("isp", Value::string(r.isp.clone()));

    let mut iface = Value::object();
    iface.set("internalIp", Value::string(r.interface.internal_ip.clone()));
    iface.set("name", Value::string(r.interface.name.clone()));
    iface.set("macAddr", Value::string(r.interface.mac_addr.clone()));
    iface.set("isVpn", Value::bool(r.interface.is_vpn));
    iface.set("externalIp", Value::string(r.interface.external_ip.clone()));
    root.set("interface", iface);

    let mut server = Value::object();
    server.set("id", Value::int(r.server.id));
    server.set("host", Value::string(r.server.host.clone()));
    server.set("port", Value::int(r.server.port as i64));
    server.set("name", Value::string(r.server.name.clone()));
    server.set("location", Value::string(r.server.location.clone()));
    server.set("country", Value::string(r.server.country.clone()));
    server.set("ip", Value::string(r.server.ip.clone()));
    root.set("server", server);

    let mut result = Value::object();
    if !r.result_id.is_empty() {
        result.set("id", Value::string(r.result_id.clone()));
    }
    if !r.result_url.is_empty() {
        result.set("url", Value::string(r.result_url.clone()));
    }
    result.set("persisted", Value::bool(r.persisted));
    root.set("result", result);
    root
}

// ---- test stages -----------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn run_stage(
    is_upload: bool,
    server: &Server,
    http: &Http,
    threads: i32,
    duration_sec: i32,
    sink: &mut Option<&mut dyn ProgressSink>,
    direction: &str,
    loaded_samples: &mut Vec<f64>,
) -> ThroughputResult {
    let mut out = ThroughputResult::default();
    let threads = threads.max(1);
    let duration_sec = duration_sec.max(1);

    let stop = AtomicBool::new(false);
    let live = AtomicI64::new(0);
    let mut partial = vec![0i64; threads as usize];

    let chunk: i64 = 25 * 1000 * 1000; // 25 MB per ranged download request
    let url = if is_upload { upload_url(server) } else { download_url(server, chunk) };
    let buffer = if is_upload { make_upload_buffer(16 * 1024 * 1024) } else { Vec::new() };

    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs((duration_sec + 2) as u64);

    std::thread::scope(|scope| {
        for slot in partial.iter_mut() {
            let stop = &stop;
            let live = &live;
            let url = &url;
            let buffer = &buffer;
            scope.spawn(move || {
                *slot = if is_upload {
                    http.upload_for(url, buffer, stop, deadline, live)
                } else {
                    http.download_for(url, stop, deadline, live)
                };
            });
        }

        if let Some(s) = sink.as_deref_mut() {
            s.on_stage_start(direction);
        }

        let mut last_probe_s = 0.0f64;
        let mut last_report_s = 0.0f64;
        while t0.elapsed().as_secs_f64() < duration_sec as f64 {
            std::thread::sleep(Duration::from_millis(50));
            let now = t0.elapsed().as_secs_f64();
            if now - last_report_s >= 0.1 {
                last_report_s = now;
                let mut prog = now / duration_sec as f64;
                if prog > 1.0 {
                    prog = 1.0;
                }
                if let Some(s) = sink.as_deref_mut() {
                    s.on_progress(direction, live.load(Ordering::Relaxed), now, prog);
                }
            }
            if now - last_probe_s >= 0.35 {
                last_probe_s = now;
                let ms = tcp_connect_ms(&server.host_only(), server.port());
                if ms >= 0.0 {
                    loaded_samples.push(ms);
                }
            }
        }
        stop.store(true, Ordering::Relaxed);
    });

    let elapsed = t0.elapsed().as_secs_f64();
    out.bytes = partial.iter().sum();
    out.elapsed_ms = (elapsed * 1000.0) as i64;
    out.bandwidth = if elapsed > 0.0 { (out.bytes as f64 / elapsed) as i64 } else { 0 };
    out.latency = compute_latency_set(std::mem::take(loaded_samples));
    if let Some(s) = sink.as_deref_mut() {
        s.on_stage_end(direction, &out);
    }
    out
}

pub fn run_test(
    cfg: &Config,
    server: &Server,
    ro: &RunOptions,
    http: &Http,
    sink: &mut Option<&mut dyn ProgressSink>,
) -> Result {
    let mut r = Result::default();
    r.timestamp = iso8601_utc_now();
    r.isp = cfg.isp_name.clone();
    r.server.id = server.id;
    r.server.host = server.host_only();
    r.server.port = server.port();
    r.server.name = if server.sponsor.is_empty() { server.name.clone() } else { server.sponsor.clone() };
    r.server.location = server.name.clone();
    r.server.country = server.country.clone();
    r.server.ip = resolve_ipv4(&server.host_only());

    r.interface = gather_interface_info(server, ro, http);
    if let Some(s) = sink.as_deref_mut() {
        s.on_test_start(&r);
    }

    // Idle latency.
    let mut pings = if ro.ping_count > 0 { ro.ping_count } else { cfg.latency_ping_count };
    pings = pings.max(1);
    let mut samples: Vec<f64> = Vec::with_capacity(pings as usize);
    for i in 0..pings {
        let ms = tcp_connect_ms(&server.host_only(), server.port());
        if ms >= 0.0 {
            let mut jitter_so_far = 0.0;
            if !samples.is_empty() {
                let mut jsum = 0.0;
                for k in 1..samples.len() {
                    jsum += (samples[k] - samples[k - 1]).abs();
                }
                jsum += (ms - samples[samples.len() - 1]).abs();
                jitter_so_far = jsum / samples.len() as f64;
            }
            samples.push(ms);
            if let Some(s) = sink.as_deref_mut() {
                s.on_ping(ms, jitter_so_far, i + 1, pings);
            }
        }
    }
    if !samples.is_empty() {
        r.ping.latency = samples.iter().sum::<f64>() / samples.len() as f64;
        r.ping.low = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        r.ping.high = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mut jsum = 0.0;
        for i in 1..samples.len() {
            jsum += (samples[i] - samples[i - 1]).abs();
        }
        r.ping.jitter = if samples.len() > 1 {
            jsum / (samples.len() as f64 - 1.0)
        } else {
            0.0
        };
    }

    let duration = if ro.test_duration_seconds > 0 {
        ro.test_duration_seconds
    } else {
        cfg.engine.test_duration_seconds
    };
    let down_conns =
        if ro.connections > 0 { ro.connections } else { cfg.dynamic.max_connections.min(32) };
    let up_conns =
        if ro.connections > 0 { ro.connections } else { cfg.dynamic.max_connections.min(16) };

    let mut down_loaded = Vec::new();
    let mut up_loaded = Vec::new();
    r.download = run_stage(false, server, http, down_conns, duration, sink, "download", &mut down_loaded);
    r.upload = run_stage(true, server, http, up_conns, duration, sink, "upload", &mut up_loaded);

    r.packet_loss = 0.0;
    // The result timestamp reflects when the test completed.
    r.timestamp = iso8601_utc_now();
    r
}

pub fn persist_result(cfg: &Config, http: &Http, result: &mut Result) -> bool {
    if cfg.save_test_result_url.is_empty() {
        return false;
    }

    let guid = uuid4();
    let root = to_json(result);
    let mut app = Value::object();
    app.set("identifier", Value::string("cli"));
    app.set("version", Value::string(k_app_version()));
    let mut device = Value::object();
    device.set("identifier", Value::string("cli"));
    // guid/app/device are required by the reporting API; keep them first.
    let mut payload = Value::object();
    payload.set("guid", Value::string(guid.clone()));
    payload.set("app", app);
    payload.set("device", device);
    for (k, v) in root.as_object() {
        payload.set(k.clone(), v.clone());
    }

    let (code, body) = match http.post_json(&cfg.save_test_result_url, &payload.dump(false), Duration::from_secs(20)) {
        Ok(v) => v,
        Err(_) => return false,
    };
    if !(200..300).contains(&code) {
        return false;
    }
    let resp = match crate::json::parse(&body) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut id = result.result_id.clone();
    if let Some(v) = resp.find("guid") {
        if v.is_string() {
            id = v.as_string();
        }
    }
    if id.is_empty() {
        id = guid;
    }
    let mut url = String::new();
    if let Some(save) = resp.find("save") {
        url = save.get_string("shareUrl", "");
    }
    if url.is_empty() {
        url = format!("https://www.speedtest.net/result/c/{id}");
    }
    result.result_id = id;
    result.result_url = url;
    result.persisted = true;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nearly(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-6
    }

    #[test]
    fn latency_set_stats() {
        let s = compute_latency_set(vec![4.612, 4.606, 4.579, 4.882, 4.579]);
        assert_eq!(s.samples, 5);
        assert!(nearly(s.low, 4.579));
        assert!(nearly(s.high, 4.882));
        assert!(nearly(s.iqm, (4.579 + 4.606 + 4.612) / 3.0));
        assert!(nearly(s.jitter, 0.15975));
    }

    #[test]
    fn uuid_shape() {
        let u = uuid4();
        assert_eq!(u.len(), 36);
        assert_eq!(u.as_bytes()[8], b'-');
        assert_eq!(u.as_bytes()[13], b'-');
        assert_eq!(u.as_bytes()[14], b'4'); // version 4
        let variant = u.as_bytes()[19];
        assert!(variant == b'8' || variant == b'9' || variant == b'a' || variant == b'b');
    }

    #[test]
    fn iso8601_shape() {
        let t = iso8601_utc_now();
        assert_eq!(t.len(), 20);
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
        assert!(t.ends_with('Z'));
    }
}
