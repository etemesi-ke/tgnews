use std::fs::read_to_string;
use std::path::Path;
use std::sync::{Arc, Mutex};

use fasttext::FastText;
use select::document::Document;
use select::predicate::{Attr, Name};
use whatlang::{Detector, Lang};

use crate::categories::enums::Categories;
use crate::categories::enums::Categories::{
    Economy, Entertainment, Other, Science, Society, Sports, Technology,
};
use crate::categories::{classify_en_with_accuracy, classify_ru_with_accuracy};
use crate::news::{is_news, is_news_ru};
use crate::slink::{slink, Articles};
use crate::threads::annotator::Annotator;
use crate::threads::annotator::FileAnnotator;
use crate::utils::{clean, split_files_for_threads};

const SMALL_CUTOFF: f32 = 0.15;
const LARGE_CUTOFF: f32 = 0.12;

pub mod annotator;
lazy_static! {
    /// English Vectors
    pub static ref EN_VECTORS: FastText = {
        let mut m = FastText::new();
        m.load_model("./data/en_vectors.bin")
            .unwrap();
        m
    };
    /// RUSSIAN fastText vectors
    pub static ref RU_VECTORS:FastText={
        let mut m = FastText::new();
        m.load_model("./data/ru_vectors.bin").unwrap();
        m
    };
}
/// Objective:`Annotate documents`
///
/// Steps
/// * Read all HTML files
/// * Read all `p` tags and get their text
/// * If the text is `English` or `Russian`, `is news` and it's `category` is not `Unknown` we annotate it
/// * For annotation, we take the `title`,`url`,`timestamp`, and `category`
///
/// * Done
fn classify(paths: &[String], en_clone: Arc<Mutex<Annotator>>, ru_clone: Arc<Mutex<Annotator>>) {
    for file in paths {
        if file.ends_with(".html") {
            let fd = read_to_string(file).expect("Could not read file to string");
            let doc = Document::from(fd.as_str());
            let mut body = String::with_capacity(1000);
            doc.find(Name("p")).for_each(|f| body.push_str(&f.text()));
            let detector = Detector::new().detect(&body);
            if let Some(lang) = detector {
                if (lang.lang() == Lang::Eng
                    && (lang.confidence() - 1.0).abs() < f64::EPSILON
                    && is_news(&doc))
                    || (lang.lang() == Lang::Rus && lang.is_reliable() && is_news_ru(&doc))
                {
                    let title = doc
                        .find(Attr("property", "og:title"))
                        .next()
                        .unwrap()
                        .attr("content")
                        .unwrap()
                        .to_string();
                    let url = doc
                        .find(Attr("property", "og:url"))
                        .next()
                        .unwrap()
                        .attr("content")
                        .unwrap()
                        .to_string();
                    let published_time = chrono::DateTime::parse_from_rfc3339(
                        doc.find(Attr("property", "article:published_time"))
                            .next()
                            .expect(
                                "Could not find `article published time meta tag` in the document",
                            )
                            .attr("content")
                            .expect(
                                "Could not find content attribute in `article published time tag`",
                            ),
                    )
                    .expect("Could not parse date time info from file")
                    .timestamp();
                    let language = lang.lang();
                    let cleaned = clean(body.clone(), false);

                    let (text, accuracy) = {
                        match language {
                            Lang::Eng => classify_en_with_accuracy(url.clone(), cleaned),
                            Lang::Rus => {
                                classify_ru_with_accuracy(title.clone(), url.clone(), body)
                            }
                            _ => unreachable!(),
                        }
                    };
                    if Categories::Unknown == text {
                        continue;
                    }
                    match language {
                        Lang::Eng => en_clone.lock().expect("Could not acquire lock\n").push(
                            title.clone(),
                            file.split('/').last().unwrap().to_string(),
                            accuracy,
                            url,
                            text,
                            published_time,
                            EN_VECTORS.get_sentence_vector(clean(title, true).as_str()),
                        ),
                        Lang::Rus => ru_clone.lock().expect("Could not acquire lock\n").push(
                            title.clone(),
                            file.split('/').last().unwrap().to_string(),
                            accuracy,
                            url,
                            text,
                            published_time,
                            RU_VECTORS.get_sentence_vector(title.as_str()),
                        ),
                        _ => (),
                    }
                }
            }
        }
    }
}
pub fn entry(dir: &str, threads: usize) {
    assert!(Path::new(dir).exists(), "Paths {:?} doesn't exist", dir);
    let small_paths = split_files_for_threads(dir.to_string(), threads);
    let en = Arc::new(Mutex::new(Annotator::new()));
    let ru = Arc::new(Mutex::new(Annotator::new()));
    crossbeam_utils::thread::scope(|s| {
        for range in small_paths {
            let en_clone = en.clone();
            let ru_clone = ru.clone();
            s.spawn(move |_| classify(range.as_slice(), en_clone, ru_clone));
        }
    })
    .expect("Could not spawn threads");
    let u = &*en.lock().unwrap();
    let v = &*ru.lock().unwrap();
    let f = Arc::new(Mutex::new(Vec::with_capacity(10000)));
    crossbeam_utils::thread::scope(|s| {
        let g = f.clone();
        let h = f.clone();
        s.spawn(move |_| cluster_files(u, g));
        s.spawn(move |_| cluster_files(v, h));
    })
    .expect("Could not spawn clustering threads");
    // finally print output
    println!(
        "{}",
        serde_json::to_string_pretty(&*f.lock().unwrap()).unwrap()
    );
}
/// CLuster files
fn cluster_files(files: &Annotator, f: Arc<Mutex<Vec<Articles>>>) {
    let length = files.len();
    if length < 10000 {
        // for less than 3000 files, we classify them once
        thread_within_categories(files.get_categories(), SMALL_CUTOFF, f);
    } else {
        // break each individual into its category and send it to be clustered
        crossbeam_utils::thread::scope(|s| {
            // Rust btw: sometimes you just foolish
            //
            // So i can't just send an f.clone() because of the move keyword
            // so hello ugly code
            let a = f.clone();
            let b = f.clone();
            let c = f.clone();
            let d = f.clone();
            let e = f.clone();
            let g = f.clone();
            let h = f.clone();

            s.spawn(move |_| {
                thread_within_categories(files.get_category(Society), LARGE_CUTOFF, a)
            });
            s.spawn(move |_| {
                thread_within_categories(files.get_category(Economy), LARGE_CUTOFF, b)
            });
            s.spawn(move |_| thread_within_categories(files.get_category(Sports), 0.10, c));
            s.spawn(move |_| {
                thread_within_categories(files.get_category(Science), LARGE_CUTOFF, d)
            });
            s.spawn(move |_| {
                thread_within_categories(files.get_category(Entertainment), LARGE_CUTOFF, e)
            });
            s.spawn(move |_| thread_within_categories(files.get_category(Other), LARGE_CUTOFF, g));
            s.spawn(move |_| thread_within_categories(files.get_category(Technology), 0.10, h));
        })
        .expect("Error in spawned threads");
    }
}
fn thread_within_categories(c: Vec<FileAnnotator>, cutoff: f32, f: Arc<Mutex<Vec<Articles>>>) {
    slink(c.to_vec(), cutoff, f);
}
