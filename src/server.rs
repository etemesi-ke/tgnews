use std::sync::RwLock;

use rocket::config::Config;
use rocket::http::Status;
use rocket::logger::LogLevel;
use sled::{Config as SConfig, Mode};
use sled::Db;

use cluster::SClusterer;

use crate::server::static_pools::execute_static_pools;

pub mod cluster;
mod delete_article;
pub mod enums;
mod fast_text;
pub mod protos;
mod static_pools;
mod top;
mod upload;
lazy_static! {

     /// TODO: Change this path to be relative
     ///
     /// The first time it will be called it will create the file so there is no need
     /// to block the call, other needed libraries will be called dynamically
     pub static ref GLOBAL_DBASE:Db = {
            SConfig::default().path("./server/dbase".to_owned())
            .cache_capacity(96*1024*1024).mode(Mode::HighThroughput).open().expect("Could not create DB file")

    };
    pub static ref EN_CLUSTERS:RwLock<SClusterer>={
        RwLock::new(SClusterer::new("English"))
    };
    pub static ref RU_CLUSTERS:RwLock<SClusterer>={
        RwLock::new(SClusterer::new("Russian"))
    };

}
/// Whether the database has been rebuilt
pub static mut FINISHED_REBUILDING: bool = false;
/// Have English clusters been built
pub static mut FINISHED_CLUSTERING_EN: bool = false;
/// Have Russian clusters been built
pub static mut FINISHED_CLUSTERING_RU: bool = false;

/// Mount the server
///
/// This is the starting point for server part
pub async fn mount(port: u16) {
    execute_static_pools();

    let config = Config::figment()
        .merge(("port",port)).
        merge(("address","0.0.0.0"))
        .merge(("log_level","critical"));
    rocket::custom(config)
        .mount(
            "/",
            routes![
                get,
                upload::upload,
                upload::malformed_upload,
                delete_article::delete_file,
                top::get_top,
                top::get_all_top
            ],
        )
        .launch()
        .await
        .expect("Aww Snap, server crashed, should have spent more time here\n");
}
#[get("/")]
async fn get() -> Status {
    unsafe {
        return if FINISHED_REBUILDING {
            Status::Ok
        } else {
            Status::NotImplemented
        };
    }
}
