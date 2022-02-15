use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use clap::Parser as clap_parser;
use hostname::get as get_hostname;
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
    #[clap(short, long, default_value = "")]
    cert: String,
    #[clap(short = 'k', long, default_value = "")]
    private_key: String,
}

lazy_static! {
    /// 実行時引数の構造体
    static ref ARGS: Args = Args::parse();
    /// マークダウンコンバータ
    static ref CONVERTER: md2html::Converter = {

    let mut converter = md2html::Converter::new(|main, meta| {
        format!(
            r#"<!DOCTYPE html>
<html>
    <head>
        <meta charset="utf-8">
        <title>{wiki_name} | {entry_name}</title>
        <meta name="viewport" content="width=device-width, initial-scale=1">

        <link rel="stylesheet" href="https://unpkg.com/latex.css/style.min.css" />
        <style>
            body {{ font-family: 'Times New Roman', "游明朝体", 'Yu Mincho', 'YuMincho', 'Noto Serif JP', serif; }}
        </style>

        <link rel="stylesheet" href="https://latex.now.sh/prism/prism.css">
        <script src="https://cdn.jsdelivr.net/npm/prismjs/prism.min.js"></script>
        <script src="https://cdn.jsdelivr.net/npm/prismjs@1.26.0/components/prism-latex.min.js" crossorigin="anonymous"></script>
        <script type="text/javascript" id="MathJax-script" src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-svg.js"></script>

        <style>
            body {{
                overflow: auto scroll hidden;
                overflow-wrap: break-word;
                word-wrap: break-word;
            }}
        </style>
    </head>
    <body>

<main class="container">
{main}
</main>

    </body>
</html>"#,
            wiki_name = meta.wiki_name,
            entry_name = meta.entry_name,
        )
    });
    for reg in [
        r"\$\$(?s:.)*?\$\$",
        r"\\\((?s:.)*?\\\)",
        r"\\\[(?s:.)*?\\\]",
        r"\\begin\{\w+\*?\}(?s:.)*?\\end\{\w+\*?\}",
    ] {
        converter
            .bypass_rules
            .push((Regex::new(reg).unwrap(), |caps| {
                let mut escaped = String::new();
                pulldown_cmark::escape::escape_html(&mut escaped, &caps[0]).unwrap();
                escaped
            }));
    }
    converter
        .bypass_rules
        .push((Regex::new(r"\[\[(.*?)\]\]").unwrap(), |caps| {
            let entry = &caps[1];
            let encoded = urlencode(entry);
            format!(r#"<a href="./{encoded}">{entry}</a>"#)
        }));
    converter
    };
}

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
            .route("/", web::get().to(router))
            .route("/{entry}", web::get().to(router))
    });

    let ssl_builder = (|| {
        let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        builder.set_private_key_file(&ARGS.private_key, SslFiletype::PEM)?;
        builder.set_certificate_chain_file(&ARGS.cert)?;
        std::io::Result::Ok(builder)
    })();

    match ssl_builder {
        Ok(builder) => server.bind_openssl(address, builder)?,
        Err(_) => {
            eprintln!("Could not complete the configuration of TLS communication.");
            server.bind(address)
        }?,
    }
    .run()
    .await
}

// route wiki pages
async fn router(req: HttpRequest) -> Result<HttpResponse, Error> {
    let entry_name;
    match req.match_info().get("entry") {
        Some(entry) => {
            entry_name = sanitize_path(entry);
            if entry_name == ARGS.home {
                return Ok(HttpResponse::TemporaryRedirect()
                    .header(actix_web::http::header::LOCATION, "./")
                    .finish());
            }
        }
        None => {
            entry_name = sanitize_path(&ARGS.home);
        }
    }

    // read markdown
    let mut markdown = String::new();
    let mut file = File::open(format!("./{}.md", &entry_name))?;
    file.read_to_string(&mut markdown)?;

    let html = CONVERTER.convert(
        markdown,
        md2html::MetaData {
            entry_name,
            wiki_name: ARGS.wiki_name.clone(),
        },
    );
    // return html
    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}
