#[macro_export]
macro_rules! lazy_regex {
    ($($x:ident:$y:tt),*) => {
        $(pub static $x : LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new($y).unwrap());)*};
}
