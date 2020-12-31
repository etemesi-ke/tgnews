use crate::server::{
    FINISHED_CLUSTERING_EN, FINISHED_CLUSTERING_RU, FINISHED_REBUILDING, GLOBAL_DBASE,
};
use rocket::http;
use rocket::http::Status;

/// Delete an article from the index
#[delete("/<article>")]
pub async fn delete_file(article: &http::RawStr) -> Status {
    unsafe {
        if !FINISHED_REBUILDING {
            warn!("Files not rebuilt, cannot process requests");
            return Status::ServiceUnavailable;
        } else if !FINISHED_CLUSTERING_EN && !FINISHED_CLUSTERING_RU {
            warn!("Either English Clustering or Russian clustering not finished, sending not implemented");
            return Status::ServiceUnavailable;
        }
    }

    let delete_lock = &GLOBAL_DBASE;
    match delete_lock.remove(article.as_bytes()) {
        // If there article exists return  NoContent, otherwise return NotFound
        Ok(result) => match result {
            Some(_) => http::Status::NoContent,
            None => http::Status::NotFound,
        },
        Err(_) => http::Status::NotFound,
    }
}
