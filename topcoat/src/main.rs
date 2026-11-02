mod app;
mod cart;
mod catalog;
mod images;
mod remote;
mod stock;
mod urls;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    // `start` binds to `HOST`:`PORT` (default `127.0.0.1:3000`). `topcoat dev`
    // sets both for the app it spawns; containers must set `HOST=0.0.0.0`,
    // since a loopback-only listener is unreachable from the platform proxy.
    topcoat::start(app::router()).await.unwrap();
}
