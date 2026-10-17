// speedtest-cli reimplementation: entry point and orchestration.

mod config;
mod engine;
mod http;
mod json;
mod options;
mod output;
mod settings;
mod units;

use config::fetch_config;
use engine::{persist_result, run_test, select_server, ProgressSink, RunOptions};
use http::Http;
use options::{parse_options, Format, Options, USER_AGENT};
use output::{print_delimited, print_human, print_json, print_server_list, JsonlSink, OutputOptions};
use settings::{license_banner, license_token, load_settings, save_settings};

fn basename(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[pos + 1..].to_string(),
        None => path.to_string(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut opt = Options::default();
    parse_options(&args, &mut opt);
    let prog = if !args.is_empty() { basename(&args[0]) } else { "speedtest".to_string() };

    if !opt.ok {
        eprintln!("{}", opt.error);
        print!("{}", options::help_text(&prog));
        std::process::exit(255);
    }
    if opt.help {
        print!("{}", options::help_text(&prog));
        return;
    }
    if opt.version {
        print!("{}", options::version_text());
        return;
    }

    let http = Http::new(USER_AGENT);

    let cfg = match fetch_config(&http, &opt.config_url) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    if opt.servers {
        print_server_list(&cfg);
        return;
    }

    // License acceptance (only required to actually run a test).
    let mut settings = load_settings();
    let token = license_token(&cfg);
    let accepted = settings.license_accepted == token;
    if !accepted {
        eprint!("{}", license_banner(&cfg.license_message));
        if opt.accept_license {
            settings.license_accepted = token;
            save_settings(&settings);
            eprint!("\nLicense acceptance recorded. Continuing.\n\n");
        } else if isatty_stdin() {
            eprint!("Do you accept the license? [type YES to accept]: ");
            let _ = std::io::Write::flush(&mut std::io::stderr());
            let mut line = String::new();
            let _ = std::io::stdin().read_line(&mut line);
            let line = line.trim_end_matches(['\n', '\r']);
            if line == "YES" {
                settings.license_accepted = token;
                save_settings(&settings);
                eprint!("License acceptance recorded. Continuing.\n\n");
            } else {
                eprint!("License not accepted. Exiting.\n");
                std::process::exit(1);
            }
        } else {
            eprint!(
                "\nTo accept the message please run speedtest interactively or use the following: \n\n    {prog} --accept-license\n"
            );
            std::process::exit(1);
        }
    }

    let ro = RunOptions {
        server_id: opt.server_id,
        host: opt.host.clone(),
        interface_name: opt.interface_name.clone(),
        bind_ip: opt.bind_ip.clone(),
        ca_bundle: opt.ca_bundle.clone(),
        ..RunOptions::default()
    };

    let chosen = match select_server(&cfg, &ro) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

    let oo = OutputOptions {
        unit: opt.unit,
        precision: opt.precision,
        output_header: opt.output_header,
    };

    let mut jsonl = JsonlSink::new();
    let mut sink: Option<&mut dyn ProgressSink> =
        if opt.format == Format::Jsonl { Some(&mut jsonl) } else { None };
    let mut result = run_test(&cfg, &chosen, &ro, &http, &mut sink);
    drop(sink);

    persist_result(&cfg, &http, &mut result);

    match opt.format {
        Format::Human => print_human(&result, &oo),
        Format::Csv => print_delimited(&result, &oo, ',', true),
        Format::Tsv => print_delimited(&result, &oo, '\t', false),
        Format::Json | Format::Jsonl => print_json(&result, false),
        Format::JsonPretty => print_json(&result, true),
    }
}

fn isatty_stdin() -> bool {
    unsafe { libc::isatty(0) == 1 }
}
