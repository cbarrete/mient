use std::io::Write;

pub fn info(message: &str) {
    log(message, "/tmp/mient.log");
}

pub fn error(message: &str) {
    log(message, "/tmp/mient_errors.log");
}

fn log(message: &str, path: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();

    file.write_all(message.as_bytes()).unwrap();
}
