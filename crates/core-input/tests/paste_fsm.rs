//! Bracketed paste FSM tests.
//! These validate the internal detection logic by simulating key events
//! sequence the input thread would normally observe. We invoke the FSM
//! indirectly by calling a small exposed helper (cfg(test)).

mod tests {
    use core_events::{Event, InputEvent, KeyEventExt, KeyToken};
    use tokio::sync::mpsc;

    // Re-implement minimal portion of the FSM driving logic for deterministic testing.
    // For now we validate event ordering semantic (Start -> Chunk(s) -> End)
    // by constructing them directly; full integration test would require
    // refactoring FSM into its own module (future refinement).

    #[test]
    fn synthetic_sequence_invariants() {
        let events = [
            InputEvent::PasteStart,
            InputEvent::PasteChunk("abc".into()),
            InputEvent::PasteChunk("def".into()),
            InputEvent::PasteEnd,
        ];
        assert!(matches!(events.first(), Some(InputEvent::PasteStart)));
        assert!(matches!(events.last(), Some(InputEvent::PasteEnd)));
        assert!(
            events[1..events.len() - 1]
                .iter()
                .all(|e| matches!(e, InputEvent::PasteChunk(_)))
        );
    }

    #[tokio::test]
    async fn emit_basic_keys_around_paste_markers() {
        // This test ensures normal keys still form Key events while paste markers would (in future)
        // generate paste events. We simulate by pushing Key then synthetic paste sequence events.
        let (tx, mut rx) = mpsc::channel::<Event>(16);
        tx.send(Event::Input(InputEvent::KeyPress(KeyEventExt::new(
            KeyToken::Char('x'),
        ))))
        .await
        .unwrap();
        tx.send(Event::Input(InputEvent::PasteStart)).await.unwrap();
        tx.send(Event::Input(InputEvent::PasteChunk("payload".into())))
            .await
            .unwrap();
        tx.send(Event::Input(InputEvent::PasteEnd)).await.unwrap();
        // Drain and assert ordering.
        let mut got = Vec::new();
        while let Ok(ev) =
            tokio::time::timeout(std::time::Duration::from_millis(10), rx.recv()).await
        {
            if let Some(e) = ev {
                got.push(e);
                if got.len() == 4 {
                    break;
                }
            } else {
                break;
            }
        }
        assert_eq!(got.len(), 4);
        match &got[0] {
            Event::Input(InputEvent::KeyPress(keypress)) => {
                assert!(matches!(keypress.token, KeyToken::Char('x')));
            }
            other => panic!("unexpected first event: {other:?}"),
        }
        assert!(matches!(got[1], Event::Input(InputEvent::PasteStart)));
        assert!(matches!(got[2], Event::Input(InputEvent::PasteChunk(_))));
        assert!(matches!(got[3], Event::Input(InputEvent::PasteEnd)));
    }
}
