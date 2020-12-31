use std::collections::hash_map::Values;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::server::cluster::SingleArticle;
use crate::server::enums::HTMLData;
use crate::server::protos::server_files::ProtoFile;
use crate::server::GLOBAL_DBASE;
use crate::threads::annotator::FileAnnotator;
use eddie::Levenshtein;
use ndarray::{arr1, Array1, Array2, ArrayView1};
use ndarray_stats::QuantileExt;
use num_traits::Float;
use protobuf::parse_from_bytes;
use serde::Serialize;

#[derive(Serialize, Default, Clone)]
pub struct Articles {
    pub title: String,
    #[serde(skip)]
    pub decay: i64,
    #[serde(skip)]
    pub article_times: Vec<u64>,
    pub articles: Vec<String>,
}

impl Articles {
    pub fn from_annotator(files: Vec<FileAnnotator>) -> Articles {
        let mut files = files;
        let lev = Levenshtein::new();
        // sort by their importance
        // this is in ascending order
        files.sort_unstable_by(|a, b| a.importance.cmp(&b.importance));
        // reverse to get descending order
        files.reverse();
        let master = files[0].title.clone();
        let mut articles = vec![files[0].file.clone()];
        files.remove(0);
        // sort by lev distance, a shorter edit distance means they are closer
        files.sort_unstable_by_key(|a| lev.distance(master.as_str(), a.title.as_str()));
        articles.extend(files.iter().map(|f| f.file.clone()));
        Articles {
            title: master,
            decay: 0,
            article_times: vec![],
            articles,
        }
    }
    pub fn push(&mut self, file: &SingleArticle) {
        self.title = file.title.to_string();
        self.articles.insert(0, file.file.clone());
        self.article_times.insert(0, file.time);
    }
    /// Create an article from a server file
    pub fn from_server(files: &[SingleArticle]) -> Articles {
        let mut files = files.to_vec();
        // no need to reverse, those with the smallest decay should be on top
        files.sort_unstable_by(|a, b| a.decay.cmp(&b.decay));
        let master = files[0].title.clone();
        let mut articles = vec![files[0].file.clone()];
        let mut article_times = vec![files[0].time];
        files.remove(0);
        let lev = Levenshtein::new();
        let mut decay = 0;
        files.sort_unstable_by_key(|a| lev.distance(master.as_str(), a.title.as_str()));
        articles.extend(files.iter().map(|f| {
            decay += f.decay;
            article_times.push(f.time);
            f.file.clone()
        }));
        decay /= (files.len() + 1).pow(2) as i64;
        Articles {
            title: master,
            decay,
            article_times,
            articles,
        }
    }
    /// remove stale documents
    ///
    /// This is for server only, you others don't worry
    pub fn remove_stale_documents(&self, ttl_time: u64, stale_time: u64) -> Option<Articles> {
        let mut files = vec![];
        for (pos, document_time) in self.article_times.clone().into_iter().enumerate() {
            // the time to live time, aka latest article in document
            // minus current document time aka when the document was published,
            // if it is lesser  than the stale time,aka what we receive from the server,
            // add it to files
            if ttl_time - document_time < stale_time {
                files.push(self.articles[pos].clone());
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
        let articles = Articles::from_server(single_articles.as_slice());
        return Some(articles);
    }
    /// Check whether there is any sign of life in this cluster
    ///
    /// Again a server thing
    pub fn is_empty(&self) {}
}

/// The thresh-hold for combining news into clusters
///
/// Setting this to a higher value will lead to large clusters with irrelevant news
/// while setting to a smaller thresh-hold will lead to smaller clusters with quite similar news
const MAX_FILES: usize = 10000;
const BATCH_SIZE: usize = 8000;
/// Calculate the cosine distance between two slices
#[inline(always)]
pub fn cosine<T>(a: &[T], b: &[T]) -> T
where
    T: Float + 'static,
{
    let a = arr1(a);
    let b = arr1(b);
    a.dot(&b) / ((a.dot(&a) * (b.dot(&b))).sqrt())
}

/// Build a dissimilarity matrix of files in the `values` slice
///
/// This runs in `O(n^2)` time
///
/// Matrix returned has `values.len()` by `values.len()` dimension
///
/// A dissimilarity matrix is a matrix that tells us how similar i and j are by checking the value
/// at dmin[i,j], we precompute this to prevent us to recompute dissimilarities between  files while clustering them
///
/// Overall this is amazing
fn build_dissimilarity_matrix(values: &[FileAnnotator]) -> (Array2<f32>, Array1<usize>) {
    let mut dissimilarity_matrix = Array2::<f32>::zeros((values.len(), values.len()));
    let mut closest_cluster = Array1::<usize>::ones(values.len());
    for (i, first) in values.iter().enumerate() {
        for (j, second) in values.iter().enumerate() {
            if i == j {
                // Comparing ourself with ourself
                dissimilarity_matrix[[i, j]] = 0.0;
            } else {
                let e = 1.0 - cosine(first.vectors.as_slice(), second.vectors.as_slice());
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
/// Okay , this probably isn't slink but borrows some ideas from them
pub fn slink(docs: Vec<FileAnnotator>, cutoff: f32, f: Arc<Mutex<Vec<Articles>>>) {
    // before we slink, we need to check the size, I.e how large the files we have been sent are
    // if its above the file maximum capacity,we use multithreading and articles are sorted by their time
    // and clustered in batch(borrowed from Mindful Squirrel code),
    // otherwise we use single threaded
    if docs.len() < MAX_FILES {
        slink_single(docs.as_slice(), cutoff, f)
    } else {
        slink_multi(docs, cutoff, f)
    }
}
pub fn slink_multi(docs: Vec<FileAnnotator>, cutoff: f32, f: Arc<Mutex<Vec<Articles>>>) {
    let mut docs = docs;
    //sort unstable by using their time
    docs.sort_unstable_by(|a, b| a.time.cmp(&b.time));
    // spawn appropriate threads;
    let length = docs.len();
    // split it into batches of 8000
    let a = length / BATCH_SIZE;
    let mut each_group = Vec::with_capacity(a);
    let mut start = 0;
    let mut end = BATCH_SIZE;
    for i in 0..a {
        if i == a - 1 {
            each_group.push(&docs[start..docs.len()]);
        } else {
            each_group.push(&docs[start..end - 1]);
        }
        start += BATCH_SIZE;
        end += BATCH_SIZE;
    }
    crossbeam_utils::thread::scope(|s| {
        for i in each_group {
            let g = f.clone();
            s.spawn(move |_| slink_single(i, cutoff, g));
        }
    })
    .expect("Could not spawn clustering threads");
}
fn slink_single(docs: &[FileAnnotator], cutoff: f32, f: Arc<Mutex<Vec<Articles>>>) {
    let mut labels = Array1::<isize>::from_elem(docs.len(), -1);
    let (mut dissimilarity_matrix, min_dist) = build_dissimilarity_matrix(docs);
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
                // check if it's below Thresh-hold
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
    let mut m = HashMap::with_capacity(labels.len());
    let mut negative_counters = *labels.max().unwrap() + 1;
    for (j, i) in labels.iter().enumerate() {
        if *i != -1 {
            let a = m.entry(*i).or_insert(vec![]);
            a.push(docs[j].clone())
        } else {
            m.insert(negative_counters, vec![docs[j].clone()]);
            negative_counters += 1;
        }
    }

    // get values
    format_for_output(m.values(), f);
}
fn format_for_output(clusters: Values<isize, Vec<FileAnnotator>>, f: Arc<Mutex<Vec<Articles>>>) {
    clusters.for_each(|g| f.lock().unwrap().push(Articles::from_annotator(g.to_vec())));
}
