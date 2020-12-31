//! News classifier features
//! Extends language module by checking if an article is news or not.

// Language detector
extern crate whatlang;
// Html parser
extern crate select;
// Directory walker
extern crate walkdir;
// Thread spawner
extern crate crossbeam_utils;
// Pretty print json
extern crate fasttext;

extern crate serde_json;
extern crate url;

/// Main module
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;

use regex::{Regex, RegexSet};
use select::document::Document;
use select::predicate::{Attr, Name};
use serde_json::json;
use whatlang::{Detector, Lang};

use crate::logger::tgnews_debug;
use crate::utils::split_files_for_threads;
use std::fs::read_to_string;
use std::path::Path;
use std::time::Instant;
use url::Url;

//Hold title and url of an article
pub struct DataDigest {
    title: String,
    url: String,
}

lazy_static! {
/// Names linked with ads,reviews ,how-tos etc
static ref BAD_STRING:RegexSet= RegexSet::new(&[r"(reasons to|review|quick start|interview|case study|can be|watch now|how to|guide to|will you|things in|can do|this day in time|steps to|ways on|types of|to get|top picks|need to|have to|must have|things to|will put|should have|this date|simple tip|to help you|why the|it's time|it is time|hands on|it's about|what to)\s+",
r"^[\d\s]*(do|does|why|what|how to|are|is|can|you|use|my|why|on|this|did|where|here|how|things|have)\s+"]).unwrap();

static ref BAD_STRING_RU:RegexSet = RegexSet::new(&[r"(причина|будут|интервью|обзор|быстрый старт|лучший,интервь|тематическое исследование|этот день|может быть|смотреть сейчас|пути|как|как|руководство|вы|вещи|можете сделать)\s",
r"^(елать|делает|почему|что|каk|есть|может|вы|использовать|мой|я|почему|по этому|сделал)\s"]).unwrap();

static ref LIST_REGEX:RegexSet = RegexSet::new(&[r"\d+\s*(акци|банальн|важн|вещ|вопрос|главн|животн|знаменит|качествен|книг|лайфхак|лучш|мобил|необычн|популяр|привыч|прилож|причин|признак|продукт|прост|професс|самы|способ|технолог|худш|урок|шаг|факт|фильм|экзотичес|adorable|big|beaut|best|creative|crunchy|easy|huge|fantastic|innovative|iconic|baking|inspiring|perfect|stunning|stylish|unconventional|unexpected|wacky|wondeful|worst|habit|event|food|gift|question|reason|sign|step|thing|tip|trick|way)",
r"^\d+.{0,16} (акци|банальн|важн|вещ|вопрос|главн|животн|знаменит|качествен|книг|лайфхак|лучш|мобил|необычн|популяр|привыч|прилож|причин|признак|продукт|прост|професс|самы|способ|технолог|худш|урок|шаг|факт|фильм|экзотичес|adorable|big|beaut|best|creative|crunchy|easy|huge|fantastic|innovative|iconic|baking|inspiring|perfect|stunning|stylish|unconventional|unexpected|wacky|wondeful|worst|habit|event|food|gift|question|reason|sign|step|thing|tip|trick|way)",
r"^(the|top|топ)[\s-]\d+"]).unwrap();

static ref SALE_REGEX:Regex = Regex::new("(on|for) sale|(anniversary|apple|huge|amazon|friday|monday|christmas|fragrance|%) sale").unwrap();

static ref BAD_PHRASES_REGEX:Regex = Regex::new("(смотреть онлайн|можно приобрести|стоит всего|со скидкой|лучшие скидки|составлен топ|простой способ|простейший способ|способа|способов|free download|shouldn\'t miss|of the week|рецепт|правила|the week in)").unwrap();

}
///Takes a vector containing articles in en and ru.
///And returns a formatted string
///```txt
///   {
///   articles:[
///         "13456576756.html",
///         "33224534322.html"
///        ...
///         ]
///    }
///```
fn format_for_output(articles: &[String]) {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({ "articles": articles })).unwrap()
    )
}

/// Check whether the title inferences to a non-news category.
/// `:param article:` An article formatted according to telegram's format
fn get_digest(soup: &Document) -> DataDigest {
    DataDigest {
        title: soup
            .find(Attr("property", "og:title"))
            .next()
            .unwrap()
            .attr("content")
            .unwrap()
            .to_lowercase(),
        url: String::from(
            soup.find(Attr("property", "og:url"))
                .next()
                .unwrap()
                .attr("content")
                .unwrap(),
        ),
    }
}
pub fn server_is_news_en(title: String, url: String) -> bool {
    is_news_(DataDigest {
        title: title.to_lowercase(),
        url,
    })
}
pub fn server_is_news_ru(title: String, url: String) -> bool {
    is_news_ru_(DataDigest {
        title: title.to_lowercase(),
        url,
    })
}
pub fn is_news(soup: &Document) -> bool {
    let digest = get_digest(&soup);
    is_news_(digest)
}
pub fn is_news_(digest: DataDigest) -> bool {
    if SALE_REGEX.is_match(&digest.title)
        || LIST_REGEX.is_match(&digest.title)
        || BAD_PHRASES_REGEX.is_match(&digest.title)
    {
        return false;
    }
    let url = Url::parse(digest.url.as_str()).unwrap();

    // Contains /news/ in url
    if String::from(url.path()).contains("news") {
        return true;
    } else if digest.title.contains('?')
        || url.path().to_string().contains("blog")
        || url.path().to_string().contains("history")
        || url.path().to_string().contains("opinion")
    {
        return false;
    } else {
        // Compare with bad strings.
        if BAD_STRING.is_match(&digest.title) {
            return false;
        }
    }
    //If has less than 5 words
    let title_len: Vec<&str> = digest.title.split_ascii_whitespace().collect();
    if title_len.len() <= 3 {
        return false;
    }
    true
}
/// Check whether the title inferences to a non-news category for russian news.
/// `:param article:` An article formatted according to telegram's format
pub fn is_news_ru(soup: &Document) -> bool {
    let digest = get_digest(&soup);
    is_news_ru_(digest)
}
pub fn is_news_ru_(digest: DataDigest) -> bool {
    if SALE_REGEX.is_match(&digest.title)
        || LIST_REGEX.is_match(&digest.title)
        || BAD_PHRASES_REGEX.is_match(&digest.title)
        || BAD_STRING_RU.is_match(&digest.title)
    {
        return false;
    }
    true
}
/// Opens file passed to it  and  detects if its news
fn parse_files(filenames: &[String], news_clone: Arc<Mutex<Vec<String>>>) {
    /*
    Opens a file and parsers it using what i term as rusts beautiful soup
    It detects language and stores that in memory location somewhere
    */
    for file in filenames {
        if file.ends_with(".html") {
            // Read filename content to string
            let data = read_to_string(file).expect("Could not open file");
            // Extract data from html file using select.rs
            let soup = Document::from(data.as_str());
            let article = soup.find(Name("body")).next().unwrap().text();
            let detector = Detector::new();
            let lang = detector.detect(article.as_str());
            if let Some(lang) = lang {
                if (lang.lang() == Lang::Eng
                    && (lang.confidence() - 1.0).abs() < f64::EPSILON
                    && is_news(&soup))
                    || (lang.lang() == Lang::Rus
                        && (lang.confidence() - 1.0).abs() < f64::EPSILON
                        && is_news_ru(&soup))
                {
                    news_clone
                        .lock()
                        .unwrap()
                        .push(file.split('/').last().unwrap().to_owned());
                }
            }
        }
    }
}
/// Called to handle news sorting
pub fn entry(path: &str, thread: usize) {
    assert!(Path::new(path).exists(), "Path {:?} not found", path);
    // Get filenames in the directory
    let small_paths = split_files_for_threads(path.to_string(), thread);
    // Allocate the vector once
    let news = Arc::new(Mutex::new(Vec::with_capacity(10000)));
    //Spawn threads
    let tm = Instant::now();
    crossbeam_utils::thread::scope(|scope| {
        for range in small_paths {
            let news_clone = news.clone();
            scope.spawn(move |_| parse_files(range.as_slice(), news_clone));
        }
    })
    .expect("Could not spawn threads");
    tgnews_debug(format!(
        "Finished filtering news files in {} seconds",
        tm.elapsed().as_secs()
    ));
    tgnews_debug(format!(
        "Found {} news articles",
        news.lock().unwrap().len()
    ));
    format_for_output(&*news.lock().unwrap());
}
