# Hello there Citizen

## Building
`git clone [whatever this repository is]` and then

Run 
```
./make-solution.sh
```
Will
> * Install `protobuf-compiler`
> * Check if cargo(rust package manager) exists and if not downloads and prompts you to install
> * Run `cargo build --release` with `target-cpu=native`
> * copy the file over at `./tgnews/target/release/tgnews` to this directory
> * clean build files

If you want to do it yourself;

`apt install protobuf-compiler`

`cargo build --release`
`cp ./target/release/tgnews ./`

Then run the binary with the specified arguments

## How things work
All modules spawn `16` worker threads to handle processing 
(this can be changed with the `--threads` cmd argument)

###Languages
Use `whatlang` to provide language detection on the body of the article.
Whatlang utilizes `trigrams` to classify languages. 

Passing the whole article results in a lot of time spent in calculating trigrams, but allows me
to set high thresholds for accuracy, (a threshold of 1.0)

### News
A set of `regex based filters` that match the titles that were indicated to be evil
and flags them as ot news

### Categories
Broken into 2:
> * ##### Manual Classification
>>
>> I had time to investigate the URLs  on the websites and with a bit of wrangling, and common knowledge
>> we can easily see that some websites pre-pend certain keywords for certain categories
>> eg with a url like `https://news.com/tech/how-not-to-classify-articles` we can assume with high confidence that
>> it is a technological article
>>
>> A full list of url key-words used can be found in `src/server/categories/classifier.rs`  
>>
>> Note, when using this, we need to remove the last part of a url since it can be totally misleading
>> for example `https://news.com/food/I-love-politics` might be thought of as politics but is 
>> in fact food( I had no better example)
>
> * ##### A FastText model
>> 
>> Done by a FastText model trained on some ~~stolen data~~ freely available data from [here](https://github.com/IlyaGusev/tgcontest#data)
>> The model has 7 labels each corresponding to the categories listed 
>>
>> Text is cleaned by, removing stopwords, lower casing it, removing punctuation and removing newlines,(normalizing the whole article to one line)
>> and then the cleaned body is passed to the classifier
>>
>> Everything classified with an accuracy of less than `0.48` is dropped (I.E the were no distinct features in the article or the article is weird)

### Threads
Haaa Dissimilarity matrices are amazing
> Okay repeat steps 1,2,3

* Detect language, is news? categorize, normal stuff

* Put them to respective categories and one master category

* If the files are less than 5000(or 7000 idk) we cluster them all at once

* If more than 7000 i guess we classify them based on respective category
i.e those of society are clustered in one group,economy in another and so on so forth

* Still if a group contains more than 10000 articles(still not sure with this numbers) we sort them
 by published time, split em into batches and categorize them

* Clustering is done by some very weird SLINK algorithm
which returns a label of files whose title dissimilarities are low,

* OKay a little bit about the clustering, it pulls documents which have the lowest dissimilarity to self
* So for example say a document A is dissimilar to B by 0.12 and A is dissimilar to C by 0.15 but B
is dissimilar to C by 0.05, A won't pull B, B will pull C so we end up with something looking like
```hjson
{
 [ title:"A",
  "articles": [A],
  ],
 [
  title:"B",
 "articles":[B,C]
 ],
}
``` 
This prevents

 1. Creation of really really girnomous clusters(like 500 articles);
 because I've never met a person reading 500 articles
 2. Pushes similar news into small meaningful clusters

* Title is determined by weird stuff, but mainly influenced by `ALEXA_PAGERANK` i borrowed from [here](https://github.com/IlyaGusev/tgcontest)
so titles for large threads should always be from known sources, though not the best but its honest work.

* Then boom print those stuff like crazy

* Is it fast?
> * Well yes but actually no.
> * It's compiled with optimizations so it's okay
> * Jokes it's pretty fast  

### Server
I was partially sane and insane here.

Server chosen was rocket because of its ease of use and i defaulted to its async 0.5.0-dev branch

Async works magic

1. #### `PUT`ting of articles
> * File `/src/server/upload.rs`
> * It is preprocessed and the following things are extracted from it
>  *  `url`,`title`,`body` and `published_time`
>  * Later, the language and category of the file is determined. and the file is stored
>  * For storing we use `sled` a  persistence key value store , the key is the article file and the value is a `protobuf` representation of a `TDocument`(defined in `src/server/enums.rs`)
>  * All dirty buffers in sled are flushed after 5 minutes using a global worker thread pool

2. ###`DELETE`ing an article
> * File `/src/server/delete_article.rs`
> * Just deletes the article from the DataBase,
> * When removing an article, `sled` returns the value if it existed or not so 
> whether we get a `Some(Article)` or `None` influenced our return type
>  the former returns `204` code the latter returning a `404` in compliance with the specifications.
> 
> **NOTE**:When an article is deleted, it is still stored in the clustering index, and will be
>removed some few minutes, because deleting articles was expensive and since clusters are in real time quite frankly mind
>numbing ( though one of the contestants did manage to do that, check out [his medium article](https://medium.com/@alexkuznetsov/2nd-place-solution-for-telegram-data-clustering-contest-f28d55b98d30))
>
> I wanted to maintain my sanity so I made a compromise
>

3. ### Clustering and returning
#### Okay a request just came in

`PUT /upload/longticlename.html`
1. Detect language.
> * English or Russian? No? Okay tell the client we can't handle this.

2. Is it news?
> No tell the client we only speak news

3. Categorize it.
> Low detection accuracy.
> > Slowly drop it.. slowly..

4. Passed all the above?
> Get its Alexa rating, its title and its url its language and every info needed to present it as a 
> `/proto/server_files.proto` and then represent it as such
> 
> Add it to the cluster
> 
> Asynchronously send it to sled(a KV database) to store it.
>
#### Okay another request came in

`GET /threads/?lang_code='en'&category=science&period=3200`

And I parse that
And then we

1. Look at the category. Do i know it?, Ya , Do I have it? Ya . So lemme fetch it
2. I have the category cluster, now then we pass it to the time filter and we see how many articles actually made
   the time(meaning some clusters will be empty)
3. Remove empty clusters and for those remaining, sort them by size(not the best metric, but gets the job done)
4. Send them back like crazy


#### Okay a server reload was needed
Okay server is up

1. We got any articles in DBASE?
2. Read them and put them into threads
3. Await further commands

And i believe that is all