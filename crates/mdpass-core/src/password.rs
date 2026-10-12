//! Генератор паролей.

use zeroize::Zeroizing;

pub const MIN_LENGTH: usize = 8;
pub const DEFAULT_LENGTH: usize = 20;
pub const MAX_LENGTH: usize = 128;

/// Без похожих символов: нет l, I, 1, o, O, 0.
const LOWER: &str = "abcdefghijkmnpqrstuvwxyz";
const UPPER: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const DIGITS: &str = "23456789";
/// Символы, которые без экранирования вводятся в оболочке и формах.
const SYMBOLS: &str = "!@#$%&*-_";

pub const ALPHABET: &str = concat!("abcdefghijkmnpqrstuvwxyz", "ABCDEFGHJKLMNPQRSTUVWXYZ", "23456789", "!@#$%&*-_");

/// Случайный пароль из [`ALPHABET`], в котором есть буквы обоих регистров, цифра
/// и символ. Длина приводится к границам `MIN_LENGTH..=MAX_LENGTH`.
pub fn generate(length: usize) -> Zeroizing<String> {
    let length = length.clamp(MIN_LENGTH, MAX_LENGTH);
    let alphabet = ALPHABET.as_bytes();
    loop {
        let mut password = Zeroizing::new(String::with_capacity(length));
        for _ in 0..length {
            password.push(char::from(alphabet[random_below(alphabet.len())]));
        }
        let has = |class: &str| password.chars().any(|c| class.contains(c));
        if has(LOWER) && has(UPPER) && has(DIGITS) && has(SYMBOLS) {
            return password;
        }
    }
}

/// Равномерное случайное число в `0..bound` без перекоса остатка от деления.
fn random_below(bound: usize) -> usize {
    let bound = u32::try_from(bound).expect("маленький алфавит");
    let zone = u32::MAX - u32::MAX % bound;
    loop {
        let mut bytes = [0u8; 4];
        getrandom::fill(&mut bytes).expect("системный генератор случайных чисел");
        let value = u32::from_le_bytes(bytes);
        if value < zone {
            return (value % bound) as usize;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_is_the_union_of_the_classes() {
        assert_eq!(ALPHABET, [LOWER, UPPER, DIGITS, SYMBOLS].concat());
    }
}
