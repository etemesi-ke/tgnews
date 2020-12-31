use std::time::{SystemTime, UNIX_EPOCH};

use rocket::data::{ByteUnit, FromData, Outcome};
use rocket::http::Status;

use crate::news::{server_is_news_en, server_is_news_ru};
use crate::server::enums::HTMLData;
use crate::server::static_pools::pool;
use crate::server::{
    FINISHED_CLUSTERING_EN, FINISHED_CLUSTERING_RU, FINISHED_REBUILDING, GLOBAL_DBASE,
};
use rocket::request::FromRequest;
use rocket::{http, request, Data, Request};
use whatlang::{Detector, Lang};

const LIMIT: ByteUnit = ByteUnit::Megabyte(12);

#[derive(Debug)]
pub enum HTMLError {
    Format(String),
}
#[rocket::async_trait]
impl FromData for HTMLData {
    type Error = HTMLError;

    async fn from_data(_: &Request<'_>, data: Data) -> Outcome<Self, Self::Error> {
        let string = match data.open(LIMIT).stream_to_string().await {
            Ok(st) => st,
            Err(e) => {
                return Outcome::Failure((
                    Status::InternalServerError,
                    HTMLError::Format(format!("{:?}", e)),
                ))
            }
        };
        match HTMLData::from_string(string) {
            Some(s) => Outcome::Success(s),
            None => Outcome::Failure((
                Status::UnprocessableEntity,
                HTMLError::Format("Could not process entity".to_string()),
            )),
        }
    }
}
#[allow(dead_code)]
pub struct CacheControl {
    cache_control: u64,
}
#[rocket::async_trait]
impl<'a, 'r> FromRequest<'a, 'r> for CacheControl {
    // Implements a fetcher to fetch our Cache-Control header
    // This is because we can't read headers in any other form
    type Error = ();
    async fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        const CACHE_CONTROL: &str = "Cache-Control";
        let agent = request.headers().get_one(CACHE_CONTROL);
        match agent {
            //If header exists, we convert it to string
            Some(a) => rocket::outcome::Outcome::Success(CacheControl {
                cache_control: a
                    .to_string()
                    .strip_prefix("max-age=")
                    .unwrap()
                    .parse::<u64>()
                    .unwrap(),
            }),
            // If not we send a bad request status code.
            None => rocket::outcome::Outcome::Failure((http::Status::BadRequest, ())),
        }
    }
}
#[put("/<article>", format = "html", data = "<html>")]
pub async fn upload(
    article: &rocket::http::RawStr,
    html: HTMLData,
    _cache_control: CacheControl,
) -> Status {
    unsafe {
        if !FINISHED_REBUILDING {
            return Status::ServiceUnavailable;
        } else if !FINISHED_CLUSTERING_EN && !FINISHED_CLUSTERING_RU {
            warn!("Either English Clustering or Russian clustering not finished, sending not implemented");
            return Status::ServiceUnavailable;
        }
    }
    // Check if the store contains the value,if it does skip the rest, am not overwriting values
    if let Ok(okay) = GLOBAL_DBASE.contains_key(article.as_bytes()) {
        if okay {
            warn!("Found existing entry `{}` skipping overwrite", article);
            return Status::NoContent;
        }
    }
    let lang_info = match Detector::new().detect(html.body.as_str()) {
        // TODO:ADD news filter
        Some(lang) => {
            if (lang.confidence() - 1.0).abs() < f64::EPSILON && lang.lang() == Lang::Eng {
                // For non-news articles return null
                if !server_is_news_en(html.title.clone(), html.url.clone()) {
                    return Status::NoContent;
                }
                Lang::Eng
            } else if (lang.confidence() - 1.0).abs() < f64::EPSILON && lang.lang() == Lang::Rus {
                if !server_is_news_ru(html.title.clone(), html.url.clone()) {
                    return Status::NoContent;
                }
                Lang::Rus
            } else {
                return Status::NoContent;
            }
        }
        None => return http::Status::NoContent,
    };
    let _time_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // TODO: Re-enable this restriction
    //if time_now-html.date_published > cache_control.cache_control{
    //  return Status::NoContent
    //}
    let mut html = html;

    html.set_lang(lang_info);
    html.set_file_name(article.to_string());
    // HELLO WORLD
    pool(crate::server::cluster::cluster(html.clone()));

    Status::Created
}
#[put("/<_article>", rank = 2)]
pub async fn malformed_upload(_article: &http::RawStr) -> http::Status {
    unsafe {
        if !FINISHED_REBUILDING {
            warn!("Files not rebuilt from database, cannot process requests");
            return Status::ServiceUnavailable;
        } else if !FINISHED_CLUSTERING_RU && !FINISHED_CLUSTERING_EN {
            warn!("Either English Clustering or Russian clustering not finished, sending not implemented");
            return Status::ServiceUnavailable;
        }
    }
    // If the request doesn't contain necessary information
    return http::Status::UnprocessableEntity;
}
