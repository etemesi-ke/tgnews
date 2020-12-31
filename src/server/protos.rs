//! Contains only one function
//!
//! To write to a DataBase back-end
//!
//! Only that,
//!
//! Ps its async
use std::convert::TryInto;
use std::str::FromStr;

use protobuf::Message;
use url::Url;
use whatlang::Lang;

use server_files::Category;
use server_files::Language;
use server_files::ProtoFile;

use crate::categories::enums::Categories;
use crate::server::enums::HTMLData;
use crate::server::GLOBAL_DBASE;

pub mod server_files;
/// Write a file to the database
pub async fn write_to_dbase(article: &HTMLData) {
    let parsed_url = Url::from_str(article.url.as_str())
        .unwrap()
        .domain()
        .unwrap()
        .to_owned();

    let mut file = ProtoFile::new();
    file.set_file_name(article.file_name.clone());
    file.set_title(article.title.clone());
    file.set_accuracy(article.accuracy);
    file.set_date_published(
        article
            .date_published
            .try_into()
            .expect("Could not convert date to an i64, seems its below 1970"),
    );
    // Match categories
    file.set_category(match article.category {
        Categories::Society => Category::Society,
        Categories::Economy => Category::Economy,
        Categories::Sports => Category::Sports,
        Categories::Entertainment => Category::Entertainment,
        Categories::Technology => Category::Technology,
        Categories::Science => Category::Science,
        Categories::Other => Category::Other,
        Categories::Unknown => unreachable!(),
    });
    // Languages
    file.set_language(match article.lang {
        Some(Lang::Eng) => Language::Eng,
        Some(Lang::Rus) => Language::Rus,
        _ => unreachable!(),
    });
    file.set_us_rating(article.alexa_rating_us as f32);
    file.set_ru_rating(article.alexa_rating_rus as f32);
    file.gb_rating = article.global_rating as f32;
    file.set_url(parsed_url.clone().replace("www.", ""));
    // Acquire lock to prevent concurrent writes which is
    // Also update global time in the DBASE to be the one with the most recent article
    let x = &GLOBAL_DBASE;
    x.insert(article.file_name.as_bytes(), file.write_to_bytes().unwrap())
        .expect("Could not add value to DBASE");
    update_ttl(article.date_published);
    debug!("Article `{}` written to DBASE", article.file_name);
}
/// Update the time to live when a new article arrives
fn update_ttl(new_time: u64) {
    let value = match GLOBAL_DBASE.get(b"TTL") {
        Ok(ttl) => match ttl {
            None => {
                warn!("TTL value was none, defaulting to zero");
                0
            }
            Some(time) => {
                let time: [u8; 8] = time.as_ref().try_into().unwrap();
                let old_time = u64::from_be_bytes(time);
                if new_time > old_time {
                    new_time
                } else {
                    old_time
                }
            }
        },
        Err(e) => {
            error!("Global TTL was not found, defaulting to 0\n{}", e);
            0
        }
    };
    GLOBAL_DBASE
        .insert(b"TTL", value.to_be_bytes().as_ref())
        .expect("Could not update time to Live for article");
}
