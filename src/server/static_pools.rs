//! Contains function and lazy initialized statics for  dealing with Pooled executions eg
//! stuff that should be run asynchronously in pools
//!
//! Because rust async types are lazy(like me ) by default
use std::thread::sleep;
use std::time::{Duration, Instant};

use colored::*;
use futures::executor::{ThreadPool, ThreadPoolBuilder};
use futures::Future;
use protobuf::{parse_from_bytes, ProtobufResult};
use whatlang::Lang;

use crate::server::enums::HTMLData;
use crate::server::protos::server_files::ProtoFile;
use crate::server::{EN_CLUSTERS, RU_CLUSTERS};
use crate::server::{
    FINISHED_CLUSTERING_EN, FINISHED_CLUSTERING_RU, FINISHED_REBUILDING, GLOBAL_DBASE,
};

lazy_static! {
    /// A Global thread-pool use this for running  every other stuff except STATIC POOLS
    ///
    /// Aka run temporary asynchronous programs on this one
    ///
    /// Pool size is currently:`12`
    static ref GLOBAL_POOL: ThreadPool = ThreadPoolBuilder::new()
        .pool_size(12)
        .name_prefix("Global pool")
        .create()
        .expect("Could not create pool");
    /// Used for running STATIC POOLS aka stuff that will run forever
    ///
    /// Note: Not all processes run forever, some are dropped so there will be some extra pools
    /// but still, DON'T USE IT(jokes I meant do not touch it)
    ///
    /// Pool size is 7
    static ref STATIC_POOL:ThreadPool=ThreadPoolBuilder::new()
        .pool_size(7)
        .name_prefix("Static pool")
        .create()
        .expect("Could not create pool");
}
/// Run a future to completion in another thread other than the main one
pub fn pool<Fut>(future: Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    GLOBAL_POOL.spawn_ok(future)
}
/// A static pool for executing processes that will run forever
pub fn static_pool<Fut>(future: Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    STATIC_POOL.spawn_ok(future)
}
/// Flush all dirty IO to the Database,
/// guaranteeing in case of a system failure all previous io calls can be recovered
/// this is put in a static pool to prevent it from hogging the main thread
pub async fn flush_dirty_io() {
    loop {
        if let Err(e) = GLOBAL_DBASE.flush_async().await {
            error!("SLED ERROR, could not flush dirty io to file \n{}", e)
        };
        std::thread::sleep(std::time::Duration::from_secs(300))
    }
}
/// Execute all static pools in the Server
/// I.e all  the processes that will be running throughout the server
///
/// Processes are
/// * `rebuild`: to read articles from the DataBase
/// * `cluster`: Clustering articles
/// * `get_stats*`: Print Human friendly statistics to the terminal
/// * `flush_dirty_io`: Flush input to the database so in case of a crush we can recover(pro tip: always back up)
/// * `remove_stale_docs`:Remove documents that have expired
/// * `cluster_at_random_times`:Clusters at random times(where random is 5 minutes)
pub fn execute_static_pools() {
    static_pool(rebuild_async());
    static_pool(cluster());
    static_pool(get_stats());
    static_pool(get_stats_ru());
    // Create a pool for flushing dirty io, run every 5 minutes
    static_pool(flush_dirty_io());
    // Remove Stale documents
    static_pool(remove_stale_docs());
    static_pool(cluster_at_random_times());
}
/// Remove Stale documents from the Index
async fn remove_stale_docs() {
    loop {
        sleep(Duration::from_secs(600));
        if GLOBAL_DBASE.len() < 40_000{
            continue
        }
        debug!("Starting DBASE iteration");
        GLOBAL_DBASE.iter().for_each(|f| {
            if let Ok((key, value)) = f {
                // key is name, value is proto file
                if let Ok(instance) = parse_from_bytes(value.as_ref()) {
                    let to_html = HTMLData::from_proto(instance);
                    if to_html.calc_decay(10_000.) > 5.0 {
                        if let Err(e) = GLOBAL_DBASE.remove(key) {
                            error!("Error Removing document '{}' \n {}", to_html.file_name, e);
                        } else {
                            warn!("Removed stale file `{}`", to_html.file_name);
                        }
                    }
                }
            }
        });
    }
}
fn rebuild_sync() {
    let time_now = Instant::now();
    // Flush all documents and restart
    EN_CLUSTERS.write().unwrap().flush();
    RU_CLUSTERS.write().unwrap().flush();
    for i in GLOBAL_DBASE.iter() {
        if let Ok((_, value)) = i {
            let file: ProtobufResult<ProtoFile> = parse_from_bytes(value.as_ref());
            if file.is_err() {
                continue;
            }
            let html_file = HTMLData::from_proto(file.unwrap());
            match html_file.lang.unwrap() {
                Lang::Rus => RU_CLUSTERS.write().unwrap().add(&html_file),
                Lang::Eng => EN_CLUSTERS.write().unwrap().add(&html_file),
                _ => (),
            }
        }
    }
    eprintln!(
        "Finished reading `{}` files in  {} second(s)",
        GLOBAL_DBASE.len().to_string().green(),
        time_now.elapsed().as_secs().to_string().red()
    );
    // Haa unsafe code
    unsafe {
        FINISHED_REBUILDING = true;
    }
}
/// Print English statistics to the command line
async fn get_stats() {
    loop {
        sleep(Duration::from_secs(60));
        EN_CLUSTERS.read().unwrap().get_stats();
    }
}
/// Print Russian statistics to the command line
async fn get_stats_ru() {
    loop {
        sleep(Duration::from_secs(70));
        RU_CLUSTERS.read().unwrap().get_stats();
    }
}
/// Rebuild the clusters from the database
///
/// Pro tip: It's not a full database but a Key-Value one named `sled` damn n may I say that it's on fire
async fn rebuild_async() {
    rebuild_sync();
}
/// Main pool for clustering documents
///
/// This is ran  once, when the server starts to cluster articles already in the database
/// then dies of in a ret value in assembly stack
///
/// Thanks mate
///
/// There is another instance aka `cluster_every_five_minutes`(yes function name is on point) that clusters every 5 mins
/// Head over there to see magic
async fn cluster() {
    loop {
        unsafe {
            if !FINISHED_REBUILDING {
                // sleep 5 seconds and check again
                sleep(Duration::from_secs(5));
                continue;
            }
        }
        crossbeam_utils::thread::scope(|f| {
            f.spawn(move |_| cluster_en());
            f.spawn(move |_| cluster_ru());
        })
        .expect("Could not spawn threads");
        // we have finished clustering documents in the DBASE
        break;
    }
}
/// Cluster English articles
/// # Arguments
/// `rebuild_from_dbase`:Rebuild the whole cache from the Database
fn cluster_en() {
    let mut global_docs = EN_CLUSTERS.write().unwrap();
    global_docs.cluster();

    unsafe { FINISHED_CLUSTERING_EN = true };
}
/// Cluster Russian articles
/// # Arguments
/// `rebuild_from_dbase`:Okay am Just being lazy, it's me reading every 5 mins the DataBase(takes about 5 seconds
/// for 29000 files in an SSD) and re-clustering from there, this is to reflect on article deletions and such shenanigans because I have NO TIME
/// TO DELETE MATRICES dynamically( am not mad)
fn cluster_ru() {
    let mut global = RU_CLUSTERS.write().unwrap();
    global.cluster();
    unsafe { FINISHED_CLUSTERING_RU = true };
}
/// CLuster items every 5 minutes forever
///
/// Sadly, there is no magic :*(
async fn cluster_at_random_times() {
    loop {
        unsafe {
            // if we haven't finished rebuilding, sleep for 10 seconds
            if !FINISHED_REBUILDING {
                sleep(Duration::from_secs(10));
                continue;
            }
            sleep(Duration::from_secs(600));

            rebuild_sync();
            crossbeam_utils::thread::scope(|f| {
                f.spawn(move |_| cluster_en());
                f.spawn(move |_| cluster_ru());
            })
            .expect("Could not spawn lazy threads for our 5 minute clustering");
            eprintln!("10 minute clustering done see you in 10 minutes");
        }
    }
}
