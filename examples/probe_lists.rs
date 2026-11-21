//! How long asking for the audio inputs takes (it runs while the menu shows).
fn main() {
    for _ in 0..5 {
        let t = std::time::Instant::now();
        let n = noctiluca::audio::list_inputs().len();
        println!("{n} inputs in {:.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    }
}
