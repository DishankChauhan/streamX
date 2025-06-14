use std::convert::Infallible;
use warp::Filter;
use tracing::info;

pub struct HttpServer {
    port: u16,
    streams_dir: String,
}

impl HttpServer {
    pub fn new(port: u16, streams_dir: String) -> Self {
        Self { port, streams_dir }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("HTTP server listening on port {}", self.port);

        let routes = self.create_routes();

        warp::serve(routes)
            .run(([0, 0, 0, 0], self.port))
            .await;

        Ok(())
    }

    fn create_routes(&self) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        let static_files = warp::path("static")
            .and(warp::fs::dir("./static"));

        let index = warp::path::end()
            .map(|| warp::reply::html(include_str!("../static/index.html")));

        index.or(static_files)
    }
} 