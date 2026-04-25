use crossterm::event::KeyCode;

pub(crate) fn is_cancel_key(key_code: &KeyCode) -> bool {
    matches!(key_code, KeyCode::Esc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_escape_is_a_cancel_key() {
        assert!(is_cancel_key(&KeyCode::Esc));
        assert!(!is_cancel_key(&KeyCode::Char('q')));
        assert!(!is_cancel_key(&KeyCode::Char('Q')));
    }
}
