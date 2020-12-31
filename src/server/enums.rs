use crate::categories::enums::Categories;
use crate::server::protos::server_files::{Category, Language, ProtoFile};
use crate::utils::clean;
use select::document::Document;
use select::predicate::{Attr, Name};
use std::convert::TryInto;
use std::time::{SystemTime, UNIX_EPOCH};
use whatlang::Lang;

#[derive(Clone, Debug)]
pub struct HTMLData {
    pub title: String,
    pub url: String,
    pub file_name: String,
    pub date_published: u64,
    pub lang: Option<Lang>,
    pub category: Categories,
    pub alexa_rating_us: f64,
    pub alexa_rating_rus: f64,
    pub global_rating: f64,
    pub accuracy: f32,
    pub body: String,
}
pub enum HTErr {
    NoCategory(f32),
}
impl HTMLData {
    /// Calculate the decay of an article
    ///
    /// If the decay is large means that we need to remove the article, if small we can continue to retain it
    ///
    /// Current formula tries to remove mainly documents with low ratings, low accuracy and a large time passed since published
    ///
    /// # Parameters
    /// `div`: This is the number to divide the current time with time elapsed since article published time to prevent overflows
    pub fn calc_decay(&self, div: f64) -> f64 {
        // Should not overflow as we are assured that time should not move backwards
        let time_passed = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time moved backwards")
            .as_secs()
            - self.date_published) as f64
            / div;
        let rating = {
            if self.global_rating > 100. {
                100.
            } else {
                self.global_rating + 1.0
            }
        };
        //
        time_passed.exp()
            / (((self.accuracy as f64 * 10.)
                * rating
                * self.alexa_rating_us
                * self.alexa_rating_rus)
                + 1.0)
    }
    pub fn from_string(string: String) -> Option<HTMLData> {
        let doc = Document::from(string.as_str());
        let url = doc
            .find(Attr("property", "og:url"))
            .next()?
            .attr("content")?
            .to_string();
        let title = doc
            .find(Attr("property", "og:title"))
            .next()?
            .attr("content")?
            .to_string();
        let published_time = chrono::DateTime::parse_from_rfc3339(
            doc.find(Attr("property", "article:published_time"))
                .next()?
                .attr("content")?,
        )
        .unwrap()
        .timestamp();
        let mut body = String::with_capacity(1000);
        doc.find(Name("p")).for_each(|f| body.push_str(&f.text()));
        let body = clean(body, false);
        Some(HTMLData {
            title,
            url,
            // Will be set later
            file_name: "".to_string(),
            date_published: published_time.try_into().expect("Could not parse date"),
            // will be set later
            category: Categories::Unknown,
            accuracy: 0.0,
            alexa_rating_us: 1.,
            alexa_rating_rus: 1.,
            global_rating: 0.1,
            body,
            lang: None,
        })
    }
    /// Recover an instance of a HTML document from a Proto file
    /// Note that the body is set to " "since the body isn't saved( since we only need it
    /// for categorization) all, other info is recovered
    pub fn from_proto(file: ProtoFile) -> HTMLData {
        HTMLData {
            title: file.title,
            url: file.url,
            file_name: file.file_name,
            date_published: file.date_published.try_into().unwrap(),
            lang: {
                Some(match file.language {
                    Language::Eng => Lang::Eng,
                    Language::Rus => Lang::Rus,
                })
            },
            category: {
                match file.category {
                    Category::Society => Categories::Society,
                    Category::Economy => Categories::Economy,
                    Category::Technology => Categories::Technology,
                    Category::Entertainment => Categories::Entertainment,
                    Category::Sports => Categories::Sports,
                    Category::Science => Categories::Science,
                    Category::Other => Categories::Other,
                }
            },
            alexa_rating_us: file.us_rating as f64,
            alexa_rating_rus: file.ru_rating.into(),
            global_rating: file.gb_rating.into(),
            accuracy: file.accuracy,
            body: "".to_string(),
        }
    }
    /// Set language
    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = Some(lang)
    }
    pub fn set_file_name(&mut self, lang: String) {
        self.file_name = lang
    }
    pub fn set_alexa_rating_us(&mut self, rating: f64) {
        self.alexa_rating_us = rating
    }
    pub fn set_alexa_rating_rus(&mut self, rating: f64) {
        self.alexa_rating_rus = rating
    }
    /// Set category for the file
    /// The file should already be cleaned
    pub fn set_category_and_accuracy(&mut self) -> Result<(), HTErr> {
        let (category, accuracy) = {
            if self.lang == Some(Lang::Eng) {
                crate::categories::classify_en_with_accuracy(self.url.clone(), self.body.clone())
            } else {
                crate::categories::classify_ru_with_accuracy(
                    self.title.clone(),
                    self.url.clone(),
                    self.body.clone(),
                )
            }
        };
        if category == Categories::Unknown {
            return Err(HTErr::NoCategory(accuracy));
        }
        self.category = category;
        self.accuracy = accuracy;

        Ok(())
    }
}
