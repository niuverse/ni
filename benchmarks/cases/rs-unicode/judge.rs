use rs_unicode::{excerpt,filter::filter};
#[test] fn boundaries() {
    for (input,limit,want) in [("中文🙂",2,"中文…"),("中文🙂",3,"中文🙂"),("",0,""),("x",0,"…"),("e\u{301}",1,"e…"),("🙂a",1,"🙂…")] {
        assert_eq!(excerpt(input,limit),want);
    }
}
#[test] fn filtering() {assert_eq!(filter("中文abc\nskip\n中文🙂\n","中文",3),vec!["中文a…","中文🙂"]);}
