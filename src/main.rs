use std::{
    env, fs,
    io::{self, ErrorKind},
    ops::Index,
    path::PathBuf,
};

use axum::{
    Router,
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use ini::Ini;
use rand::distr::{Alphanumeric, SampleString};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use url::Url;

const MAX_SIZE: u64 = 1024 * 1024 * 64;

async fn create_paste(
    socket: &mut TcpStream,
    dir: PathBuf,
    external_url: &Url,
) -> Result<String, anyhow::Error> {
    let mut data = Vec::new();
    let mut tooken = socket.take(MAX_SIZE);

    loop {
        let amount = tooken.read_buf(&mut data).await?;
        if amount == 0 {
            break;
        }

        let last = data.last().unwrap();
        if *last == 10u8 {
            // Apparently needed to use netcat
            break;
        }
    }

    let id = Alphanumeric.sample_string(&mut rand::rng(), 16);
    let endpoint = external_url.join(&id)?;
    let file_out = dir.join(id);
    tokio::fs::write(file_out, data).await?;

    Ok(endpoint.as_str().to_string())
}

async fn get_paste(Path(hash): Path<String>, path: PathBuf) -> impl IntoResponse {
    let file = path.join(hash.trim().replace("/", ""));
    let result = fs::read(file);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", "text/plain".parse().unwrap());

    if let Err(err) = result {
        if err.kind() == ErrorKind::NotFound {
            return (
                StatusCode::BAD_REQUEST,
                headers,
                "File does not exist.".as_bytes().to_vec(),
            );
        }

        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            headers,
            err.to_string().as_bytes().to_vec(),
        );
    }
    return (StatusCode::OK, headers, result.unwrap());
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    let config_name = args.index(1);
    let config_file = fs::read_to_string(config_name).unwrap_or("".to_string());

    let conf = Ini::load_from_str(&config_file).unwrap();
    let dir = std::path::Path::new(conf.general_section().get("dir").unwrap_or("./files"))
        .to_path_buf()
        .canonicalize()
        .unwrap();
    if !dir.exists() || !dir.is_dir() {
        panic!("dir {:?} does not exist", dir);
    }

    let upload_port = conf
        .general_section()
        .get("port")
        .unwrap_or("9999")
        .parse::<usize>()
        .expect("invalid port");

    let http_port = conf
        .general_section()
        .get("http_port")
        .unwrap_or("3000")
        .parse::<usize>()
        .expect("invalid http port");

    let external_url = Url::parse(
        conf.general_section()
            .get("url")
            .unwrap_or(&format!("http://localhost:{}", http_port)),
    )
    .expect("invalid url");

    let upload_server = TcpListener::bind(format!("0.0.0.0:{}", upload_port))
        .await
        .expect("failed to create upload server");
    let http_server = TcpListener::bind(format!("0.0.0.0:{}", http_port))
        .await
        .expect("failed to create http server");
    let server_buf = dir.clone();
    let server = Router::new().route(
        "/{hash}",
        get(move |path| get_paste(path, server_buf.clone())),
    );

    let dir = dir.clone();
    tokio::spawn(async move {
        loop {
            let result = upload_server.accept().await;
            if let Err(err) = result {
                println!("err creating tcp stream: {}", err);
                continue;
            }
            let (mut socket, _addr) = result.unwrap();
            let dir = dir.clone();
            let external_url = external_url.clone();
            tokio::spawn(async move {
                let result = create_paste(&mut socket, dir, &external_url).await;

                if let Err(err) = result {
                    let _ = socket.write_all(format!("err: {}", err).as_bytes()).await;
                    return;
                }

                let endpoint = format!("{}\n", result.unwrap());
                let _ = socket.write_all(endpoint.as_bytes()).await;
            });
        }
    });

    axum::serve(http_server, server)
        .await
        .expect("failed to start server");
}
