use crate::alexa::ALEXA_RATINGS;
use crate::categories::enums::Categories;
use serde::export::Formatter;
use std::fmt::Debug;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

#[derive(Default, Clone)]
pub struct FileAnnotator {
    pub title: String,
    pub accuracy: f32,
    pub url: String,
    pub file: String,
    pub time: i64,
    pub importance: i32,
    pub vectors: Vec<f32>,
}
impl PartialEq for FileAnnotator {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
    }
}
impl Debug for FileAnnotator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            format!(
                "\ttitle:\t{}\n\tacc:\t{}\n\tfile:\t{}\n",
                self.title, self.accuracy, self.file
            )
            .as_str(),
        )
    }
}
impl FileAnnotator {
    pub fn new(
        title: String,
        accuracy: f32,
        file: String,
        time: i64,
        url: String,
        vectors: Vec<f32>,
    ) -> FileAnnotator {
        let mut a = FileAnnotator {
            title,
            accuracy,
            time,
            url,
            file,
            importance: 0,
            vectors,
        };
        a.calc_importance();
        a
    }
    pub fn calc_importance(&mut self) {
        let accuracy = (self.accuracy * 10.0).round() as i32;
        let time_diff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - self.time;
        let rating = {
            let host = Url::from_str(self.url.as_str())
                .unwrap()
                .host_str()
                .unwrap()
                .replace("www.", "");
            if let Some(ratings) = ALEXA_RATINGS.get(&host) {
                ratings.get_rating().round() as i32
            } else {
                1
            }
        };
        self.importance = accuracy * rating / ((time_diff as f32).log(10.0).round()) as i32;
    }
}
#[derive(Debug, Default, Clone)]
pub struct Annotator {
    society: Vec<FileAnnotator>,
    economy: Vec<FileAnnotator>,
    technology: Vec<FileAnnotator>,
    entertainment: Vec<FileAnnotator>,
    sports: Vec<FileAnnotator>,
    science: Vec<FileAnnotator>,
    other: Vec<FileAnnotator>,
    all: Vec<FileAnnotator>,
}
impl Annotator {
    pub fn new() -> Annotator {
        Annotator {
            society: Vec::with_capacity(1000),
            economy: Vec::with_capacity(1000),
            technology: Vec::with_capacity(1000),
            entertainment: Vec::with_capacity(1000),
            sports: Vec::with_capacity(1000),
            science: Vec::with_capacity(1000),
            other: Vec::with_capacity(1000),
            all: Vec::with_capacity(10000),
        }
    }
    pub fn push(
        &mut self,
        title: String,
        file: String,
        accuracy: f32,
        url: String,
        category: Categories,
        time: i64,
        values: Vec<f32>,
    ) {
        self.all.push(FileAnnotator::new(
            title.clone(),
            accuracy,
            file.clone(),
            time,
            url.clone(),
            values.clone(),
        ));
        match category {
            Categories::Society => self
                .society
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Sports => self
                .sports
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Technology => self
                .technology
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Entertainment => self
                .entertainment
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Other => self
                .other
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Science => self
                .science
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Economy => self
                .economy
                .push(FileAnnotator::new(title, accuracy, file, time, url, values)),
            Categories::Unknown => (),
        }
    }
    pub fn get_category(&self, cat: Categories) -> Vec<FileAnnotator> {
        match cat {
            Categories::Society => self.society.clone(),
            Categories::Sports => self.sports.clone(),
            Categories::Technology => self.technology.clone(),
            Categories::Entertainment => self.entertainment.clone(),
            Categories::Other => self.other.clone(),
            Categories::Science => self.science.clone(),
            Categories::Economy => self.economy.clone(),
            Categories::Unknown => unreachable!(),
        }
    }
    pub fn get_categories(&self) -> Vec<FileAnnotator> {
        self.all.clone()
    }
    // Get the length of all items in the Annotator
    pub fn len(&self) -> usize {
        self.all.len()
    }
}
