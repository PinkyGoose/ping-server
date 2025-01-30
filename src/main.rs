use axum::extract::State;
use axum::{
    extract::ConnectInfo,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Router,
};
use clap::Parser;
use std::net::IpAddr;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::TcpListener;
use tokio::time;
mod cli {
    use clap::Parser;
    use std::net::SocketAddr;

    #[derive(Parser, Debug)]
    #[clap(version, about)]
    pub struct Args {
        #[clap(long, env, default_value_t = 1000)]
        pub stats_timeout_ms: u64,
        /// Адрес, на котором будет запущен сервис.
        #[clap(long, env, default_value_t = SocketAddr::from(([0, 0, 0, 0], 8080)))]
        pub listen_addr: SocketAddr,
    }
}
#[derive(Clone)]
struct AppState {
    request_counts: Arc<Mutex<HashMap<IpAddr, usize>>>,
}
#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let state = AppState {
        request_counts: Arc::new(Mutex::new(HashMap::new())),
    };
    let request_counts = state.request_counts.clone();

    tokio::spawn(async move {
        loop {
            time::sleep(Duration::from_millis(args.stats_timeout_ms)).await;
            print_request_statistics(&request_counts).await;
        }
    });

    let app = Router::new()
        .route("/ping", get(ping_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_logger,
        ))
        .with_state(state);

    match TcpListener::bind(&args.listen_addr).await {
        Ok(tcp_listener) => {
            println!("Listening on http://{}", &args.listen_addr);
            axum::serve(
                tcp_listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        }
        Err(err) => {
            eprintln!(
                "не удалось привязаться к порту. выход из приложения: {:?}",
                err
            );
        }
    };
}

async fn ping_handler() -> &'static str {
    "pong"
}

async fn request_logger(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = addr.ip();

    {
        let mut counts = state.request_counts.lock().unwrap();
        *counts.entry(ip).or_insert(0) += 1;
    }

    next.run(req).await
}

async fn print_request_statistics(map: &Arc<Mutex<HashMap<IpAddr, usize>>>) {
    let mut counts = map.lock().unwrap();

    if !counts.is_empty() {
        println!("IPs:");
        let mut sorted_counts: Vec<_> = counts.iter().collect();
        sorted_counts.sort_by(|a, b| b.1.cmp(a.1));
        for (ip, count) in sorted_counts {
            println!("{:>15}: {}", ip, count);
        }
        counts.clear();
    }
}
