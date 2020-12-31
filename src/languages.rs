//! Language detection features
//! This function is called to detect languages in a given directory
// Language detector
extern crate whatlang;
// Html parser
extern crate select;
// Directory walker
extern crate walkdir;
// Thread spawner
extern crate crossbeam_utils;
// Pretty print json
extern crate serde_json;

use crate::logger;
use select::document::Document;
use select::predicate::Name;
/// Language module
/// Most functions are private except entry
/// which is the entry point for the module.
/// After getting directory, we handle everything else
use std::fs::read_to_string;
use std::sync::{Arc, Mutex};

use serde_json::json;

use crate::logger::tgnews_debug;
use crate::utils::split_files_for_threads;
use std::time::Instant;
use whatlang::{Detector, Lang};

///Format output as specified
///Takes a vector containing articles in en and ru.
///And returns a formatted string
///```
///    lang_code:"en":
///    {
///         articles:[
///         "13456576756.html"
///        ...
///         ]
///    }
///    lang_code:"ru"
///```
fn format_for_output(en_articles: &[String], ru_articles: &[String]) {
    let vec_holder = vec![
        json!({"lang_code":"en","articles":en_articles}),
        json!({"lang_code":"ru","articles":ru_articles}),
    ];
    //Pretty print JSON
    println!("{}", serde_json::to_string_pretty(&vec_holder).unwrap());
}

/// Opens file passed to it  and  detects languages
/// If the language is English or Russian, adds it to either the `en` mutex or `ru` mutex
fn parse_files(
    filenames: &[String],
    en_clone: Arc<Mutex<Vec<String>>>,
    ru_clone: Arc<Mutex<Vec<String>>>,
) {
    for file in filenames {
        // Don't open anything not ending with html
        if file.ends_with(".html") {
            // Read filename content to string

            // Use buffered IO to increase reading time
            let data = read_to_string(file).expect("Could not open file");

            // Extract data from html file using select.rs
            let soup = Document::from(data.as_str());
            let article = soup.find(Name("body")).next().unwrap().text();
            // Detect language
            let detector = Detector::new();
            let lang = detector.detect(article.as_str());
            if let Some(lang) = lang {
                //Add to our mutex
                if lang.lang() == Lang::Eng && (lang.confidence() - 1.0).abs() < f64::EPSILON {
                    en_clone
                        .lock()
                        .unwrap()
                        .push(file.split('/').last().unwrap().to_owned());
                } else if lang.lang() == Lang::Rus && lang.is_reliable() {
                    ru_clone
                        .lock()
                        .unwrap()
                        .push(file.split('/').last().unwrap().to_owned());
                }
            }
        }
    }
}
/// Entry point for all submodules
/// Takes a `path` which is a directory containing html files.
/// And a `thread` which specifies how many threads to spawn
pub fn entry(path: &str, thread: usize) {
    logger::tgnews_debug("Mode: \tLanguages");
    // Get filenames in the directory
    let small_paths = split_files_for_threads(path.to_string(), thread);
    // Declare mutable variables
    let en = Arc::new(Mutex::new(Vec::with_capacity(10000)));
    let ru = Arc::new(Mutex::new(Vec::with_capacity(10000)));
    //Spawn threads
    tgnews_debug(format!(
        "Files split into {} for {} worker threads",
        small_paths[0].len(),
        thread
    ));
    let time_now = Instant::now();
    crossbeam_utils::thread::scope(|scope| {
        for range in small_paths {
            let en_clone = en.clone();
            let ru_clone = ru.clone();
            scope.spawn(move |_| parse_files(range.as_slice(), en_clone, ru_clone));
        }
    })
    .expect("Could not spawn threads");
    tgnews_debug(format!(
        "Finished classifying languages in {} seconds",
        time_now.elapsed().as_secs()
    ));
    let en_v = &*en.lock().unwrap();
    let ru_v = &*ru.lock().unwrap();
    tgnews_debug(format!(
        "English articles:\t{}\n\tRussian articles:\t{}\n",
        en_v.len(),
        ru_v.len()
    ));
    // Call formatter
    format_for_output(en_v, ru_v);
}
