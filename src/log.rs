use std::io::Write;

pub fn log(message: &str) {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open("/tmp/mient.log")
        .unwrap();

    file.write_all(message.as_bytes()).unwrap();
}
