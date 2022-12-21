use scale::cmd;
fn main() {
    match cmd::run() {
        Ok(_) => {}
        Err(e) => {
            println!("{:?}", e)
        }
    }
}
