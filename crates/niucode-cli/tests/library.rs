use niucode::TurnHooks;

#[test]
fn library_target_exposes_the_noop_hooks() {
    fn accepts<T: niucode::TurnHooks>() {}
    accepts::<niucode::NoHooks>();

    let mut prompt = String::from("base");
    niucode::NoHooks.augment_system_prompt(&mut prompt);
    assert_eq!(prompt, "base");
}
