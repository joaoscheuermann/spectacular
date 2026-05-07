    use super::*;

    #[test]
    fn styles_emit_truecolor_escape_sequences() {
        assert!(paint(user_style(), "user").contains("\x1b[38;2;34;197;94m"));
        assert!(paint(dim_style(), "dim").contains("\x1b[38;2;148;163;184m"));
        assert!(paint(error_style(), "error").contains("\x1b[38;2;248;113;113m"));
        assert!(paint(selection_style(), "selected").contains("\x1b[48;2;51;65;85m"));
    }
