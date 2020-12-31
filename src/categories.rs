use crate::categories::classifiers::classify_url;
use crate::categories::enums::Categories;
use crate::logger::tgnews_debug;
use crate::news::{is_news, is_news_ru};
use crate::utils::{clean, split_files_for_threads};
use fasttext::FastText;
use select::document::Document;
use select::predicate::{Attr, Name};
use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use whatlang::{Detector, Lang};

pub mod classifiers;
pub mod enums;

lazy_static! {
      // Model holders
        pub static ref EN_MODEL: FastText = {
                    let mut m = fasttext::FastText::new();
                    m.load_model("/home/caleb/tgnews/data/en_cat_v1.ftz").unwrap();
                    m
               };
        pub static ref RU_MODEL:FastText ={
            let mut m = fasttext::FastText::new();
            m.load_model("/home/caleb/tgnews/data/ru_cat.ftz").unwrap();
            m
         };
}
/// Classify Server articles
pub fn classify_en_with_accuracy(url: String, body: String) -> (Categories, f32) {
    if let Some(from_url) = classifiers::classify_url(url.as_str()) {
        return (from_url, 0.95);
    }
    let category = EN_MODEL.predict(&body, 1, 0.0).unwrap();
    if let Some(c) = category.first() {
        // Texts with less than 48 probability return unknown
        if c.prob > 0.45 {
            return match c.label.as_str() {
                "__label__society" => (Categories::Society, c.prob),
                "__label__economy" => (Categories::Economy, c.prob),
                "__label__other" => (Categories::Other, c.prob),
                "__label__entertainment" => (Categories::Entertainment, c.prob),
                "__label__technology" => (Categories::Technology, c.prob),
                "__label__science" => (Categories::Science, c.prob),
                "__label__sports" => (Categories::Sports, c.prob),
                _ => unreachable!(),
            };
        } else {
            return (Categories::Unknown, c.prob);
        }
    }
    (Categories::Unknown, 1.0)
}
/// Classify English Categories
pub fn classify_en(doc: &Document) -> Categories {
    let url = doc
        .find(Attr("property", "og:url"))
        .next()
        .unwrap()
        .attr("content")
        .unwrap();
    if let Some(from_url) = classifiers::classify_url(url) {
        return from_url;
    }
    let mut body = String::with_capacity(1000);
    doc.find(Name("p")).for_each(|f| body.push_str(&f.text()));
    let cleaned = clean(body, false);
    // Texts that came back to small remove em
    if cleaned.len() < 60 {
        return Categories::Unknown;
    }
    let category = EN_MODEL.predict(&cleaned, 1, 0.0).unwrap();
    if let Some(c) = category.first() {
        // Texts with less than 48 probability return unknown
        if c.prob > 0.45 {
            return match_label(c.label.as_str());
        };
    }
    Categories::Unknown
}
fn classify_ru(doc: &Document) -> Categories {
    let url = doc
        .find(Attr("property", "og:url"))
        .next()
        .unwrap()
        .attr("content")
        .unwrap();
    let title = doc
        .find(Attr("property", "og:title"))
        .next()
        .unwrap()
        .attr("content")
        .unwrap();
    if let Some(cat) = classify_url(url) {
        return cat;
    }
    let mut body = String::with_capacity(1000);
    doc.find(Name("p")).for_each(|f| body.push_str(&f.text()));
    let cleaned = clean(body, false) + title;
    // Texts that came back to small remove em
    if cleaned.len() < 10 {
        return Categories::Unknown;
    }
    let category = RU_MODEL.predict(&cleaned, 1, 0.0).unwrap();
    if let Some(c) = category.first() {
        // Texts with less than 43 probability return unknown
        if c.prob > 0.43 {
            return match_label(c.label.as_str());
        }
    }
    Categories::Unknown
}
fn match_label(c: &str) -> Categories {
    return match c {
        "__label__society" => Categories::Society,
        "__label__economy" => Categories::Economy,
        "__label__other" => Categories::Other,
        "__label__entertainment" => Categories::Entertainment,
        "__label__technology" => Categories::Technology,
        "__label__science" => Categories::Science,
        "__label__sports" => Categories::Sports,
        // we can be sure that it wont reach here since our data has only
        // 7  categories
        _ => unreachable!(),
    };
}
pub fn classify_ru_with_accuracy(title: String, url: String, body: String) -> (Categories, f32) {
    if let Some(cat) = classify_url(url.as_str()) {
        return (cat, 0.90);
    }
    let cleaned = title + body.as_str();
    // Texts that came back to small remove em
    if cleaned.len() < 10 {
        return (Categories::Unknown, 1.0);
    }
    let category = RU_MODEL.predict(&cleaned, 1, 0.0).unwrap();
    if let Some(c) = category.first() {
        // Texts with less than 43 probability return unknown
        if c.prob > 0.40 {
            return match c.label.as_str() {
                "__label__society" => (Categories::Society, c.prob),
                "__label__economy" => (Categories::Economy, c.prob),
                "__label__other" => (Categories::Other, c.prob),
                "__label__entertainment" => (Categories::Entertainment, c.prob),
                "__label__technology" => (Categories::Technology, c.prob),
                "__label__science" => (Categories::Science, c.prob),
                "__label__sports" => (Categories::Sports, c.prob),
                _ => unreachable!(),
            };
        }
        return (Categories::Unknown, c.prob);
    }
    (Categories::Unknown, 1.0)
}
#[rustfmt::skip]
#[allow(clippy::too_many_arguments)]
fn classify(
    paths: &[String],
    society: Arc<Mutex<Vec<String>>>,
    economy: Arc<Mutex<Vec<String>>>,
    entertainment: Arc<Mutex<Vec<String>>>,
    technology: Arc<Mutex<Vec<String>>>,
    sports: Arc<Mutex<Vec<String>>>,
    science: Arc<Mutex<Vec<String>>>,
    other: Arc<Mutex<Vec<String>>>,
) {
    for file in paths {
        if file.ends_with(".html") {
            let fd = read_to_string(file).expect("Could not read file to string");
            let doc = Document::from(fd.as_str());
            let article = doc.find(Name("body")).next().unwrap().text();
            let detector = Detector::new().detect(&article);

            if let Some(lang)=detector {
                    if (lang.lang() == Lang::Eng && (lang.confidence() - 1.0).abs() < f64::EPSILON  && is_news(&doc) )||
                        (lang.lang() == Lang::Rus && (lang.confidence() - 1.0).abs() < f64::EPSILON && is_news_ru(&doc) )

                    {
                        let category = {
                                match lang.lang(){
                                    Lang::Eng=>{classify_en(&doc)}
                                    Lang::Rus=>{classify_ru(&doc)}
                                    _=>unreachable!()
                                }
                        };
                        match category{
                            Categories::Society => {
                                society.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Economy => {
                                economy.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Entertainment => {
                                entertainment.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Sports => {
                                sports.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Technology => {
                                technology.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Science => {
                                science.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Other => {
                                other.lock().unwrap()
                                    .push(file.split('/').last().unwrap().to_string());
                            }
                            Categories::Unknown=> {

                            }
                        }
                    }
            }
        }
    }
}
pub fn classifier_entry(path: &str, threads: usize) {
    assert!(Path::new(path).exists(), "Path '{}' does not exist", path);
    let small_paths = split_files_for_threads(path.to_string(), threads);
    // Declare variables for types, not my best work but meh
    let society = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    let economy = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    let technology = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    let sports = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    let entertainment = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    let science = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    let other = Arc::new(Mutex::new(Vec::with_capacity(1000)));
    tgnews_debug(format!(
        "Files split into  {} for {} worker threads",
        small_paths[0].len(),
        threads
    ));
    let time_now = Instant::now();
    crossbeam_utils::thread::scope(|s| {
        for range in small_paths {
            let society_clone = society.clone();
            let economy_clone = economy.clone();
            let technology_clone = technology.clone();
            let sports_clone = sports.clone();
            let entertainment_clone = entertainment.clone();
            let science_clone = science.clone();
            let other_clone = other.clone();
            s.spawn(move |_| {
                classify(
                    range.as_slice(),
                    society_clone,
                    economy_clone,
                    entertainment_clone,
                    technology_clone,
                    sports_clone,
                    science_clone,
                    other_clone,
                )
            });
        }
    })
    .expect("Could not spawn threads");
    tgnews_debug(format!(
        "Spent {} seconds categorizing files",
        time_now.elapsed().as_secs()
    ));
    format_output(
        &*society.lock().unwrap(),
        &*economy.lock().unwrap(),
        &*technology.lock().unwrap(),
        &*entertainment.lock().unwrap(),
        &*sports.lock().unwrap(),
        &*science.lock().unwrap(),
        &*other.lock().unwrap(),
    );
}

fn format_output(
    society: &[String],
    economy: &[String],
    technology: &[String],
    entertainment: &[String],
    sports: &[String],
    science: &[String],
    other: &[String],
) {
    tgnews_debug(format!("Society:\t{}\n\tEconomy:\t{}\n\tTechnology:\t{}\n\tSports:\t\t{}\n\tEntertainment:\t{}\n\tScience:\t{}\n\tOther:\t\t{}\n",
                         society.len(),economy.len(),technology.len(),sports.len(),entertainment.len(),science.len(),other.len()));
    let vec_holder = vec![
        json!({"category":"society","articles":society}),
        json!({"category":"economy","articles":economy}),
        json!({"category":"technology","articles":technology}),
        json!({"category":"sports","articles":sports}),
        json!({"category":"entertainment","articles":entertainment}),
        json!({"category":"science","articles":science}),
        json!({"category":"other","articles":other}),
    ];
    println!("{}", serde_json::to_string_pretty(&vec_holder).unwrap());
}
