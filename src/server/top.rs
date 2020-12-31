#![allow(unused_variables)]

use std::collections::BTreeMap;
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};

use colored::Colorize;
use rocket::{http, response};
use rocket::http::Status;
use rocket::Request;
use rocket::response::content;
use serde::Serialize;

use crate::server::{
    EN_CLUSTERS, FINISHED_CLUSTERING_EN, FINISHED_CLUSTERING_RU, FINISHED_REBUILDING, GLOBAL_DBASE,
    RU_CLUSTERS,
};
use crate::server::cluster::AllArticles;
use crate::slink::Articles;

pub struct PrettyJson<T>(T);

impl<T> Default for PrettyJson<T>
    where
        T: Default,
{
    fn default() -> Self {
        PrettyJson(T::default())
    }
}

impl<'a, T: Serialize> response::Responder<'a, 'static> for PrettyJson<T> {
    fn respond_to(self, request: &'a Request<'_>) -> response::Result<'static> {
        match request.client_ip() {
            Some(a) => {
                eprintln!(
                    "[PrettyJSON]: Responding to IP : {} for path: {} with query: {}",
                    a.to_string().red(),
                    request.uri().path().blue(),
                    request.uri().query().unwrap_or("stuff").green()
                );
            }
            None => (),
        }
        serde_json::to_string_pretty(&self.0)
            .map(|string| content::Json(string).respond_to(request).unwrap())
            .map_err(|_e| http::Status::InternalServerError)
    }
}

#[get("/threads?<period>&<lang_code>&category=any")]
pub async fn get_all_top(
    period: u64,
    lang_code: String,
) -> Result<PrettyJson<BTreeMap<String, Vec<AllArticles>>>, Status> {
    unsafe {
        if !FINISHED_REBUILDING {
            warn!("Files not rebuilt from database, cannot process requests");
            return Err(Status::ServiceUnavailable);
        } else if !FINISHED_CLUSTERING_EN && lang_code.as_str() == "en" {
            warn!("English clustering not finished, cannot handle en requests now");
            return Err(Status::ServiceUnavailable);
        } else if !FINISHED_CLUSTERING_RU && lang_code.as_str() == "ru" {
            warn!("RU clustering not finished returning service unavailable");
            return Err(Status::ServiceUnavailable);
        }
    }
    let mut files = match lang_code.as_str() {
        "en" => EN_CLUSTERS.write().unwrap().get_all(),
        "ru" => RU_CLUSTERS.write().unwrap().get_all(),
        _ => return Err(Status::BadRequest),
    };
    files = remove_more_stale_docs(files.as_slice(), period);
    files.sort_unstable_by(|a, b| a.articles.len().cmp(&b.articles.len()));
    files.reverse();
    let mut tree = BTreeMap::new();
    tree.insert("threads".to_string(), files);
    return Ok(PrettyJson(tree));
}

#[rustfmt::skip]
#[get("/threads?<period>&<lang_code>&<category>")]
pub async fn get_top(
    period: u64,
    lang_code: String,
    category: String,
) -> Result<PrettyJson<BTreeMap<String, Vec<Articles>>>, Status> {
    unsafe {
        if !FINISHED_REBUILDING {
            warn!("Files not rebuilt from database, cannot process requests");
            return Err(Status::ServiceUnavailable);
        } else if !FINISHED_CLUSTERING_EN && lang_code.as_str() == "en" {
            warn!("English clustering not finished, cannot handle en requests now");
            return Err(Status::ServiceUnavailable);
        } else if !FINISHED_CLUSTERING_RU && lang_code.as_str() == "ru" {
            warn!("RU clustering not finished returning service unavailable");
            return Err(Status::ServiceUnavailable);
        }
    }
    let mut articles = {
        match lang_code.as_str() {
            "en" => {
                EN_CLUSTERS.write().unwrap().cluster();
                match category.as_str() {
                    "society" => EN_CLUSTERS.read().unwrap().docs.society.read().unwrap().clone(),
                    "economy" => EN_CLUSTERS.read().unwrap().docs.economy.read().unwrap().clone(),
                    "technology" => EN_CLUSTERS.read().unwrap().docs.technology.read().unwrap().clone(),
                    "sports" => EN_CLUSTERS.read().unwrap().docs.sports.read().unwrap().clone(),
                    "entertainment" => EN_CLUSTERS.read().unwrap().docs.entertainment.read().unwrap().clone(),
                    "science" => EN_CLUSTERS.read().unwrap().docs.science.read().unwrap().clone(),
                    "other" => EN_CLUSTERS.read().unwrap().docs.other.read().unwrap().clone(),
                    _ => return Err(Status::BadRequest),
                }
            }
            "ru" => {
                // re-cluster
                RU_CLUSTERS.write().unwrap().cluster();
                match category.as_str() {
                    "society" => RU_CLUSTERS.read().unwrap().docs.society.read().unwrap().clone(),
                    "economy" => RU_CLUSTERS.read().unwrap().docs.economy.read().unwrap().clone(),
                    "technology" => RU_CLUSTERS.read().unwrap().docs.technology.read().unwrap().clone(),
                    "sports" => RU_CLUSTERS.read().unwrap().docs.sports.read().unwrap().clone(),
                    "entertainment" => RU_CLUSTERS.read().unwrap().docs.entertainment.read().unwrap().clone(),
                    "science" => RU_CLUSTERS.read().unwrap().docs.science.read().unwrap().clone(),
                    "other" => RU_CLUSTERS.read().unwrap().docs.other.read().unwrap().clone(),
                    _ => return Err(Status::BadRequest),
                }
            }
            _ => return Err(Status::BadRequest),
        }
    };
    articles = remove_stale_docs(articles.as_slice(), period);
    // sort by importance
    articles.sort_unstable_by(|a, b| a.articles.len().cmp(&b.articles.len()));
    articles.reverse();
    let mut tree = BTreeMap::new();
    tree.insert("threads".to_string(), articles);
    return Ok(PrettyJson(tree));
}

/// remove stale documents taking a mutable reference to the articles
///
/// If TTL doesn't exist we use the current system ,meaning we will return nothing
fn remove_stale_docs(articles: &[Articles], period: u64) -> Vec<Articles> {
    let ttl = match GLOBAL_DBASE.get(b"TTL") {
        Ok(ttl) => match ttl {
            None => SystemTime::from(UNIX_EPOCH).elapsed().unwrap().as_secs(),
            Some(time) => {
                let time_to_live: [u8; 8] = time.as_ref().try_into().unwrap();
                u64::from_be_bytes(time_to_live)
            }
        },
        Err(_) => SystemTime::from(UNIX_EPOCH).elapsed().unwrap().as_secs(),
    };
    articles
        .iter()
        .map(|a| a.remove_stale_documents(ttl, period))
        .filter_map(|a| a)
        .collect()
}

fn remove_more_stale_docs(articles: &[AllArticles], period: u64) -> Vec<AllArticles> {
    let ttl = match GLOBAL_DBASE.get(b"TTL") {
        Ok(ttl) => match ttl {
            None => SystemTime::from(UNIX_EPOCH).elapsed().unwrap().as_secs(),
            Some(time) => {
                let time_to_live: [u8; 8] = time.as_ref().try_into().unwrap();
                u64::from_be_bytes(time_to_live)
            }
        },
        Err(_) => SystemTime::from(UNIX_EPOCH).elapsed().unwrap().as_secs(),
    };
    articles
        .iter()
        .map(|a| a.remove_stale_docs(ttl, period))
        .filter_map(|a| a)
        .collect()
}
