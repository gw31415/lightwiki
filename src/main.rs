use actix_files::NamedFile;
use actix_web::{
    get, middleware::DefaultHeaders, App, Error, HttpRequest, HttpResponse, HttpServer,
};
use clap::Parser as clap_parser;
use hostname::get as get_hostname;
use katex;
use regex::Regex;
use sanitize_filename::sanitize as sanitize_path;
use std::fs::File;
use std::io::prelude::*;
use urlencoding::encode as urlencode;
mod md2html;
#[macro_use]
extern crate lazy_static;

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

/// A simple wiki site program.
#[derive(clap_parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Specifies the port number to listen on.
    #[clap(short, long, default_value_t = 80)]
    port: usize,
    /// Specify the host name of this computer.
    #[clap(long, default_value_t = {
        get_hostname().unwrap_or(std::ffi::OsString::from("localhost")).into_string().unwrap_or("localhost".to_string())
    })]
    hostname: String,
    /// Specifies the site name of the wiki site.
    /// This will be assigned to the title attribute of the page.
    #[clap(short, long, default_value = "Light Wiki")]
    wiki_name: String,
    /// Specify the special entry name corresponding to the home page.
    #[clap(long, default_value = "README")]
    home: String,
    /// Enable TLS communication.
    #[clap(long)]
    tls: bool,
    /// Specifies a TLS certificate.
    #[clap(short, long, default_value = "fullchain.pem")]
    certificate: String,
    /// Specifies the private key for TLS.
    #[clap(short = 'k', long, default_value = "privkey.pem")]
    priv_key: String,
}

lazy_static! {
    /// 実行時引数の構造体
    static ref ARGS: Args = Args::parse();
    /// マークダウンコンバータ
    static ref CONVERTER: md2html::Converter = {

        let mut converter = md2html::Converter::new(|main, meta| {
            format!(
                r#"<!DOCTYPE html>
<html lang="ja">
    <head>
        <meta charset="utf-8">
        <title>{wiki_name} | {entry_name}</title>
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <meta name="description" content="A wiki site developed for personal use." />

        <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.15.2/dist/katex.min.css" integrity="sha384-MlJdn/WNKDGXveldHDdyRP1R4CTHr3FeuDNfhsLPYrq2t0UBkUdK2jyTnXPEK1NQ" crossorigin="anonymous" />
        <link rel="stylesheet" href="/{THEMEFILE}" />

    </head>
    <body>

<main class="container">
{main}</main>

    </body>
</html>"#,
wiki_name = meta.wiki_name,
entry_name = meta.entry_name,
)
});
for reg in [
    r"\$\$((?s:.)*?)\$\$",
    r"\\\[((?s:.)*?)\\\]",
] {
    converter
        .bypass_rules
        .push((Regex::new(reg).unwrap(), |caps| {
            let opts = katex::Opts::builder().display_mode(true).build().unwrap();
            katex::render_with_opts(&caps[1], &opts).unwrap()
        }));
}
converter
.bypass_rules
.push((Regex::new(r"\\\(((?s:.)*?)\\\)").unwrap(), |caps| {
    let opts = katex::Opts::builder().display_mode(false).build().unwrap();
    katex::render_with_opts(&caps[1], &opts).unwrap()
}));
converter
.bypass_rules
.push((Regex::new(r"\[\[\s*(.*?)\s*\]\]").unwrap(), |caps| {
    let entry = &caps[1];
    let encoded = urlencode(entry);
    format!(r#"<a href="./{encoded}" class="wiki-link">{entry}</a>"#)
}));
converter
};
}
const THEMEFILE: &str = "theme.css";

// entry point
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let address = format!(
        "{hostname}:{port}",
        hostname = ARGS.hostname,
        port = ARGS.port
    );
    let server = HttpServer::new(|| {
        App::new()
            .wrap(DefaultHeaders::new().header("Content-Security-Policy", "style-src * 'unsafe-inline'; script-src 'none'; img-src https://*; default-src 'self'; font-src *"))
            .service(static_files)
            .service(top_page)
            .service(entry_bind)
    });

    if ARGS.tls {
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        builder.set_private_key_file(&ARGS.priv_key, SslFiletype::PEM)?;
        builder.set_certificate_chain_file(&ARGS.certificate)?;
        server.bind_openssl(address, builder)?
    } else {
        server.bind(address)?
    }
    .run()
    .await
}

#[get("/{filename:[^\\.]+\\.[^\\.]+}")]
async fn static_files(req: HttpRequest) -> Result<HttpResponse, Error> {
    use std::path::PathBuf;
    let filename = req.match_info().query("filename");
    if filename == THEMEFILE {
        return Ok(HttpResponse::Ok()
            .content_type("text/css")
            .body(std::include_str!("./default.css")));
    }
    let path: PathBuf = req.match_info().query("filename").parse().unwrap();
    NamedFile::open(path)?.into_response(&req)
}

#[get("/{entry:.+}")]
async fn entry_bind(req: HttpRequest) -> Result<HttpResponse, Error> {
    if req.match_info().query("entry") == &ARGS.home {
        return Ok(HttpResponse::TemporaryRedirect()
            .header(actix_web::http::header::LOCATION, "/")
            .finish());
    }
    entry_to_response(req.match_info().query("entry"))
}

#[get("/")]
async fn top_page() -> Result<HttpResponse, Error> {
    entry_to_response(&ARGS.home)
}

fn entry_to_response(entry_name: &str) -> Result<HttpResponse, Error> {
    if entry_name != &sanitize_path(entry_name) {
        return Ok(HttpResponse::NotAcceptable().finish());
    }
    let mut markdown = String::new();
    let mut file = File::open(format!("./{}.md", entry_name))?;
    file.read_to_string(&mut markdown)?;

    let html = CONVERTER.convert(
        markdown,
        md2html::MetaData {
            entry_name: entry_name.to_string(),
            wiki_name: ARGS.wiki_name.clone(),
        },
    );
    // return html
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}
