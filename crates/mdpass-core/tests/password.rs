use std::collections::HashSet;

use mdpass_core::password::{ALPHABET, DEFAULT_LENGTH, MAX_LENGTH, MIN_LENGTH, generate};

#[test]
fn password_has_the_requested_length_and_uses_the_alphabet() {
    for length in [MIN_LENGTH, DEFAULT_LENGTH, 64] {
        let password = generate(length);
        assert_eq!(password.chars().count(), length);
        assert!(password.chars().all(|c| ALPHABET.contains(c)), "{}", password.as_str());
    }
}

#[test]
fn length_is_clamped_to_safe_bounds() {
    assert_eq!(generate(1).len(), MIN_LENGTH);
    assert_eq!(generate(10_000).len(), MAX_LENGTH);
}

#[test]
fn every_password_mixes_all_character_classes() {
    for _ in 0..200 {
        let password = generate(MIN_LENGTH);
        let p = password.as_str();
        assert!(p.chars().any(|c| c.is_ascii_lowercase()), "{p}");
        assert!(p.chars().any(|c| c.is_ascii_uppercase()), "{p}");
        assert!(p.chars().any(|c| c.is_ascii_digit()), "{p}");
        assert!(p.chars().any(|c| !c.is_ascii_alphanumeric()), "{p}");
    }
}

#[test]
fn alphabet_has_no_lookalike_characters() {
    for c in ['l', 'I', 'O', '0', '1', 'o'] {
        assert!(!ALPHABET.contains(c), "{c}");
    }
}

#[test]
fn passwords_do_not_repeat() {
    let set: HashSet<String> = (0..500).map(|_| generate(DEFAULT_LENGTH).as_str().to_owned()).collect();
    assert_eq!(set.len(), 500);
}
