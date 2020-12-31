//! Okay this code is as optimized as code that was done in a day is.
//!
//! Which translates to `haa welcome to my Jarnik`
//!
//! There two functions here:
//! * `cluster()` is pretty easy,makes sense and won;t drive you mad
//!  it takes a document , checks if it's category is below thresh-hold(aka 0.45) and if not tries to
//! add an Alexa rating for it, then adds it to the appropriate cluster and calls `write_proto` to write the file
//! as a protobuf bytes and sends those bytes to sled, the most amazing key value database store ever created
//!
//! * Every other thing doesn't make sense actually, you're better off decoding photoshop PSD's format than doing this
//!
//! Because , f*ck rust and it's type systems(look at `SCluster.cluster()` function),
//! But a simple overview
//!  * `SCluster` -> Contains a higher level implementation of my weird slink algorithm
//!  * `Docs` -> Contains clustered documents, (clustering is done asynchronously)
//!  * `Unclustered`-> Contains all unclustered documents(since inception, probably a reason why this thing hogs memory)
//!  * `AllArticles`-> Contains an representation of the format required by a request to `/threads?period=sometime&category=any&lang_code=something`
//!
//! Hey you know what, figure out the rest, I can't make this boring, GOOD LUCK

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use chrono::Local;
use colored::*;
use eddie::Levenshtein;
use ndarray::{arr1, Array1, Array2, ArrayView1};
use ndarray_stats::QuantileExt;
use serde::Serialize;
use url::Url;
use whatlang::Lang;

use crate::alexa::ALEXA_RATINGS;
use crate::categories::enums::Categories;
use crate::server::enums::{HTErr, HTMLData};
use crate::server::protos::server_files::ProtoFile;
use crate::server::protos::write_to_dbase;
use crate::server::{EN_CLUSTERS, GLOBAL_DBASE, RU_CLUSTERS};
use crate::slink::{cosine, Articles};
use crate::threads::{EN_VECTORS, RU_VECTORS};
use crate::utils::clean;
use protobuf::parse_from_bytes;

const DECAY: f64 = 10_000.0;
/// Maximum number of files a cluster should have before breaking them when clustering
const MAX_FILES: usize = 9000;

/// Clusters document getting it's category and it's Alexa rating
///
/// And calling Write-Proto which saves the file
///
/// This is run in a separate thread from the main one so most heavy computations should be here
pub async fn cluster(article: HTMLData) {
    let mut article = article;
    // If we cannot classify it to a category return early

    if let Err(e) = article.set_category_and_accuracy() {
        match e {
            HTErr::NoCategory(f) => {
                if f != 1.0 {
                    warn!(
                        "Categories threshold below normal @  `{:.5}`, dropping article `{}`",
                        f, article.title
                    );
                } else {
                    warn!(
                        "Could not determine appropriate category for `{:?}` dropping it",
                        article.title
                    )
                }
            }
        }
        return;
    }
    // extract url and give it a rating if it exists
    let url = Url::from_str(article.url.as_str())
        .unwrap()
        .host_str()
        .unwrap()
        .replace("www.", "");
    if let Some(alexa_ratings) = ALEXA_RATINGS.get(url.as_str()) {
        if let Some(rating) = alexa_ratings.get_country("US") {
            match article.lang.unwrap() {
                Lang::Eng => article.set_alexa_rating_us(*rating),
                _ => {}
            }
        }
        if let Some(rating) = alexa_ratings.get_country("RU") {
            match article.lang.unwrap() {
                Lang::Rus => article.set_alexa_rating_rus(*rating),
                _ => {}
            }
        }
        article.global_rating = alexa_ratings.get_rating().into();
    }

    // add doc to respective cluster
    // note, the doc will not be added to a cluster until either 5 minutes elapses or we get a `GET`
    // request for threads
    match article.lang.unwrap() {
        Lang::Eng => {
            EN_CLUSTERS.write().unwrap().add(&article);
        }
        Lang::Rus => {
            RU_CLUSTERS.write().unwrap().add(&article);
        }
        _ => (),
    }
    write_to_dbase(&article).await;
}

#[derive(Default, Clone)]
pub struct SingleArticle {
    pub title: String,
    pub category: Categories,
    pub decay: i64,
    pub time: u64,
    pub file: String,
    pub embeddings: Vec<f32>,
}
impl SingleArticle {
    /// Construct a single article from a HTML Document
    pub fn from_html(h: &HTMLData) -> SingleArticle {
        let embeddings = match h.lang.unwrap() {
            Lang::Eng => EN_VECTORS.get_sentence_vector(clean(h.title.clone(), true).as_str()),
            Lang::Rus => RU_VECTORS.get_sentence_vector(clean(h.title.clone(), false).as_str()),
            _ => unreachable!(),
        };

        SingleArticle {
            title: h.title.clone(),
            decay: h.calc_decay(DECAY).round() as i64,
            category: h.category,
            time: h.date_published,
            file: h.file_name.clone(),
            embeddings,
        }
    }
}

#[derive(Default, Serialize, Clone)]
pub struct AllArticles {
    title: String,
    category: Categories,
    #[serde(skip)]
    pub decay: i64,
    #[serde(skip)]
    pub embeddings: Array1<f32>,
    #[serde(skip)]
    pub times: Vec<u64>,
    pub(crate) articles: Vec<String>,
}

impl AllArticles {
    pub fn from_single_article(files: &[SingleArticle]) -> AllArticles {
        let mut files = files.to_vec();
        let lev = Levenshtein::new();
        // sort by their importance
        // this is in ascending order
        files.sort_unstable_by(|a, b| a.decay.cmp(&b.decay));
        // reverse to get descending order
        let master = files[0].title.clone();
        let mut articles = vec![files[0].file.clone()];
        let mut embeddings = arr1(files[0].embeddings.as_slice());
        let mut times = vec![files[0].time];
        let category = files[0].category;
        let mut decay = 0;
        files.remove(0);
        // sort by lev distance, a shorter edit distance means they are closer
        files.sort_unstable_by_key(|a| lev.distance(master.as_str(), a.title.as_str()));
        articles.extend(files.iter().map(|f| {
            decay += f.decay;
            times.push(f.time);
            embeddings += &arr1(f.embeddings.as_slice());
            f.file.clone()
        }));
        decay /= ((articles.len() + 1) * (articles.len() + 1)) as i64;
        embeddings /= articles.len() as f32;
        AllArticles {
            title: master,
            category,
            decay,
            times,
            articles,
            embeddings,
        }
    }
    pub fn remove_stale_docs(&self, ttl_time: u64, stale_time: u64) -> Option<AllArticles> {
        let mut files = vec![];
        for (pos, document_time) in self.times.clone().into_iter().enumerate() {
            // the time to live time, aka latest article in document
            // minus current document time aka when the document was published,
            // if it is lesser  than the stale time,aka what we receive from the server,
            // add it to files
            if ttl_time - document_time < stale_time {
                if !files.contains(&self.articles[pos]) {
                    files.push(self.articles[pos].clone());
                }
            }
        }
        let mut single_articles = Vec::with_capacity(files.len());
        for i in files {
            match GLOBAL_DBASE.get(i.as_bytes()) {
                Ok(value) => match value {
                    None => {}
                    Some(finally) => {
                        let proto: ProtoFile = parse_from_bytes(finally.as_ref()).unwrap();
                        let html = HTMLData::from_proto(proto);
                        let file = SingleArticle::from_html(&html);
                        single_articles.push(file);
                    }
                },
                Err(_) => {}
            }
        }
        if single_articles.is_empty() {
            return None;
        }
        let articles = AllArticles::from_single_article(single_articles.as_slice());
        return Some(articles);
    }
}

#[derive(Default, Clone)]
pub struct Docs {
    pub society: Arc<RwLock<Vec<Articles>>>,
    pub economy: Arc<RwLock<Vec<Articles>>>,
    pub technology: Arc<RwLock<Vec<Articles>>>,
    pub entertainment: Arc<RwLock<Vec<Articles>>>,
    pub sports: Arc<RwLock<Vec<Articles>>>,
    pub science: Arc<RwLock<Vec<Articles>>>,
    pub other: Arc<RwLock<Vec<Articles>>>,
    pub all: Arc<RwLock<Vec<AllArticles>>>,
}

/// There are two instances of this running,
/// for en and ru
pub struct SClusterer {
    unclustered: Unclustered,
    pub docs: Docs,
    len: usize,
    name: String,
    modified: bool,
}

#[derive(Default)]
pub struct Unclustered {
    society: Vec<SingleArticle>,
    economy: Vec<SingleArticle>,
    technology: Vec<SingleArticle>,
    entertainment: Vec<SingleArticle>,
    sports: Vec<SingleArticle>,
    science: Vec<SingleArticle>,
    other: Vec<SingleArticle>,
}

impl Unclustered {
    pub fn push(&mut self, article: &HTMLData) {
        match article.category {
            Categories::Sports => {
                self.sports.push(SingleArticle::from_html(article));
            }
            Categories::Society => {
                self.society.push(SingleArticle::from_html(article));
            }
            Categories::Technology => {
                self.technology.push(SingleArticle::from_html(article));
            }
            Categories::Entertainment => {
                self.entertainment.push(SingleArticle::from_html(article));
            }
            Categories::Other => {
                self.other.push(SingleArticle::from_html(article));
            }
            Categories::Science => {
                self.science.push(SingleArticle::from_html(article));
            }
            Categories::Economy => {
                self.economy.push(SingleArticle::from_html(article));
            }
            Categories::Unknown => {}
        }
    }
}

impl SClusterer {
    /// Add an article to the Cluster
    pub fn add(&mut self, article: &HTMLData) {
        self.len += 1;
        self.unclustered.push(&article);
        self.modified = true;
    }
    pub fn new(name: &str) -> SClusterer {
        SClusterer {
            docs: Docs::default(),
            unclustered: Unclustered::default(),
            len: 0,
            name: String::from(name),
            modified: false,
        }
    }
    pub fn flush(&mut self) {
        self.len = 0;
        self.docs = Docs::default();
        self.modified = false;
        self.unclustered = Unclustered::default();
    }
    pub fn get_stats(&self) {
        let time_now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        println!("LANGUAGE:\t{}", self.name.green());
        eprintln!("Time now: {}", time_now.blue());
        eprintln!("Total files: {}", self.len.to_string().red());
        eprintln!("Clusters in each category:");
        eprintln!("\tSociety:{}\n\tEconomy:{}\n\tEntertainment:{}\n\tSports:{}\n\tTechnology:{}\n\tScience:{}\n\tOther:{}",
                  self.docs.society.read().unwrap().len(), self.docs.economy.read().unwrap().len(),
                  self.docs.entertainment.read().unwrap().len(),
                  self.docs.sports.read().unwrap().len(), self.docs.technology.read().unwrap().len(),
                  self.docs.science.read().unwrap().len(),
                  self.docs.other.read().unwrap().len()
        );
        eprintln!(
            "Database Size on disk: {} bytes\n\n",
            GLOBAL_DBASE
                .size_on_disk()
                .expect("Could not determine size")
        );
    }
    /// Cluster documents in the DataBase
    pub fn cluster(&mut self) {
        // if not modified no need to cluster em
        if !self.modified {
            return;
        }
        let time = Instant::now();
        const SMALL_CUTOFF: f32 = 0.12;
        const LARGE_CUTOFF: f32 = 0.15;
        self.docs = Docs::default();
        let f = self.docs.all.clone();
        crossbeam_utils::thread::scope(|s| {
            let a = f.clone();
            let b = f.clone();
            let c = f.clone();
            let d = f.clone();
            let e = f.clone();
            let g = f.clone();
            let h = f.clone();
            let society = self.docs.society.clone();
            let economy = self.docs.economy.clone();
            let tech = self.docs.technology.clone();
            let ent = self.docs.entertainment.clone();
            let sports = self.docs.sports.clone();
            let other = self.docs.other.clone();
            let science = self.docs.science.clone();

            let society1 = self.unclustered.society.clone();
            let economy1 = self.unclustered.economy.clone();
            let tech1 = self.unclustered.technology.clone();
            let ent1 = self.unclustered.entertainment.clone();
            let sports1 = self.unclustered.sports.clone();
            let other1 = self.unclustered.other.clone();
            let science1 = self.unclustered.science.clone();
            s.spawn(move |_| {
                cluster_articles(a, society, society1, LARGE_CUTOFF);
            });
            s.spawn(move |_| {
                // you were making ugly clusters
                cluster_articles(b, economy, economy1, SMALL_CUTOFF);
            });

            s.spawn(move |_| {
                // Ugliest clusters , soo much unrelated news
                cluster_articles(c, tech, tech1, SMALL_CUTOFF);
            });

            s.spawn(move |_| {
                // not so nice
                cluster_articles(d, science, science1, SMALL_CUTOFF);
            });

            s.spawn(move |_| {
                cluster_articles(e, ent, ent1, SMALL_CUTOFF);
            });

            s.spawn(move |_| {
                cluster_articles(g, other, other1, LARGE_CUTOFF);
            });

            s.spawn(move |_| {
                // UGLY
                cluster_articles(h, sports, sports1, SMALL_CUTOFF);
            });
        })
        .expect("Could not start clustering");
        println!(
            "TIME elapsed while clustering articles in {} : {}",
            self.name,
            time.elapsed().as_secs().to_string().blue()
        );
        self.modified = false;
    }
    pub fn get_docs(&mut self) -> Docs {
        if self.modified {
            self.cluster();
        }
        self.docs.clone()
    }
    pub fn get_all(&mut self) -> Vec<AllArticles> {
        if self.modified {
            self.cluster();
        }
        self.docs.all.clone().read().unwrap().to_vec()
    }
}

pub fn cluster_articles(
    global: Arc<RwLock<Vec<AllArticles>>>,
    local: Arc<RwLock<Vec<Articles>>>,
    docs: Vec<SingleArticle>,
    cutoff: f32,
) {
    if docs.len() < MAX_FILES {
        cluster_single(global, local, docs, cutoff)
    } else {
        cluster_multi(global, local, docs, cutoff)
    }
}

fn cluster_multi(
    global: Arc<RwLock<Vec<AllArticles>>>,
    local: Arc<RwLock<Vec<Articles>>>,
    docs: Vec<SingleArticle>,
    cutoff: f32,
) {
    let mut docs = docs;
    //sort unstable by using their time
    docs.sort_unstable_by(|a, b| a.time.cmp(&b.time));
    // spawn appropriate threads;
    let length = docs.len();
    // split it into batches of 8000
    let a = length / MAX_FILES;
    let mut each_group = Vec::with_capacity(a);
    let mut start = 0;
    let mut end = MAX_FILES;
    for i in 0..a {
        if i == a - 1 {
            each_group.push(&docs[start..docs.len()]);
        } else {
            each_group.push(&docs[start..end - 1]);
        }
        start += MAX_FILES;
        end += MAX_FILES;
    }
    crossbeam_utils::thread::scope(|s| {
        for i in each_group {
            let g = global.clone();
            let l = local.clone();
            s.spawn(move |_| cluster_single(g, l, i.to_vec(), cutoff));
        }
    })
    .expect("Could not spawn multi-threaded clustering threads");
}

/// Cluster individual articles into categories
///
/// #Params
/// `global`: The Global param will be updated with all articles regardless
/// `local` : The local cluster;
/// `docs`  : Raw unclustered documents
/// `cutoff`: A smaller cutoff means smaller more condensed clusters, larger cutoff means large
/// variative clusters(clusters with some similar and some not similar news)
fn cluster_single(
    global: Arc<RwLock<Vec<AllArticles>>>,
    local: Arc<RwLock<Vec<Articles>>>,
    docs: Vec<SingleArticle>,
    cutoff: f32,
) {
    if docs.len() < 10 {
        return;
    }
    let mut labels = Array1::<isize>::from_elem(docs.len(), -1);
    let (mut dissimilarity_matrix, min_dist) = build_dissimilarity(&docs);
    let mut cluster_num = 0;
    for i in 0..docs.len() {
        let min = *min_dist.get(i).unwrap();
        // means we have already checked this
        if i > min {
            continue;
        }
        // we already have their dissimilarities
        // the dissimilarity between i and j is in d_min[i,j]
        let e = dissimilarity_matrix[[i, min]];
        if e < cutoff {
            // Expand cluster
            let mut lowest_dissimilarity = e;
            let mut row = dissimilarity_matrix.row(i).to_owned();
            // whatever value we were in update it to 10 to prevent it from recurring
            row[i] = 10.0;
            labels[i] = cluster_num;
            labels[min] = cluster_num;
            while lowest_dissimilarity < cutoff {
                // we have a row of dissimilarity matrices, let's find the next
                // cluster with a minimum dissimilarity
                let pos = row.argmin_skipnan().unwrap();
                let x = row[pos];
                // check if it's below threshold
                if x > cutoff {
                    break;
                }
                // before we merge this, we need to check if it has a nearer cluster than this so fetch it's row again..
                let m: ArrayView1<f32> = dissimilarity_matrix.row(pos);
                let min_row = m.argmin_skipnan().unwrap();
                let n = m[min_row];
                if (x - m[min_row]).abs() > f32::EPSILON {
                    // okay we can merge it
                    labels[pos] = cluster_num;
                    // update the dissimilarity matrix at X
                    dissimilarity_matrix[[pos, min_row]] = 10.0;
                    lowest_dissimilarity = n;
                }
                row[pos] = 10.0;
            }
            cluster_num += 1;
        }
        //update min dist to match the new cluster
    }
    // update cluster
    let mut m = HashMap::with_capacity(10000);
    let mut negative_counters = *labels.max().unwrap() + 1;
    for (j, i) in labels.iter().enumerate() {
        if *i != -1 {
            let a = m.entry(*i).or_insert(vec![]);
            a.push(docs[j].clone());
        } else {
            m.insert(negative_counters, vec![docs[j].clone()]);
            negative_counters += 1;
        }
    }
    crossbeam_utils::thread::scope(|f| {
        let n = m.clone();
        f.spawn(move |_| {
            for i in m.values() {
                local
                    .write()
                    .unwrap()
                    .push(Articles::from_server(i.as_slice()));
            }
        });
        f.spawn(move |_| {
            for i in n.values() {
                global
                    .write()
                    .unwrap()
                    .push(AllArticles::from_single_article(i))
            }
        });
    })
    .expect("Could not update threads");
}
/// Build a dissimilarity matrix
///
/// The same as the one over in slink.rs except it takes a `SingleArticle` instead of a `FileAnnotator`
pub fn build_dissimilarity(values: &[SingleArticle]) -> (Array2<f32>, Array1<usize>) {
    let mut dissimilarity_matrix = Array2::<f32>::zeros((values.len(), values.len()));
    let mut closest_cluster = Array1::<usize>::ones(values.len());
    for (i, first) in values.iter().enumerate() {
        for (j, second) in values.iter().enumerate() {
            if i == j {
                // Comparing ourself with ourself
                dissimilarity_matrix[[i, j]] = 0.0;
            } else {
                let e = 1.0 - cosine(first.embeddings.as_slice(), second.embeddings.as_slice());
                dissimilarity_matrix[[i, j]] = e;
            }
            // Store the index with the lowest dissimilarity to X
            // also ensure i is not equal to j
            if dissimilarity_matrix[[i, j]] < dissimilarity_matrix[[i, closest_cluster[i]]]
                && i != j
            {
                closest_cluster[i] = j;
            }
        }
    }
    (dissimilarity_matrix, closest_cluster)
}
