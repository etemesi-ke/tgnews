use crate::logger::tgnews_debug;
use regex::Regex;
use std::collections::HashSet;
use walkdir::WalkDir;

lazy_static! {
        // Do nor remove full stops
        pub static ref PUNCTUATION:Regex=Regex::new(r"[!#$%&'()*+,-:\.;=?@\[\]\\^_`\{\|\}~]\s*").unwrap();
        /// Remove extra whitespaces from the text
        static ref REMOVE_EXTRA_SPACES:Regex= Regex::new(r"\s\s+").unwrap();
        /// Remove newlines with 0 or more spaces
        static ref NEWLINES:Regex= Regex::new("[\n]\\s*").unwrap();
        /// Load stop words
        /// The stop word list is stored in the app itself, one reason why the app is 10 mbs
        static ref STOPWORDS_EN:HashSet<&'static str>={
           include_str!("../data/stop-words.txt").split("\r\n").collect()
        };
}
///Clean text for clustering and threading
///
/// The cleaning process goes like
/// * Lowercase text
/// * Replace newlines
/// * Replace punctuation marks with a space character, preventing sentences like`he said.I love you` to be `he saidi love you`
/// * Remove stop words
/// * Stem words using porter stemmer
/// * Remove excess whitespaces
pub fn clean(doc: String, stem: bool) -> String {
    let lower = doc.to_ascii_lowercase();
    let mut new_val = String::with_capacity(doc.len());
    let lower = NEWLINES.replace_all(&lower, "").to_string();
    let cleaned = PUNCTUATION.replace_all(&lower, " ").to_string();
    for i in cleaned.split_whitespace() {
        if !STOPWORDS_EN.contains(i) {
            if stem {
                new_val.push_str(&format!("{} ", porter_stemmer::stem(i)));
                continue;
            }
            new_val.push_str(&format!("{} ", i));
        }
    }
    REMOVE_EXTRA_SPACES
        .replace_all(new_val.as_str(), " ")
        .to_string()
}

/// Iterate over all entries in a folder and extract all files
///
/// Split those files into groups for each worker thread
///
/// It is guaranteed to return a `Vec<Vec<[String]>>` with a length of `worker_threads`
pub fn split_files_for_threads(root: String, worker_threads: usize) -> Vec<Vec<String>> {
    let mut paths: Vec<String> = vec![];
    let mut small_paths = vec![];
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.is_dir() {
            paths.push(String::from(entry.path().to_str().unwrap()));
        }
    }
    tgnews_debug(format!("Read {} files", paths.len()));
    // Divide paths into segments.
    let size = paths.len() / worker_threads;
    let mut start: usize = 0;
    let mut end: usize = size;
    for a in 0..worker_threads {
        // Remove off by one error
        let value = {
            if a == worker_threads - 1 {
                paths[start..paths.len()].to_vec()
            } else {
                paths[start..end - 1].to_vec()
            }
        };
        small_paths.push(value);
        start += size;
        end += size;
    }
    small_paths
}
