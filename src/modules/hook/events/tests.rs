use super::RustMailerEvent;

#[test]
fn test1() {
    let examples = RustMailerEvent::generate_event_examples();
    println!("{:#?}", examples);
}
