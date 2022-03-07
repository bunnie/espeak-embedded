use log::{Record, Level, Metadata};
pub struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            print!("{}*{}* {} (espeak-embedded:{}:{})\r\n", // * indicates an externally linked program in the log
            record.level(),
            record.module_path().unwrap_or("unknown"),
            record.args(),
            record.file().unwrap_or("unknown"),
            record.line().unwrap_or(0));
        }
    }
    fn flush(&self) {}
}
pub static LOGGER: SimpleLogger = SimpleLogger;