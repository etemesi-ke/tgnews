#![allow(dead_code)]
use fasttext::{Args, FastText, ModelName};
use rocket::logger::error;
use std::fs::write;
use std::thread::sleep;
use std::time::Duration;

pub async fn train_en() {
    loop {
        // sleep for 5 days
        sleep(Duration::from_secs(60 * 60 * 120));

        let mut args = Args::new();
        // no of threads to spawn
        args.set_thread(16);
        // how many times the data will be visited
        args.set_epoch(25);
        args.set_dim(50);
        // set a skipgram model
        args.set_model(ModelName::SG);
        // input
        args.set_input("./server/en_articles.txt");
        args.set_verbose(0);
        //output
        args.set_save_output(true);
        let mut ft = FastText::new();
        match ft.train(&args) {
            Ok(_) => match ft.save_model("./server/en_vectors") {
                Ok(_) => {}
                Err(e) => error(format!("Could not save fast-text model \n{}", e).as_str()),
            },
            Err(e) => error(format!("FastText English model error \n{}", e).as_str()),
        }
        // Clean everything after train
        // Clean everything after train
        if let Err(e) = write("./server/en_articles.txt", b"") {
            error(format!("could not clean en_articles.txt file  \n {}", e).as_str())
        }
    }
}
pub async fn train_ru() {
    loop {
        // sleep for 4 days
        sleep(Duration::from_secs(60 * 60 * 108));

        let mut args = Args::new();
        // no of threads to spawn
        args.set_thread(16);
        // how many times the data will be visited
        args.set_epoch(25);
        args.set_dim(50);
        // set a skipgram model
        args.set_model(ModelName::SG);
        // input
        args.set_input("./server/ru_articles.txt");
        args.set_verbose(0);
        //output
        args.set_save_output(true);
        let mut ft = FastText::new();
        match ft.train(&args) {
            Ok(_) => match ft.save_model("./server/ru_vectors") {
                Ok(_) => {}
                Err(e) => error(format!("Could not save fast-text Russian model \n{}", e).as_str()),
            },
            Err(e) => error(format!("FastText Russian model error \n{}", e).as_str()),
        }
        // Clean everything after train
        if let Err(e) = write("./server/ru_articles.txt", b"") {
            error(format!("could not clean ru-articles.txt file  \n {}", e).as_str())
        }
    }
}
