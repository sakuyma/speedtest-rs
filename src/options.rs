//! Command line option parsing plus help/version text.

use crate::units::{self, Unit};

pub const APP_VERSION: &str = "1.2.0.84";
pub const APP_BUILD: &str = "ea6b6773cf";
pub const APP_PLATFORM: &str = "Linux/x86_64-linux-musl";
pub const USER_AGENT: &str = "ookla-cli/1.2.0.84";

pub fn k_app_version() -> &'static str {
    APP_VERSION
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Human,
    Csv,
    Tsv,
    Json,
    Jsonl,
    JsonPretty,
}

#[allow(dead_code)]
pub fn format_name(f: Format) -> &'static str {
    match f {
        Format::Human => "human-readable",
        Format::Csv => "csv",
        Format::Tsv => "tsv",
        Format::Json => "json",
        Format::Jsonl => "jsonl",
        Format::JsonPretty => "json-pretty",
    }
}

// The help template with the program name inserted where given. Written as a
// raw string so leading/trailing spaces and blank lines are preserved exactly.
const HELP_TEMPLATE: &str = r#"Speedtest by Ookla is the official command line client for testing the speed and performance of your internet connection.

Version: %PROG% 1.2.0.84

Usage: %PROG% [<options>]
  -h, --help                        Print usage information
  -V, --version                     Print version number
  -L, --servers                     List nearest servers
  -s, --server-id=#                 Specify a server from the server list using its id
  -I, --interface=ARG               Attempt to bind to the specified interface when connecting to servers
  -i, --ip=ARG                      Attempt to bind to the specified IP address when connecting to servers
  -o, --host=ARG                    Specify a server, from the server list, using its host's fully qualified domain name
  -p, --progress=yes|no             Enable or disable progress bar (Note: only available for 'human-readable'
                                    or 'json' and defaults to yes when interactive)
  -P, --precision=#                 Number of decimals to use (0-8, default=2)
  -f, --format=ARG                  Output format (see below for valid formats)
      --progress-update-interval=#  Progress update interval (100-1000 milliseconds)
  -u, --unit[=ARG]                  Output unit for displaying speeds (Note: this is only applicable
                                    for ‘human-readable’ output format and the default unit is Mbps)
  -a                                Shortcut for [-u auto-decimal-bits]
  -A                                Shortcut for [-u auto-decimal-bytes]
  -b                                Shortcut for [-u auto-binary-bits]
  -B                                Shortcut for [-u auto-binary-bytes]
      --selection-details           Show server selection details
      --ca-certificate=ARG          CA Certificate bundle path
  -v                                Logging verbosity. Specify multiple times for higher verbosity
      --output-header               Show output header for CSV and TSV formats

 Valid output formats: human-readable (default), csv, tsv, json, jsonl, json-pretty

 Machine readable formats (csv, tsv, json, jsonl, json-pretty) use bytes as the unit of measure with max precision

 Valid units for [-u] flag: 
   Decimal prefix, bits per second:  bps, kbps, Mbps, Gbps
   Decimal prefix, bytes per second: B/s, kB/s, MB/s, GB/s
   Binary prefix, bits per second:   kibps, Mibps, Gibps
   Binary prefix, bytes per second:  kiB/s, MiB/s, GiB/s
   Auto-scaled prefix: auto-binary-bits, auto-binary-bytes, auto-decimal-bits, auto-decimal-bytes
"#;

pub fn help_text(prog: &str) -> String {
    format!("{}\n", HELP_TEMPLATE.replace("%PROG%", prog))
}

pub fn version_text() -> String {
    let mut release = "unknown".to_string();
    let mut machine = "x86_64".to_string();
    unsafe {
        let mut u: libc::utsname = std::mem::zeroed();
        if libc::uname(&mut u) == 0 {
            release = std::ffi::CStr::from_ptr(u.release.as_ptr()).to_string_lossy().into_owned();
            machine = std::ffi::CStr::from_ptr(u.machine.as_ptr()).to_string_lossy().into_owned();
        }
    }
    format!(
        "Speedtest by Ookla {APP_VERSION} ({APP_BUILD}) {APP_PLATFORM} {release} {machine}\n\nThe official command line client for testing the speed and performance\nof your internet connection.\n"
    )
}

#[derive(Debug, Clone)]
pub struct Options {
    pub help: bool,
    pub version: bool,
    pub servers: bool,

    pub server_id: i64,
    pub host: String,
    pub interface_name: String,
    pub bind_ip: String,
    pub ca_bundle: String,

    pub progress: i32, // -1 = unset (auto), 0 = no, 1 = yes
    pub precision: i32,
    pub format: Format,
    pub progress_update_interval_ms: i32,
    pub unit: Unit,

    pub selection_details: bool,
    pub verbosity: i32,
    pub output_header: bool,
    pub accept_license: bool,
    pub accept_gdpr: bool,
    pub config_url: String,
    pub stage_timestamps: bool,
    pub supplemental_data: bool,
    pub extra_options: Vec<String>,

    pub ok: bool,
    pub error: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            help: false,
            version: false,
            servers: false,
            server_id: 0,
            host: String::new(),
            interface_name: String::new(),
            bind_ip: String::new(),
            ca_bundle: String::new(),
            progress: -1,
            precision: 2,
            format: Format::Human,
            progress_update_interval_ms: 100,
            unit: Unit::Mbps,
            selection_details: false,
            verbosity: 0,
            output_header: false,
            accept_license: false,
            accept_gdpr: false,
            config_url: "https://cli.speedtest.net/api/cli/config".to_string(),
            stage_timestamps: false,
            supplemental_data: false,
            extra_options: Vec::new(),
            ok: true,
            error: String::new(),
        }
    }
}

/// C `atoi`/`strtol`-style integer parse: leading sign + digits, stop at the
/// first non-digit, whole-string fallback of 0.
fn c_parse_i64(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] as char).is_ascii_whitespace() {
        i += 1;
    }
    let mut sign = 1i64;
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        if bytes[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let mut val: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        val = val.wrapping_mul(10).wrapping_add((bytes[i] - b'0') as i64);
        i += 1;
    }
    sign.wrapping_mul(val)
}

fn set_format(f: &str) -> Option<Format> {
    Some(match f {
        "human-readable" | "legacy" => Format::Human,
        "csv" => Format::Csv,
        "tsv" => Format::Tsv,
        "json" => Format::Json,
        "jsonl" => Format::Jsonl,
        "json-pretty" => Format::JsonPretty,
        _ => return None,
    })
}

pub fn parse_options(args: &[String], out: &mut Options) {
    out.ok = true;
    out.error.clear();
    let prog = if !args.is_empty() { args[0].clone() } else { "speedtest".to_string() };

    let mut i = 1usize;
    while i < args.len() && out.ok {
        let a = args[i].clone();
        if a == "--" {
            break;
        }
        if a.len() > 2 && a.starts_with('-') && a.as_bytes()[1] == b'-' {
            let eq = a.find('=');
            let (name, val, has_val) = match eq {
                Some(pos) => (a[..pos].to_string(), a[pos + 1..].to_string(), true),
                None => (a.clone(), String::new(), false),
            };
            macro_rules! need_val {
                ($nm:expr) => {{
                    if has_val {
                        val.clone()
                    } else if i + 1 < args.len() {
                        i += 1;
                        args[i].clone()
                    } else {
                        if out.ok {
                            out.ok = false;
                            out.error = format!("{prog}: Missing argument for {}", $nm);
                        }
                        String::new()
                    }
                }};
            }
            match name.as_str() {
                "--help" => out.help = true,
                "--version" => out.version = true,
                "--servers" => out.servers = true,
                "--server-id" => out.server_id = c_parse_i64(&need_val!("--server-id")),
                "--interface" => out.interface_name = need_val!("--interface"),
                "--ip" => out.bind_ip = need_val!("--ip"),
                "--host" => out.host = need_val!("--host"),
                "--progress" => {
                    let v = need_val!("--progress");
                    if !set_progress(&v, out) {
                        fail(out, &prog, &format!("Invalid progress value: {v}"));
                    }
                }
                "--precision" => out.precision = c_parse_i64(&need_val!("--precision")) as i32,
                "--format" => {
                    let v = need_val!("--format");
                    match set_format(&v) {
                        Some(f) => out.format = f,
                        None => fail(out, &prog, &format!("Unknown output format: {v}")),
                    }
                }
                "--progress-update-interval" => {
                    out.progress_update_interval_ms =
                        c_parse_i64(&need_val!("--progress-update-interval")) as i32;
                }
                "--unit" => {
                    let v = need_val!("--unit");
                    match units::parse_unit(&v) {
                        Some(u) => out.unit = u,
                        None => fail(out, &prog, &format!("Invalid unit: {v}")),
                    }
                }
                "--selection-details" => out.selection_details = true,
                "--ca-certificate" => out.ca_bundle = need_val!("--ca-certificate"),
                "--output-header" => out.output_header = true,
                "--accept-license" => out.accept_license = true,
                "--accept-gdpr" => out.accept_gdpr = true,
                "--config-url" => out.config_url = need_val!("--config-url"),
                "--stage-timestamps" => out.stage_timestamps = true,
                "--supplemental-data" => out.supplemental_data = true,
                "--option" => out.extra_options.push(need_val!("--option")),
                "--verbose" => out.verbosity += 1,
                _ => fail(out, &prog, &format!("Unrecognized option: {a}")),
            }
        } else if a.len() >= 2 && a.starts_with('-') {
            let chars: Vec<char> = a.chars().collect();
            let mut j = 1usize;
            while j < chars.len() && out.ok {
                let c = chars[j];
                // Value getter: rest of this token, else the next argument.
                macro_rules! short_val {
                    ($nm:expr) => {{
                        if j + 1 < chars.len() {
                            let v: String = chars[j + 1..].iter().collect();
                            v
                        } else if i + 1 < args.len() {
                            i += 1;
                            args[i].clone()
                        } else {
                            if out.ok {
                                out.ok = false;
                                out.error = format!("{prog}: Missing argument for -{}", $nm);
                            }
                            String::new()
                        }
                    }};
                }
                match c {
                    'h' => {
                        out.help = true;
                        j += 1;
                    }
                    'V' => {
                        out.version = true;
                        j += 1;
                    }
                    'L' => {
                        out.servers = true;
                        j += 1;
                    }
                    's' => {
                        out.server_id = c_parse_i64(&short_val!('s'));
                        j = chars.len();
                    }
                    'I' => {
                        out.interface_name = short_val!('I');
                        j = chars.len();
                    }
                    'i' => {
                        out.bind_ip = short_val!('i');
                        j = chars.len();
                    }
                    'o' => {
                        out.host = short_val!('o');
                        j = chars.len();
                    }
                    'p' => {
                        let v = short_val!('p');
                        if !set_progress(&v, out) {
                            fail(out, &prog, &format!("Invalid progress value: {v}"));
                        }
                        j = chars.len();
                    }
                    'P' => {
                        out.precision = c_parse_i64(&short_val!('P')) as i32;
                        j = chars.len();
                    }
                    'f' => {
                        let v = short_val!('f');
                        match set_format(&v) {
                            Some(f) => out.format = f,
                            None => fail(out, &prog, &format!("Unknown output format: {v}")),
                        }
                        j = chars.len();
                    }
                    'u' => {
                        let v = short_val!('u');
                        match units::parse_unit(&v) {
                            Some(u) => out.unit = u,
                            None => fail(out, &prog, &format!("Invalid unit: {v}")),
                        }
                        j = chars.len();
                    }
                    'a' => {
                        out.unit = Unit::AutoDecimalBits;
                        j += 1;
                    }
                    'A' => {
                        out.unit = Unit::AutoDecimalBytes;
                        j += 1;
                    }
                    'b' => {
                        out.unit = Unit::AutoBinaryBits;
                        j += 1;
                    }
                    'B' => {
                        out.unit = Unit::AutoBinaryBytes;
                        j += 1;
                    }
                    'v' => {
                        out.verbosity += 1;
                        j += 1;
                    }
                    'O' => {
                        out.extra_options.push(short_val!('O'));
                        j = chars.len();
                    }
                    _ => {
                        fail(out, &prog, &format!("Unrecognized option: -{c}"));
                        j += 1;
                    }
                }
            }
        } else {
            fail(out, &prog, &format!("Unrecognized option: {a}"));
        }
        i += 1;
    }
}

fn set_progress(v: &str, out: &mut Options) -> bool {
    match v {
        "yes" => {
            out.progress = 1;
            true
        }
        "no" => {
            out.progress = 0;
            true
        }
        _ => false,
    }
}

fn fail(out: &mut Options, prog: &str, message: &str) {
    if out.ok {
        out.ok = false;
        out.error = format!("{prog}: {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let mut o = Options::default();
        parse_options(&owned, &mut o);
        o
    }

    #[test]
    fn basic_options() {
        let o = parse(&["speedtest", "-f", "json-pretty", "-P", "3", "--output-header"]);
        assert!(o.ok);
        assert_eq!(o.format, Format::JsonPretty);
        assert_eq!(o.precision, 3);
        assert!(o.output_header);
    }

    #[test]
    fn server_id_and_shortcut() {
        let o = parse(&["speedtest", "--server-id=58664", "-a"]);
        assert!(o.ok);
        assert_eq!(o.server_id, 58664);
        assert_eq!(o.unit, Unit::AutoDecimalBits);
    }

    #[test]
    fn reject_unknown() {
        let o = parse(&["speedtest", "--bogus"]);
        assert!(!o.ok);
        assert!(o.error.contains("Unrecognized option: --bogus"));
    }

    #[test]
    fn clustered_short_options() {
        let o = parse(&["speedtest", "-Lv"]);
        assert!(o.ok);
        assert!(o.servers);
        assert_eq!(o.verbosity, 1);
    }

    #[test]
    fn missing_argument() {
        let o = parse(&["speedtest", "-f"]);
        assert!(!o.ok);
        assert!(o.error.contains("Missing argument for -f"));
    }

    #[test]
    fn help_and_version_text() {
        let h = help_text("speedtest");
        assert!(h.contains("Usage: speedtest [<options>]"));
        assert!(h.contains("Version: speedtest 1.2.0.84"));
        assert!(h.contains(" Valid units for [-u] flag: \n"));
        let v = version_text();
        assert!(v.starts_with("Speedtest by Ookla 1.2.0.84 (ea6b6773cf) Linux/"));
    }
}
