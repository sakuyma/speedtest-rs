//! Output formatting for every supported format.

use crate::config::Config;
use crate::engine::{iso8601_utc_now, to_json, ProgressSink, Result, ThroughputResult};
use crate::units::{self, Unit};

pub struct OutputOptions {
    pub unit: Unit,
    pub precision: i32,
    pub output_header: bool,
}

impl Default for OutputOptions {
    fn default() -> Self {
        OutputOptions { unit: Unit::Mbps, precision: 2, output_header: false }
    }
}

fn rj(s: &str, w: usize) -> String {
    if s.chars().count() >= w {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(w - s.chars().count()), s)
    }
}

fn lj(s: &str, w: usize) -> String {
    if s.chars().count() >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - s.chars().count()))
    }
}

/// Fixed "%.*f".
fn fixed(v: f64, precision: i32) -> String {
    let p = precision.max(0) as usize;
    format!("{v:.p$}")
}

/// Default C++/boost "%g" (6 significant digits), used by machine formats.
fn gfmt(v: f64) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if !v.is_finite() {
        if v.is_nan() {
            return "nan".to_string();
        }
        return if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
    }
    let p: i32 = 6;
    let x = v.abs().log10().floor() as i32;
    if x < -4 || x >= p {
        let prec = (p - 1) as usize;
        let s = format!("{v:.prec$e}");
        fix_scientific(&s)
    } else {
        let digits = (p - 1 - x).max(0) as usize;
        let s = format!("{v:.digits$}");
        strip_zeros(&s)
    }
}

fn strip_zeros(s: &str) -> String {
    if s.contains('.') {
        let t = s.trim_end_matches('0');
        t.trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
}

fn fix_scientific(s: &str) -> String {
    let (mant, exp) = match s.split_once('e') {
        Some((m, e)) => (m, e),
        None => return s.to_string(),
    };
    let mut mant = mant.to_string();
    if mant.contains('.') {
        while mant.ends_with('0') {
            mant.pop();
        }
        if mant.ends_with('.') {
            mant.pop();
        }
    }
    let e: i32 = exp.parse().unwrap_or(0);
    let sign = if e < 0 { '-' } else { '+' };
    format!("{}e{}{:02}", mant, sign, e.abs())
}

fn server_name(s: &crate::config::Server) -> String {
    if s.sponsor.is_empty() {
        s.name.clone()
    } else {
        s.sponsor.clone()
    }
}
fn server_location(s: &crate::config::Server) -> String {
    if s.sponsor.is_empty() {
        String::new()
    } else {
        s.name.clone()
    }
}

fn csv_field(s: &str, quote: bool) -> String {
    if !quote {
        return s.to_string();
    }
    let mut out = String::from("\"");
    for c in s.chars() {
        if c == '"' {
            out.push_str("\"\"");
        } else {
            out.push(c);
        }
    }
    out.push('"');
    out
}

/// "Closest servers:" table used by -L/--servers.
pub fn server_list_string(cfg: &Config) -> String {
    let mut o = String::new();
    o.push_str("Closest servers:\n");
    o.push('\n');
    o.push_str(&rj("ID", 6));
    o.push_str("  ");
    o.push_str(&lj("Name", 31));
    o.push_str(&lj("Location", 21));
    o.push_str("Country\n");
    o.push_str(&"=".repeat(78));
    o.push('\n');
    for s in &cfg.servers {
        o.push_str(&rj(&s.id.to_string(), 6));
        o.push_str("  ");
        o.push_str(&lj(&server_name(s), 31));
        o.push_str(&lj(&server_location(s), 21));
        o.push_str(&s.country);
        o.push('\n');
    }
    o
}

pub fn print_server_list(cfg: &Config) {
    print!("{}", server_list_string(cfg));
}

/// Human-readable result.
pub fn human_string(r: &Result, oo: &OutputOptions) -> String {
    let p = oo.precision;
    let dl = units::speed_parts(r.download.bandwidth as f64, oo.unit);
    let ul = units::speed_parts(r.upload.bandwidth as f64, oo.unit);

    let speed = |parts: &units::SpeedParts| {
        let mut s = rj(&fixed(parts.value, p), 9);
        s.push(' ');
        s.push_str(parts.unit);
        s
    };

    let mut o = String::new();
    o.push('\n');
    o.push_str("   Speedtest by Ookla\n");
    o.push('\n');
    o.push_str(&format!(
        "      Server: {} - {} (id: {})\n",
        r.server.name, r.server.location, r.server.id
    ));
    o.push_str(&format!("         ISP: {}\n", r.isp));
    o.push_str(&format!(
        "Idle Latency:{} ms   (jitter: {}ms, low: {}ms, high: {}ms)\n",
        rj(&fixed(r.ping.latency, p), 9),
        fixed(r.ping.jitter, p),
        fixed(r.ping.low, p),
        fixed(r.ping.high, p)
    ));

    // Download speed.
    o.push_str(&format!(
        "    Download:{} (data used: {})\n",
        speed(&dl),
        units::format_data(r.download.bytes)
    ));
    o.push_str(&format!(
        "{}{} ms   (jitter: {}ms, low: {}ms, high: {}ms)\n",
        " ".repeat(13),
        rj(&fixed(r.download.latency.iqm, p), 9),
        fixed(r.download.latency.jitter, p),
        fixed(r.download.latency.low, p),
        fixed(r.download.latency.high, p)
    ));

    // Upload speed.
    o.push_str(&format!(
        "      Upload:{} (data used: {})\n",
        speed(&ul),
        units::format_data(r.upload.bytes)
    ));
    o.push_str(&format!(
        "{}{} ms   (jitter: {}ms, low: {}ms, high: {}ms)\n",
        " ".repeat(13),
        rj(&fixed(r.upload.latency.iqm, p), 9),
        fixed(r.upload.latency.jitter, p),
        fixed(r.upload.latency.low, p),
        fixed(r.upload.latency.high, p)
    ));

    o.push_str(&format!(
        " Packet Loss:{}\n",
        rj(&format!("{}%", fixed(r.packet_loss, 1)), 9)
    ));
    if !r.result_url.is_empty() {
        o.push_str(&format!("  Result URL: {}\n", r.result_url));
    }
    o
}

pub fn print_human(r: &Result, oo: &OutputOptions) {
    print!("{}", human_string(r, oo));
}

/// CSV/TSV result.
pub fn delimited_string(r: &Result, oo: &OutputOptions, sep: char, csv_quoting: bool) -> String {
    const HEADER: [&str; 21] = [
        "server name",
        "server id",
        "idle latency",
        "idle jitter",
        "packet loss",
        "download",
        "upload",
        "download bytes",
        "upload bytes",
        "share url",
        "download server count",
        "download latency",
        "download latency jitter",
        "download latency low",
        "download latency high",
        "upload latency",
        "upload latency jitter",
        "upload latency low",
        "upload latency high",
        "idle latency low",
        "idle latency high",
    ];

    let emit = |cols: &[String]| {
        let mut line = String::new();
        for (i, c) in cols.iter().enumerate() {
            if i > 0 {
                line.push(sep);
            }
            line.push_str(&csv_field(c, csv_quoting));
        }
        line.push('\n');
        line
    };

    let mut o = String::new();
    if oo.output_header {
        let h: Vec<String> = HEADER.iter().map(|s| s.to_string()).collect();
        o.push_str(&emit(&h));
    }

    let name = format!("{} - {}", r.server.name, r.server.location);
    let row: Vec<String> = vec![
        name,
        r.server.id.to_string(),
        gfmt(r.ping.latency),
        gfmt(r.ping.jitter),
        gfmt(r.packet_loss),
        r.download.bandwidth.to_string(),
        r.upload.bandwidth.to_string(),
        r.download.bytes.to_string(),
        r.upload.bytes.to_string(),
        r.result_url.clone(),
        "1".to_string(),
        gfmt(r.download.latency.iqm),
        gfmt(r.download.latency.jitter),
        gfmt(r.download.latency.low),
        gfmt(r.download.latency.high),
        gfmt(r.upload.latency.iqm),
        gfmt(r.upload.latency.jitter),
        gfmt(r.upload.latency.low),
        gfmt(r.upload.latency.high),
        gfmt(r.ping.low),
        gfmt(r.ping.high),
    ];
    o.push_str(&emit(&row));
    o
}

pub fn print_delimited(r: &Result, oo: &OutputOptions, sep: char, csv_quoting: bool) {
    print!("{}", delimited_string(r, oo, sep, csv_quoting));
}

/// json / json-pretty result.
pub fn print_json(r: &Result, pretty: bool) {
    println!("{}", to_json(r).dump(pretty));
}

// ---- jsonl -----------------------------------------------------------------

pub struct JsonlSink;

impl JsonlSink {
    pub fn new() -> Self {
        JsonlSink
    }
}

impl Default for JsonlSink {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressSink for JsonlSink {
    fn on_test_start(&mut self, r: &Result) {
        let mut v = crate::json::Value::object();
        v.set("type", crate::json::Value::string("testStart"));
        v.set("timestamp", crate::json::Value::string(r.timestamp.clone()));
        v.set("isp", crate::json::Value::string(r.isp.clone()));

        let mut iface = crate::json::Value::object();
        iface.set("internalIp", crate::json::Value::string(r.interface.internal_ip.clone()));
        iface.set("name", crate::json::Value::string(r.interface.name.clone()));
        iface.set("macAddr", crate::json::Value::string(r.interface.mac_addr.clone()));
        iface.set("isVpn", crate::json::Value::bool(r.interface.is_vpn));
        iface.set("externalIp", crate::json::Value::string(r.interface.external_ip.clone()));
        v.set("interface", iface);

        let mut server = crate::json::Value::object();
        server.set("id", crate::json::Value::int(r.server.id));
        server.set("host", crate::json::Value::string(r.server.host.clone()));
        server.set("port", crate::json::Value::int(r.server.port as i64));
        server.set("name", crate::json::Value::string(r.server.name.clone()));
        server.set("location", crate::json::Value::string(r.server.location.clone()));
        server.set("country", crate::json::Value::string(r.server.country.clone()));
        server.set("ip", crate::json::Value::string(r.server.ip.clone()));
        v.set("server", server);

        println!("{}", v.dump(false));
    }

    fn on_ping(&mut self, latency_ms: f64, jitter_ms: f64, index: i32, total: i32) {
        let mut v = crate::json::Value::object();
        v.set("type", crate::json::Value::string("ping"));
        v.set("timestamp", crate::json::Value::string(iso8601_utc_now()));
        let mut ping = crate::json::Value::object();
        ping.set("jitter", crate::json::Value::double(jitter_ms));
        ping.set("latency", crate::json::Value::double(latency_ms));
        ping.set(
            "progress",
            crate::json::Value::double(if total > 0 { index as f64 / total as f64 } else { 0.0 }),
        );
        v.set("ping", ping);
        println!("{}", v.dump(false));
    }

    fn on_progress(&mut self, direction: &str, bytes: i64, seconds: f64, progress: f64) {
        let mut v = crate::json::Value::object();
        v.set("type", crate::json::Value::string(direction));
        v.set("timestamp", crate::json::Value::string(iso8601_utc_now()));
        let mut body = crate::json::Value::object();
        body.set(
            "bandwidth",
            crate::json::Value::int(if seconds > 0.0 { (bytes as f64 / seconds) as i64 } else { 0 }),
        );
        body.set("bytes", crate::json::Value::int(bytes));
        body.set("elapsed", crate::json::Value::int((seconds * 1000.0) as i64));
        body.set("progress", crate::json::Value::double(progress));
        v.set(direction, body);
        println!("{}", v.dump(false));
    }

    fn on_stage_end(&mut self, direction: &str, r: &ThroughputResult) {
        let mut v = crate::json::Value::object();
        v.set("type", crate::json::Value::string(direction));
        v.set("timestamp", crate::json::Value::string(iso8601_utc_now()));
        let mut body = crate::json::Value::object();
        body.set("bandwidth", crate::json::Value::int(r.bandwidth));
        body.set("bytes", crate::json::Value::int(r.bytes));
        body.set("elapsed", crate::json::Value::int(r.elapsed_ms));
        let mut lat = crate::json::Value::object();
        lat.set("iqm", crate::json::Value::double(r.latency.iqm));
        body.set("latency", lat);
        body.set("progress", crate::json::Value::double(1.0));
        v.set(direction, body);
        println!("{}", v.dump(false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        to_json, InterfaceInfo, LatencySet, PingResult, Result, ServerInfo, ThroughputResult,
    };

    pub fn make_sample_result() -> Result {
        Result {
            type_: "result".to_string(),
            timestamp: "2026-10-15T20:56:04Z".to_string(),
            ping: PingResult { latency: 4.404, jitter: 0.129, low: 4.300, high: 4.478 },
            download: ThroughputResult {
                bandwidth: 69692676,
                bytes: 1023140336,
                elapsed_ms: 15006,
                latency: LatencySet { iqm: 4.246, low: 3.231, high: 219.627, jitter: 10.447, samples: 10 },
            },
            upload: ThroughputResult {
                bandwidth: 65897016,
                bytes: 815966616,
                elapsed_ms: 11614,
                latency: LatencySet { iqm: 4.644, low: 3.018, high: 589.979, jitter: 23.764, samples: 10 },
            },
            packet_loss: 0.0,
            isp: "Moscow City Telephone Network".to_string(),
            interface: InterfaceInfo {
                internal_ip: "192.168.10.4".to_string(),
                name: "enp42s0".to_string(),
                mac_addr: "D8:43:AE:93:9C:11".to_string(),
                external_ip: "46.138.37.219".to_string(),
                is_vpn: false,
            },
            server: ServerInfo {
                id: 58664,
                host: "speedtest.procloud.ru".to_string(),
                port: 8080,
                name: "ProCloud".to_string(),
                location: "Moscow".to_string(),
                country: "Russia".to_string(),
                ip: "194.55.234.147".to_string(),
            },
            result_id: "3e2d8632-ea44-43f6-b2de-df27bad7759e".to_string(),
            result_url: "https://www.speedtest.net/result/c/3e2d8632-ea44-43f6-b2de-df27bad7759e".to_string(),
            persisted: true,
        }
    }

    #[test]
    fn human_output_matches() {
        let r = make_sample_result();
        let oo = OutputOptions { unit: crate::units::Unit::Mbps, precision: 2, output_header: false };
        let expected = concat!(
            "\n",
            "   Speedtest by Ookla\n",
            "\n",
            "      Server: ProCloud - Moscow (id: 58664)\n",
            "         ISP: Moscow City Telephone Network\n",
            "Idle Latency:     4.40 ms   (jitter: 0.13ms, low: 4.30ms, high: 4.48ms)\n",
            "    Download:   557.54 Mbps (data used: 1.0 GB)\n",
            "                  4.25 ms   (jitter: 10.45ms, low: 3.23ms, high: 219.63ms)\n",
            "      Upload:   527.18 Mbps (data used: 816.0 MB)\n",
            "                  4.64 ms   (jitter: 23.76ms, low: 3.02ms, high: 589.98ms)\n",
            " Packet Loss:     0.0%\n",
            "  Result URL: https://www.speedtest.net/result/c/3e2d8632-ea44-43f6-b2de-df27bad7759e\n",
        );
        assert_eq!(human_string(&r, &oo), expected, "human output mismatch");
    }

    #[test]
    fn csv_output() {
        let r = make_sample_result();
        let oo = OutputOptions::default();
        let expected = "\"ProCloud - Moscow\",\"58664\",\"4.404\",\"0.129\",\"0\",\"69692676\",\"65897016\",\
\"1023140336\",\"815966616\",\
\"https://www.speedtest.net/result/c/3e2d8632-ea44-43f6-b2de-df27bad7759e\",\"1\",\
\"4.246\",\"10.447\",\"3.231\",\"219.627\",\"4.644\",\"23.764\",\"3.018\",\"589.979\",\
\"4.3\",\"4.478\"\n";
        assert_eq!(delimited_string(&r, &oo, ',', true), expected);
    }

    #[test]
    fn result_json_matches() {
        let r = make_sample_result();
        let expected = "{\"type\":\"result\",\"timestamp\":\"2026-10-15T20:56:04Z\",\
\"ping\":{\"jitter\":0.129,\"latency\":4.404,\"low\":4.300,\"high\":4.478},\
\"download\":{\"bandwidth\":69692676,\"bytes\":1023140336,\"elapsed\":15006,\
\"latency\":{\"iqm\":4.246,\"low\":3.231,\"high\":219.627,\"jitter\":10.447}},\
\"upload\":{\"bandwidth\":65897016,\"bytes\":815966616,\"elapsed\":11614,\
\"latency\":{\"iqm\":4.644,\"low\":3.018,\"high\":589.979,\"jitter\":23.764}},\
\"packetLoss\":0,\"isp\":\"Moscow City Telephone Network\",\
\"interface\":{\"internalIp\":\"192.168.10.4\",\"name\":\"enp42s0\",\
\"macAddr\":\"D8:43:AE:93:9C:11\",\"isVpn\":false,\"externalIp\":\"46.138.37.219\"},\
\"server\":{\"id\":58664,\"host\":\"speedtest.procloud.ru\",\"port\":8080,\
\"name\":\"ProCloud\",\"location\":\"Moscow\",\"country\":\"Russia\",\"ip\":\"194.55.234.147\"},\
\"result\":{\"id\":\"3e2d8632-ea44-43f6-b2de-df27bad7759e\",\
\"url\":\"https://www.speedtest.net/result/c/3e2d8632-ea44-43f6-b2de-df27bad7759e\",\
\"persisted\":true}}";
        assert_eq!(to_json(&r).dump(false), expected);
    }

    #[test]
    fn tsv_output() {
        let r = make_sample_result();
        let oo = OutputOptions { unit: crate::units::Unit::Mbps, precision: 2, output_header: false };
        let expected = "ProCloud - Moscow\t58664\t4.404\t0.129\t0\t69692676\t65897016\t1023140336\t815966616\thttps://www.speedtest.net/result/c/3e2d8632-ea44-43f6-b2de-df27bad7759e\t1\t4.246\t10.447\t3.231\t219.627\t4.644\t23.764\t3.018\t589.979\t4.3\t4.478\n";
        assert_eq!(delimited_string(&r, &oo, '\t', false), expected);
    }

    #[test]
    fn csv_header() {
        let r = make_sample_result();
        let oo = OutputOptions { unit: crate::units::Unit::Mbps, precision: 2, output_header: true };
        let out = delimited_string(&r, &oo, ',', true);
        let first = out.lines().next().unwrap();
        assert!(first.starts_with("\"server name\",\"server id\",\"idle latency\""));
        assert!(first.ends_with("\"idle latency high\""));
    }

    #[test]
    fn json_pretty_prefix() {
        let r = make_sample_result();
        let pretty = to_json(&r).dump(true);
        assert!(pretty.starts_with("{\n    \"type\": \"result\","));
    }
}

