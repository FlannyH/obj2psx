pub fn validate(r: std::io::Result<usize>) {
    if r.is_err() {
        panic!();
    }
}
