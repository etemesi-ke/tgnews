extern crate lazy_static;

use crate::categories::enums::Categories;
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

lazy_static! {
    // TOKEN_CATEGORIES_SET = set(TOKEN_CATEGORIES.keys())
    static ref COMMON_VALUES:HashMap<&'static str,Categories> ={
         [("accidents",Categories::Society),("crime",Categories::Society),("geopolitics",Categories::Society),
         ("incident",Categories::Society),("incidents",Categories::Society),("politics",Categories::Society),
         ("politika",Categories::Society),("world",Categories::Society),("international",Categories::Society),

         ("current-affairs",Categories::Society),("social",Categories::Society),("society",Categories::Society),

         ("business",Categories::Economy),("economy",Categories::Economy),("economic",Categories::Economy),
         ("economics",Categories::Economy),("ekonomika",Categories::Economy),("finance",Categories::Economy),
         ("markets",Categories::Economy),("commercial",Categories::Economy),("biznes",Categories::Economy),
         ("market",Categories::Economy),("money",Categories::Economy),("stocks",Categories::Economy),

         ("baseball",Categories::Sports),("basketball",Categories::Sports),("cricket",Categories::Sports),
         ("football",Categories::Sports),("football-news",Categories::Sports),("futbol",Categories::Sports),
         ("rugby",Categories::Sports),("soccer",Categories::Sports),("sport",Categories::Sports),
         ("sports",Categories::Sports),("tennis",Categories::Sports),("sport-cat",Categories::Sports),

         ("bollywood",Categories::Entertainment),("entertainment",Categories::Entertainment),
         ("movies",Categories::Entertainment),("showbiz",Categories::Entertainment),("music",Categories::Entertainment),
         ("art",Categories::Entertainment),("fashion",Categories::Entertainment),("lifestyle",Categories::Entertainment),
         ("culture",Categories::Entertainment),("magazine",Categories::Entertainment),("tv-and-radio",Categories::Entertainment),
         ("bollywood",Categories::Entertainment),("beauty",Categories::Entertainment),("film",Categories::Entertainment),
         ("kultura",Categories::Entertainment),

         ("health",Categories::Science),("science",Categories::Science),("environment",Categories::Science),("neuroscience",Categories::Science),
         ("physics",Categories::Science),("chemistry",Categories::Science),("biology",Categories::Science),

         ("weather",Categories::Other),("travel",Categories::Other),("family",Categories::Other),
         ("food",Categories::Other),("family",Categories::Other),("travel",Categories::Other),("recipes",Categories::Other),
         ("horoscope",Categories::Other),

         ("tech",Categories::Technology),("technology",Categories::Technology),("gadgets",Categories::Technology)].iter().cloned().collect()
    };

}

pub fn classify_url(url: &str) -> Option<Categories> {
    let url = Url::from_str(url).expect("Could not parse url");
    let mut path = url.path().split('/').collect::<Vec<&str>>();
    // remove last item, has a lot of useless info
    path.pop();
    for i in path {
        if let Some(x) = COMMON_VALUES.get(i) {
            return Some(*x);
        }
    }
    None
}
