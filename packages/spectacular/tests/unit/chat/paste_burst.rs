    use super::*;

    #[test]
    fn first_plain_char_is_held_then_flushed_as_typed() {
        let mut burst = PasteBurst::default();
        let now = Instant::now();

        assert!(matches!(burst.on_plain_char('a', now), CharDecision::Held));

        let flush_at = now + PasteBurst::recommended_flush_delay();
        assert!(matches!(
            burst.flush_if_due(flush_at),
            FlushResult::Typed('a')
        ));
        assert!(!burst.is_active());
    }

    #[test]
    fn two_fast_plain_chars_start_buffered_paste() {
        let mut burst = PasteBurst::default();
        let now = Instant::now();

        assert!(matches!(burst.on_plain_char('a', now), CharDecision::Held));
        assert!(matches!(
            burst.on_plain_char('b', now + Duration::from_millis(1)),
            CharDecision::Buffered
        ));

        let flush_at =
            now + Duration::from_millis(1) + PasteBurst::recommended_active_flush_delay();
        assert!(matches!(
            burst.flush_if_due(flush_at),
            FlushResult::Paste(ref pasted) if pasted == "ab"
        ));
    }

    #[test]
    fn newline_materializes_pending_first_char_before_the_line_break() {
        let mut burst = PasteBurst::default();
        let now = Instant::now();

        assert!(matches!(burst.on_plain_char('a', now), CharDecision::Held));
        assert!(burst.append_newline_if_active(now + Duration::from_millis(1)));

        let flush_at =
            now + Duration::from_millis(1) + PasteBurst::recommended_active_flush_delay();
        assert!(matches!(
            burst.flush_if_due(flush_at),
            FlushResult::Paste(ref pasted) if pasted == "a\n"
        ));
    }
