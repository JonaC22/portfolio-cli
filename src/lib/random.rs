use piechart::Color;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

pub fn get_char() -> char {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(1)
        .collect::<Vec<u8>>()[0] as char
}

pub fn get_color() -> Color {
    let mut rng = rand::thread_rng();
    Color::Fixed(rng.gen_range(0..255))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn get_char_success() {
        let character = get_char();
        assert_ne!(character, ' ');
    }

    #[tokio::test]
    async fn get_color_success() {
        let _color = get_color();
        assert!(true);
    }
}
