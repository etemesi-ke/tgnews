use colored::*;
use log::Level;

pub fn log_msg<T>(level: log::Level, message: T)
where
    T: AsRef<str>,
{
    match level {
        Level::Error => eprintln!("[{}]\t{}", level.to_string().red(), message.as_ref()),
        Level::Warn => eprintln!("[{}]\t{}", level.to_string().red(), message.as_ref()),
        Level::Info => {}
        Level::Debug => eprintln!("[{}]\t{}", level.to_string().blue(), message.as_ref()),
        Level::Trace => {}
    }
}
pub fn tgnews_debug<T>(_message: T)
where
    T: AsRef<str>,
{
}
pub fn tgnews_warn<T>(message: T)
where
    T: AsRef<str>,
{
    log_msg(log::Level::Warn, message);
}
