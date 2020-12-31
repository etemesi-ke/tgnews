use serde::Deserialize;
use serde_json::from_str;
use std::collections::HashMap;
use std::fs::read_to_string;
lazy_static! {
/// Alexa Ratings are mapped to `{host:[info]}` dictionary
/// to allow for O(1) searching   and not O(N) iteration since most websites are not in the index
/// it becomes a worst case where O(N) is probably 10000 requests for a failing search
///
/// Initially, this may take some time as it is de-referenced  all of that splitting,deserializing and processing
/// of about 30000 lines of JSON , but subsequent calls should be cheaper
pub static ref ALEXA_RATINGS:HashMap<String,AlexaRating>={
        let fd = read_to_string("./data/alexa_rating.json").expect("Could not read alexa agency ratings");
        let values:Vec<AlexaRating>=from_str(fd.as_str()).expect("Could not convert string to JSON array");
        let mut m = HashMap::with_capacity(values.len());
        for i in values{
            m.insert(i.host.clone(),i);
        }
        m
    };
}
/// Defines One instance of an Alexa Rating
#[derive(Deserialize, Debug)]
pub struct AlexaRating {
    pub host: String,
    rating: f32,
    country: HashMap<String, f64>,
}

impl AlexaRating {
    /// Checks whether a URL matches this instance
    /// of Alexa rating
    pub fn matches(&self, url: &str) -> bool {
        if self.host == url {
            return true;
        }
        false
    }
    /// Get Country rating for a certain url
    ///
    /// # Params
    /// `country` is a maximum of two ISO country codes
    ///
    /// Returns `None` if the country is not present
    pub fn get_country(&self, country: &str) -> Option<&f64> {
        self.country.get(country)
    }
    /// Get The alexa Global rating for this url
    pub fn get_rating(&self) -> f32 {
        self.rating
    }
}
